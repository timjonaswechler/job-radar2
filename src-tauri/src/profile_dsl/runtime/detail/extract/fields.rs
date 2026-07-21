use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::profile_dsl::runtime::detail) struct FieldEvaluation {
    pub(in crate::profile_dsl::runtime::detail) value: Option<String>,
    pub(in crate::profile_dsl::runtime::detail) failed: bool,
}

pub(in crate::profile_dsl::runtime::detail) fn evaluate_string_field(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
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

    let values = values
        .into_iter()
        .map(|value| normalize_whitespace_text(value.trim()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    match cardinality.execute(values) {
        Ok(outcome) => FieldEvaluation {
            value: match outcome {
                CardinalityOutcome::Scalar(value) => value,
                CardinalityOutcome::Sequence(values) => values.into_iter().next(),
            },
            failed: false,
        },
        Err(error) => {
            diagnostics.push(error.into_diagnostic(CardinalityDiagnosticContext {
                path,
                strategy_key,
                item_index: None,
            }));
            FieldEvaluation {
                value: None,
                failed: true,
            }
        }
    }
}

pub(in crate::profile_dsl::runtime::detail) struct RawFieldValues<'a> {
    pub(in crate::profile_dsl::runtime::detail) values: Vec<TransformValue<'static, 'static>>,
    pub(in crate::profile_dsl::runtime::detail) failed: bool,
    pub(in crate::profile_dsl::runtime::detail) cardinality: CompiledCardinality,
    pub(in crate::profile_dsl::runtime::detail) transforms: &'a CompiledTransformPipeline,
}

fn raw_field_values<'a>(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
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
        } => json_value_to_transform_values(value).into_raw(*cardinality, transforms),
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match resolve_simple_json_path(value, json_path) {
                Ok(Some(value)) => {
                    json_value_to_transform_values(value).into_raw(*cardinality, transforms)
                }
                Ok(None) => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms,
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
                        transforms: transforms,
                    }
                }
            },
            _ => incompatible_field_expression(
                "field_json_path_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms,
                diagnostics,
            ),
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => match source_config.get(key) {
            Some(value) => json_value_to_transform_values(value).into_raw(*cardinality, transforms),
            None => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
        },
        FieldExpression::PostingMeta {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: posting
                .posting_meta
                .get(key)
                .cloned()
                .into_iter()
                .map(TransformValue::Text)
                .collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms,
        },
        FieldExpression::Capture {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: captures
                .get(key)
                .cloned()
                .into_iter()
                .map(TransformValue::Text)
                .collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms,
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match value.get(key) {
                Some(value) => {
                    json_value_to_transform_values(value).into_raw(*cardinality, transforms)
                }
                None => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms,
                },
            },
            RuntimeItem::Text(value) if key == "value" || key == "." => RawFieldValues {
                values: vec![TransformValue::Text(value.clone())],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
            _ => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
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
                    values: vec![TransformValue::Text(value)],
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms,
                },
                Err(message) => {
                    diagnostics.push(runtime_error(
                        "runtime_template_context_missing",
                        format!("Field template could not be rendered: {message}"),
                        path,
                        strategy_key,
                        json!({}),
                    ));
                    RawFieldValues {
                        values: Vec::new(),
                        failed: true,
                        cardinality: *cardinality,
                        transforms: transforms,
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
                values: xml_path_texts(*node, text_path)
                    .into_iter()
                    .map(TransformValue::Text)
                    .collect(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
            RuntimeItem::Text(value) if text_path == "." => RawFieldValues {
                values: vec![TransformValue::Text(value.clone())],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
            _ => incompatible_field_expression(
                "field_xml_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms,
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
                    .map(TransformValue::Text)
                    .collect(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
            _ => incompatible_field_expression(
                "field_xml_element_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms,
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
                    .into_raw(*cardinality, transforms)
            }
            _ => incompatible_field_expression(
                "field_css_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms,
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
                    .into_raw(*cardinality, transforms)
            }
            _ => incompatible_field_expression(
                "field_css_attribute_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms,
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
        .into_raw(*cardinality, transforms),
        FieldExpression::FirstNonEmpty {
            cardinality,
            transforms,
            ..
        } => {
            diagnostics.push(runtime_error(
                "unsupported_field_expression",
                "first_non_empty execution is not available in this runtime revision",
                path,
                strategy_key,
                json!({}),
            ));
            RawFieldValues {
                values: Vec::new(),
                failed: true,
                cardinality: *cardinality,
                transforms,
            }
        }
    }
}

fn combine_field_values(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
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
    cardinality: CompiledCardinality,
    transforms: &'a CompiledTransformPipeline,
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
    values: Vec<TransformValue<'static, 'static>>,
    transforms: &CompiledTransformPipeline,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    let input = TransformShape::Sequence(values);
    match transforms.execute(input) {
        Ok(output) => {
            let mut values = Vec::new();
            for (value_index, value) in output.into_values().into_iter().enumerate() {
                match value {
                    TransformValue::Text(value) => values.push(value),
                    TransformValue::Json(Value::String(value)) => values.push(value),
                    TransformValue::Json(Value::Number(value)) => values.push(value.to_string()),
                    TransformValue::Json(Value::Bool(value)) => values.push(value.to_string()),
                    TransformValue::Json(Value::Null) => {}
                    TransformValue::Json(Value::Array(_) | Value::Object(_))
                    | TransformValue::Xml(_)
                    | TransformValue::Html(_) => {
                        diagnostics.push(runtime_error(
                            "field_type_mismatch",
                            "Field value must resolve to text or a JSON scalar",
                            path,
                            strategy_key,
                            json!({ "valueIndex": value_index }),
                        ));
                        return None;
                    }
                }
            }
            Some(values)
        }
        Err(error) => {
            diagnostics.push(runtime_error(
                match error.kind {
                    TransformErrorKind::TypeMismatch => "transform_type_mismatch",
                    TransformErrorKind::InvalidPercentEncoding => {
                        "transform_invalid_percent_encoding"
                    }
                    TransformErrorKind::InvalidUtf8 => "transform_invalid_utf8",
                },
                error.message,
                path,
                strategy_key,
                json!({
                    "transformIndex": error.transform_index,
                    "valueIndex": error.value_index,
                }),
            ));
            None
        }
    }
}
