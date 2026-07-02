use super::document::RuntimeItem;
use super::support::{render_template, TemplateRuntimeContext};
use super::values::{
    css_attribute_values, css_text_values, json_value_to_strings, xml_descendant_elements,
    xml_node_text, xml_path_texts, JsonStringsResult,
};
use super::*;

pub(super) fn evaluate_strategy_captures(
    strategy: &ExecutionPlanPostingDetailStrategy,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let mut captures = BTreeMap::new();
    let empty_document = Value::Null;
    let empty_item = RuntimeItem::Json(&empty_document);
    let Some(capture_rules) = &strategy.captures else {
        return Some(captures);
    };

    for (key, rule) in capture_rules {
        let path = format!("{base_path}/captures/{key}");
        let context_captures = captures.clone();
        let evaluation = evaluate_string_field(
            &empty_item,
            source_config,
            posting,
            &context_captures,
            &rule.from,
            &format!("{path}/from"),
            strategy_key,
            diagnostics,
        );
        if evaluation.failed {
            return None;
        }
        let Some(value) = evaluation.value else {
            diagnostics.push(runtime_error(
                "capture_source_missing",
                format!("Capture `{key}` source did not resolve to text"),
                &path,
                strategy_key,
                json!({ "captureKey": key }),
            ));
            return None;
        };
        let Some(captured) =
            apply_capture_rule(key, &value, rule, &path, strategy_key, diagnostics)
        else {
            return None;
        };
        captures.insert(key.clone(), captured);
    }

    Some(captures)
}

fn apply_capture_rule(
    key: &str,
    value: &str,
    rule: &CaptureRule,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    let regex = match Regex::new(&rule.pattern) {
        Ok(regex) => regex,
        Err(error) => {
            diagnostics.push(runtime_error(
                "capture_pattern_invalid",
                format!("Capture `{key}` pattern is invalid: {error}"),
                format!("{path}/pattern"),
                strategy_key,
                json!({ "captureKey": key, "error": error.to_string() }),
            ));
            return None;
        }
    };
    let Some(captures) = regex.captures(value) else {
        diagnostics.push(runtime_error(
            "capture_not_matched",
            format!("Capture `{key}` pattern did not match runtime text"),
            path,
            strategy_key,
            json!({ "captureKey": key }),
        ));
        return None;
    };

    let captured = captures
        .name("value")
        .or_else(|| {
            regex
                .capture_names()
                .flatten()
                .find_map(|name| captures.name(name))
        })
        .or_else(|| captures.get(1))
        .or_else(|| captures.get(0))
        .map(|matched| matched.as_str().trim().to_string())
        .filter(|value| !value.is_empty());

    match captured {
        Some(value) => Some(value),
        None => {
            diagnostics.push(runtime_error(
                "capture_empty",
                format!("Capture `{key}` resolved to empty text"),
                path,
                strategy_key,
                json!({ "captureKey": key }),
            ));
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FieldEvaluation {
    pub(super) value: Option<String>,
    pub(super) failed: bool,
}

pub(super) fn evaluate_string_field(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let RawFieldValues {
        values,
        failed,
        cardinality,
        transforms,
    } = raw_field_values(
        document,
        source_config,
        posting,
        captures,
        expression,
        path,
        strategy_key,
        diagnostics,
    );
    if failed {
        return FieldEvaluation {
            value: None,
            failed: true,
        };
    }

    let values = match apply_transforms(values, transforms, path, strategy_key, diagnostics) {
        Some(values) => values,
        None => {
            return FieldEvaluation {
                value: None,
                failed: true,
            };
        }
    };

    let mut normalized_values = Vec::new();
    for value in values {
        let value = normalize_whitespace(value.trim());
        if !value.is_empty() {
            normalized_values.push(value);
        }
    }

    match cardinality.unwrap_or(Cardinality::One) {
        Cardinality::One => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => cardinality_mismatch(path, strategy_key, count, "one", diagnostics),
        },
        Cardinality::First => FieldEvaluation {
            value: normalized_values.into_iter().next(),
            failed: false,
        },
        Cardinality::Optional => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => cardinality_mismatch(path, strategy_key, count, "optional", diagnostics),
        },
        Cardinality::All => FieldEvaluation {
            value: normalized_values.into_iter().next(),
            failed: false,
        },
    }
}

fn cardinality_mismatch(
    path: &str,
    strategy_key: Option<&str>,
    actual_count: usize,
    expected: &str,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    diagnostics.push(runtime_error(
        "field_cardinality_mismatch",
        format!("Field cardinality `{expected}` did not match {actual_count} resolved values"),
        path,
        strategy_key,
        json!({
            "expectedCardinality": expected,
            "actualCount": actual_count,
        }),
    ));
    FieldEvaluation {
        value: None,
        failed: true,
    }
}

pub(super) struct RawFieldValues<'a> {
    pub(super) values: Vec<String>,
    pub(super) failed: bool,
    pub(super) cardinality: Option<Cardinality>,
    pub(super) transforms: Option<&'a Vec<Transform>>,
}

fn raw_field_values<'a>(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &'a FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    match expression {
        FieldExpression::Const {
            value,
            cardinality,
            transforms,
        } => json_value_to_strings(value, path, strategy_key, diagnostics)
            .into_raw(*cardinality, transforms.as_ref()),
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match resolve_simple_json_path(value, json_path) {
                Ok(Some(value)) => json_value_to_strings(value, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref()),
                Ok(None) => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "field_json_path_failed",
                        format!("Field JSONPath is invalid: {error}"),
                        path,
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    RawFieldValues {
                        values: Vec::new(),
                        failed: true,
                        cardinality: *cardinality,
                        transforms: transforms.as_ref(),
                    }
                }
            },
            _ => incompatible_field_expression(
                "field_json_path_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => match source_config.get(key) {
            Some(value) => json_value_to_strings(value, path, strategy_key, diagnostics)
                .into_raw(*cardinality, transforms.as_ref()),
            None => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::PostingMeta {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: posting.posting_meta.get(key).cloned().into_iter().collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms.as_ref(),
        },
        FieldExpression::Capture {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: captures.get(key).cloned().into_iter().collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms.as_ref(),
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match value.get(key) {
                Some(value) => json_value_to_strings(value, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref()),
                None => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
            },
            RuntimeItem::Text(value) if key == "value" || key == "." => RawFieldValues {
                values: vec![value.clone()],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::Template {
            template,
            cardinality,
            transforms,
        } => {
            let context = TemplateRuntimeContext {
                source_config,
                posting,
                posting_meta: &posting.posting_meta,
                captures,
            };
            match render_template(template, &context) {
                Ok(value) => RawFieldValues {
                    values: vec![value],
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
                Err(message) => {
                    diagnostics.push(runtime_error(
                        "runtime_template_context_missing",
                        format!("Field template could not be rendered: {message}"),
                        path,
                        strategy_key,
                        json!({ "template": template }),
                    ));
                    RawFieldValues {
                        values: Vec::new(),
                        failed: true,
                        cardinality: *cardinality,
                        transforms: transforms.as_ref(),
                    }
                }
            }
        }
        FieldExpression::XmlText {
            text_path,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Xml(node) => RawFieldValues {
                values: xml_path_texts(*node, text_path),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            RuntimeItem::Text(value) if text_path == "." => RawFieldValues {
                values: vec![value.clone()],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => incompatible_field_expression(
                "field_xml_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::XmlElement {
            element,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Xml(node) => RawFieldValues {
                values: xml_descendant_elements(*node, element)
                    .into_iter()
                    .map(xml_node_text)
                    .collect(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => incompatible_field_expression(
                "field_xml_element_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::CssText {
            selector,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Html(node) => {
                css_text_values(node, selector, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            _ => incompatible_field_expression(
                "field_css_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::CssAttribute {
            selector,
            attribute,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Html(node) => {
                css_attribute_values(node, selector, attribute, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            _ => incompatible_field_expression(
                "field_css_attribute_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::Combine {
            parts,
            join,
            cardinality,
            transforms,
        } => combine_field_values(
            document,
            source_config,
            posting,
            captures,
            parts,
            join.as_deref().unwrap_or_default(),
            path,
            strategy_key,
            diagnostics,
        )
        .into_raw(*cardinality, transforms.as_ref()),
    }
}

fn combine_field_values(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    parts: &[CombinePart],
    join: &str,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part_path = format!("{path}/parts/{index}/value");
        match evaluate_string_field(
            document,
            source_config,
            posting,
            captures,
            &part.value,
            &part_path,
            strategy_key,
            diagnostics,
        ) {
            FieldEvaluation {
                value: Some(value),
                failed: false,
            } => values.push(value),
            FieldEvaluation {
                value: None,
                failed: false,
            } if part.optional.unwrap_or(false) => {}
            FieldEvaluation {
                value: None,
                failed: false,
            } => {
                diagnostics.push(runtime_error(
                    "required_combine_part_missing",
                    "Required combine part did not resolve to a non-empty string",
                    &part_path,
                    strategy_key,
                    json!({ "partIndex": index }),
                ));
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
            FieldEvaluation { failed: true, .. } => {
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
        }
    }

    JsonStringsResult {
        values: vec![values.join(join)],
        failed: false,
    }
}

fn incompatible_field_expression<'a>(
    code: &'static str,
    path: &str,
    strategy_key: Option<&str>,
    cardinality: Option<Cardinality>,
    transforms: Option<&'a Vec<Transform>>,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    diagnostics.push(runtime_error(
        code,
        "Field expression is not compatible with the selected detail document type",
        path,
        strategy_key,
        json!({}),
    ));
    RawFieldValues {
        values: Vec::new(),
        failed: true,
        cardinality,
        transforms,
    }
}

fn apply_transforms(
    values: Vec<String>,
    transforms: Option<&Vec<Transform>>,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    match apply_transform_pipeline(values, transforms) {
        Ok(values) => Some(values),
        Err(error) => {
            diagnostics.push(runtime_error(
                error.code,
                error.message,
                path,
                strategy_key,
                json!({ "transform": error.transform }),
            ));
            None
        }
    }
}
