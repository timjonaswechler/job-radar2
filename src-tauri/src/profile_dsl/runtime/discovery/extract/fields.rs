use super::*;

pub(in crate::profile_dsl::runtime::discovery) struct FieldEvaluation {
    pub(in crate::profile_dsl::runtime::discovery) value: Option<String>,
    pub(in crate::profile_dsl::runtime::discovery) failed: bool,
}

pub(in crate::profile_dsl::runtime::discovery) fn evaluate_string_field(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let RawFieldValues {
        values,
        failed,
        cardinality,
        transforms,
    } = raw_field_values(
        item,
        source_config,
        source_name,
        captures,
        expression,
        path,
        strategy_key,
        item_index,
        diagnostics,
    );
    if failed {
        return FieldEvaluation {
            value: None,
            failed: true,
        };
    }

    let values = match apply_transforms(
        values,
        transforms,
        path,
        strategy_key,
        item_index,
        diagnostics,
    ) {
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
                item_index: Some(item_index),
            }));
            FieldEvaluation {
                value: None,
                failed: true,
            }
        }
    }
}

pub(in crate::profile_dsl::runtime::discovery) struct RawFieldValues<'a> {
    pub(in crate::profile_dsl::runtime::discovery) values: Vec<TransformValue<'static, 'static>>,
    pub(in crate::profile_dsl::runtime::discovery) failed: bool,
    pub(in crate::profile_dsl::runtime::discovery) cardinality: CompiledCardinality,
    pub(in crate::profile_dsl::runtime::discovery) transforms: &'a CompiledTransformPipeline,
}

pub(in crate::profile_dsl::runtime::discovery) fn raw_field_values<'a>(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &'a FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
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
        } => match item {
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
                        json!({
                            "itemIndex": item_index,
                            "jsonPath": json_path,
                            "error": error.to_string(),
                        }),
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
                item_index,
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
        } => match item {
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
        } => match super::super::support::render_template_with_captures(
            template,
            source_config,
            source_name,
            captures,
        ) {
            Ok(value) => RawFieldValues {
                values: vec![TransformValue::Text(value)],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms,
            },
            Err(message) => {
                diagnostics.push(runtime_error(
                    "field_template_failed",
                    format!("Field template could not be rendered: {message}"),
                    path,
                    strategy_key,
                    json!({ "itemIndex": item_index }),
                ));
                RawFieldValues {
                    values: Vec::new(),
                    failed: true,
                    cardinality: *cardinality,
                    transforms: transforms,
                }
            }
        },
        FieldExpression::XmlText {
            text_path,
            cardinality,
            transforms,
        } => match item {
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
                item_index,
                *cardinality,
                transforms,
                diagnostics,
            ),
        },
        FieldExpression::XmlElement {
            element,
            cardinality,
            transforms,
        } => match item {
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
                item_index,
                *cardinality,
                transforms,
                diagnostics,
            ),
        },
        FieldExpression::CssText {
            selector,
            cardinality,
            transforms,
        } => match item {
            RuntimeItem::Html(node) => {
                css_text_values(node, selector, path, strategy_key, item_index, diagnostics)
                    .into_raw(*cardinality, transforms)
            }
            _ => incompatible_field_expression(
                "field_css_text_incompatible",
                path,
                strategy_key,
                item_index,
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
        } => match item {
            RuntimeItem::Html(node) => css_attribute_values(
                node,
                selector,
                attribute,
                path,
                strategy_key,
                item_index,
                diagnostics,
            )
            .into_raw(*cardinality, transforms),
            _ => incompatible_field_expression(
                "field_css_attribute_incompatible",
                path,
                strategy_key,
                item_index,
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
            item,
            source_config,
            source_name,
            captures,
            parts,
            join.as_deref().unwrap_or_default(),
            path,
            strategy_key,
            item_index,
            diagnostics,
        )
        .into_raw(*cardinality, transforms),
        FieldExpression::PostingMeta {
            cardinality,
            transforms,
            ..
        } => {
            diagnostics.push(runtime_error(
                "unsupported_field_expression",
                "discovery runtime supports const, template, sourceConfig, capture, itemField, JSONPath, XML, CSS, and combine field expressions",
                path,
                strategy_key,
                json!({ "itemIndex": item_index }),
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
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    parts: &[CombinePart],
    join: &str,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part_path = format!("{path}/parts/{index}/value");
        match evaluate_string_field(
            item,
            source_config,
            source_name,
            captures,
            &part.value,
            &part_path,
            strategy_key,
            item_index,
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
                    json!({ "itemIndex": item_index, "partIndex": index }),
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
    item_index: usize,
    cardinality: CompiledCardinality,
    transforms: &'a CompiledTransformPipeline,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    diagnostics.push(runtime_error(
        code,
        "Field expression is not compatible with the selected item document type",
        path,
        strategy_key,
        json!({ "itemIndex": item_index }),
    ));
    RawFieldValues {
        values: Vec::new(),
        failed: true,
        cardinality,
        transforms,
    }
}

pub(in crate::profile_dsl::runtime::discovery) fn apply_transforms(
    values: Vec<TransformValue<'static, 'static>>,
    transforms: &CompiledTransformPipeline,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
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
                            json!({ "itemIndex": item_index, "valueIndex": value_index }),
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
                    "itemIndex": item_index,
                    "transformIndex": error.transform_index,
                    "valueIndex": error.value_index,
                }),
            ));
            None
        }
    }
}
