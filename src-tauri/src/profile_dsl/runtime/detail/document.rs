use super::values::{xml_descendant_elements, xml_path_texts};
use super::*;

pub(super) enum ParsedDocument<'body> {
    Json(Value),
    Xml(roxmltree::Document<'body>),
    Html(HtmlDocument),
}

#[derive(Clone)]
pub(super) enum RuntimeItem<'doc, 'body> {
    Json(&'doc Value),
    Xml(roxmltree::Node<'doc, 'body>),
    XmlCollection(Vec<roxmltree::Node<'doc, 'body>>),
    Html(NodeRef<'doc>),
    Text(String),
}

pub(super) fn parse_response_document<'body>(
    body: &'body str,
    strategy: &ExecutionPlanDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<ParsedDocument<'body>> {
    match strategy.parse.parse_type {
        ParseType::Json => match serde_json::from_str(body) {
            Ok(document) => Some(ParsedDocument::Json(document)),
            Err(error) => {
                diagnostics.push(runtime_error(
                    "json_parse_failed",
                    format!("Fetched response could not be parsed as JSON: {error}"),
                    format!("{base_path}/parse"),
                    strategy_key,
                    json!({ "error": error.to_string() }),
                ));
                None
            }
        },
        ParseType::Xml => match roxmltree::Document::parse(body) {
            Ok(document) => Some(ParsedDocument::Xml(document)),
            Err(error) => {
                diagnostics.push(runtime_error(
                    "xml_parse_failed",
                    format!("Fetched response could not be parsed as XML: {error}"),
                    format!("{base_path}/parse"),
                    strategy_key,
                    json!({ "error": error.to_string() }),
                ));
                None
            }
        },
        ParseType::Html => Some(ParsedDocument::Html(HtmlDocument::from(body))),
        ParseType::Text => {
            diagnostics.push(runtime_error(
                "unsupported_parse_type",
                "postingDetail runtime supports JSON, XML, and HTML parse types",
                format!("{base_path}/parse/type"),
                strategy_key,
                json!({ "supportedTypes": ["json", "xml", "html"] }),
            ));
            None
        }
    }
}

pub(super) fn select_detail_document<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &Select,
    allow_collection: bool,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    match (document, select) {
        (ParsedDocument::Json(document), Select::Document) => Some(RuntimeItem::Json(document)),
        (ParsedDocument::Json(document), Select::JsonPath { json_path }) => {
            match resolve_simple_json_path(document, json_path) {
                Ok(Some(value)) => Some(RuntimeItem::Json(value)),
                Ok(None) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_missing",
                        "JSONPath selector did not match a posting detail document",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_failed",
                        format!("JSONPath selector is invalid: {error}"),
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Xml(document), Select::Document) => {
            Some(RuntimeItem::Xml(document.root_element()))
        }
        (ParsedDocument::Xml(document), Select::XmlElement { element }) => {
            let mut items = xml_descendant_elements(document.root_element(), element);
            match items.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "xml_select_missing",
                        "XML element selector did not match a posting detail document",
                        format!("{base_path}/select/element"),
                        strategy_key,
                        json!({ "element": element }),
                    ));
                    None
                }
                _ if allow_collection => Some(RuntimeItem::XmlCollection(items)),
                1 => Some(RuntimeItem::Xml(items.remove(0))),
                count => {
                    diagnostics.push(runtime_error(
                        "xml_select_multiple",
                        "XML element selector matched multiple posting detail documents",
                        format!("{base_path}/select/element"),
                        strategy_key,
                        json!({ "element": element, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Xml(document), Select::XmlText { text_path }) => {
            let texts = xml_path_texts(document.root_element(), text_path);
            match texts.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "xml_text_select_missing",
                        "XML text selector did not match posting detail text",
                        format!("{base_path}/select/textPath"),
                        strategy_key,
                        json!({ "textPath": text_path }),
                    ));
                    None
                }
                1 => Some(RuntimeItem::Text(texts.into_iter().next().unwrap())),
                count => {
                    diagnostics.push(runtime_error(
                        "xml_text_select_multiple",
                        "XML text selector matched multiple posting detail text values",
                        format!("{base_path}/select/textPath"),
                        strategy_key,
                        json!({ "textPath": text_path, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Html(document), Select::Document) => {
            Some(RuntimeItem::Html(document.tree.root()))
        }
        (ParsedDocument::Html(document), Select::Css { selector }) => {
            let matcher = match Matcher::new(selector) {
                Ok(matcher) => matcher,
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "css_select_failed",
                        format!("CSS selector is invalid: {error:?}"),
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector, "error": format!("{error:?}") }),
                    ));
                    return None;
                }
            };
            let mut nodes = document
                .select_matcher(&matcher)
                .nodes()
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            match nodes.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "css_select_missing",
                        "CSS selector did not match a posting detail document",
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector }),
                    ));
                    None
                }
                1 => Some(RuntimeItem::Html(nodes.remove(0))),
                count => {
                    diagnostics.push(runtime_error(
                        "css_select_multiple",
                        "CSS selector matched multiple posting detail documents",
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        _ => {
            diagnostics.push(runtime_error(
                "unsupported_select_type",
                "Select type is not compatible with the parsed response document",
                format!("{base_path}/select"),
                strategy_key,
                json!({}),
            ));
            None
        }
    }
}
