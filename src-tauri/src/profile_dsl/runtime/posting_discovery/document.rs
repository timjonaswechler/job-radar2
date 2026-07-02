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
    Html(NodeRef<'doc>),
    Text(String),
}

pub(super) fn parse_response_document<'body>(
    body: &'body str,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
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
                "postingDiscovery runtime supports JSON, XML, and HTML parse types",
                format!("{base_path}/parse/type"),
                strategy_key,
                json!({ "supportedTypes": ["json", "xml", "html"] }),
            ));
            None
        }
    }
}

pub(super) fn select_items<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &Select,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<RuntimeItem<'doc, 'body>>> {
    match (document, select) {
        (ParsedDocument::Json(document), Select::JsonPath { json_path }) => {
            let selected = match resolve_simple_json_path(document, json_path) {
                Ok(selected) => selected,
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_failed",
                        format!("JSONPath selector is invalid: {error}"),
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    return None;
                }
            };

            match selected {
                Some(Value::Array(items)) => Some(items.iter().map(RuntimeItem::Json).collect()),
                Some(_) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_not_array",
                        "JSONPath selector must resolve to an array for postingDiscovery",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
                None => {
                    diagnostics.push(runtime_error(
                        "json_path_select_missing",
                        "JSONPath selector did not match a posting item collection",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Json(document), Select::Document) => match document {
            Value::Array(items) => Some(items.iter().map(RuntimeItem::Json).collect()),
            value => Some(vec![RuntimeItem::Json(value)]),
        },
        (ParsedDocument::Xml(document), Select::XmlElement { element }) => {
            let items = xml_descendant_elements(document.root_element(), element)
                .into_iter()
                .map(RuntimeItem::Xml)
                .collect::<Vec<_>>();
            if items.is_empty() {
                diagnostics.push(runtime_error(
                    "xml_select_missing",
                    "XML element selector did not match any posting items",
                    format!("{base_path}/select/element"),
                    strategy_key,
                    json!({ "element": element }),
                ));
                None
            } else {
                Some(items)
            }
        }
        (ParsedDocument::Xml(document), Select::XmlText { text_path }) => {
            let items = xml_path_texts(document.root_element(), text_path)
                .into_iter()
                .map(RuntimeItem::Text)
                .collect::<Vec<_>>();
            if items.is_empty() {
                diagnostics.push(runtime_error(
                    "xml_text_select_missing",
                    "XML text selector did not match any text values",
                    format!("{base_path}/select/textPath"),
                    strategy_key,
                    json!({ "textPath": text_path }),
                ));
                None
            } else {
                Some(items)
            }
        }
        (ParsedDocument::Xml(document), Select::Document) => {
            Some(vec![RuntimeItem::Xml(document.root_element())])
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
            let items = document
                .select_matcher(&matcher)
                .nodes()
                .iter()
                .cloned()
                .map(RuntimeItem::Html)
                .collect::<Vec<_>>();
            if items.is_empty() {
                diagnostics.push(runtime_error(
                    "css_select_missing",
                    "CSS selector did not match any posting items",
                    format!("{base_path}/select/selector"),
                    strategy_key,
                    json!({ "selector": selector }),
                ));
                None
            } else {
                Some(items)
            }
        }
        (ParsedDocument::Html(document), Select::Document) => {
            Some(vec![RuntimeItem::Html(document.tree.root())])
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
