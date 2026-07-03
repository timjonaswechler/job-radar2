use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::profile_dsl::runtime::posting_detail) struct FieldEvaluation {
    pub(in crate::profile_dsl::runtime::posting_detail) value: Option<String>,
    pub(in crate::profile_dsl::runtime::posting_detail) failed: bool,
}

pub(in crate::profile_dsl::runtime::posting_detail) fn evaluate_string_field(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
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
        source_name,
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

pub(in crate::profile_dsl::runtime::posting_detail) struct RawFieldValues<'a> {
    pub(in crate::profile_dsl::runtime::posting_detail) values: Vec<String>,
    pub(in crate::profile_dsl::runtime::posting_detail) failed: bool,
    pub(in crate::profile_dsl::runtime::posting_detail) cardinality: Option<Cardinality>,
    pub(in crate::profile_dsl::runtime::posting_detail) transforms: Option<&'a Vec<Transform>>,
}

fn raw_field_values<'a>(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
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
                source_name,
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
            source_name,
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
    source_name: &str,
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
            source_name,
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
