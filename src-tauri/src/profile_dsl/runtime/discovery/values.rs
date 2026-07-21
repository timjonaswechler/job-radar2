use super::extract::RawFieldValues;
use super::*;
pub(super) use crate::profile_dsl::primitives::select::{
    xml_descendant_elements, xml_node_text, xml_path_texts,
};

pub(super) fn css_text_values(
    node: &NodeRef<'_>,
    selector: &str,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) =
        select_relative_html(node, selector, path, strategy_key, item_index, diagnostics)
    else {
        return JsonStringsResult {
            values: Vec::new(),
            failed: true,
        };
    };
    JsonStringsResult {
        values: selection
            .iter()
            .map(|selected| selected.formatted_text().to_string())
            .collect(),
        failed: false,
    }
}

pub(super) fn css_attribute_values(
    node: &NodeRef<'_>,
    selector: &str,
    attribute: &str,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) =
        select_relative_html(node, selector, path, strategy_key, item_index, diagnostics)
    else {
        return JsonStringsResult {
            values: Vec::new(),
            failed: true,
        };
    };
    JsonStringsResult {
        values: selection
            .iter()
            .filter_map(|selected| selected.attr(attribute).map(|value| value.to_string()))
            .collect(),
        failed: false,
    }
}

fn select_relative_html<'a>(
    node: &NodeRef<'a>,
    selector: &str,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<HtmlSelection<'a>> {
    let matcher = match Matcher::new(selector) {
        Ok(matcher) => matcher,
        Err(error) => {
            diagnostics.push(runtime_error(
                "field_css_selector_failed",
                format!("Field CSS selector is invalid: {error:?}"),
                path,
                strategy_key,
                json!({
                    "itemIndex": item_index,
                    "selector": selector,
                    "error": format!("{error:?}"),
                }),
            ));
            return None;
        }
    };
    Some(HtmlSelection::from(node.clone()).select_matcher(&matcher))
}

pub(super) struct JsonStringsResult {
    pub(super) values: Vec<String>,
    pub(super) failed: bool,
}

impl JsonStringsResult {
    pub(super) fn into_raw(
        self,
        cardinality: Option<Cardinality>,
        transforms: Option<&Vec<Transform>>,
    ) -> RawFieldValues<'_> {
        RawFieldValues {
            values: self.values,
            failed: self.failed,
            cardinality,
            transforms,
        }
    }
}

pub(super) fn json_value_to_strings(
    value: &Value,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    match value {
        Value::Null => JsonStringsResult {
            values: Vec::new(),
            failed: false,
        },
        Value::String(value) => JsonStringsResult {
            values: vec![value.clone()],
            failed: false,
        },
        Value::Number(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Bool(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Array(values) => {
            let mut strings = Vec::new();
            for (value_index, value) in values.iter().enumerate() {
                match value {
                    Value::Null => {}
                    Value::String(value) => strings.push(value.clone()),
                    Value::Number(value) => strings.push(value.to_string()),
                    Value::Bool(value) => strings.push(value.to_string()),
                    Value::Array(_) | Value::Object(_) => {
                        diagnostics.push(runtime_error(
                            "field_type_mismatch",
                            "Field array values must resolve to strings, numbers, booleans, or null",
                            path,
                            strategy_key,
                            json!({ "itemIndex": item_index, "valueIndex": value_index }),
                        ));
                        return JsonStringsResult {
                            values: Vec::new(),
                            failed: true,
                        };
                    }
                }
            }
            JsonStringsResult {
                values: strings,
                failed: false,
            }
        }
        Value::Object(_) => {
            diagnostics.push(runtime_error(
                "field_type_mismatch",
                "Field value must resolve to a string, number, boolean, null, or an array of scalar values",
                path,
                strategy_key,
                json!({ "itemIndex": item_index }),
            ));
            JsonStringsResult {
                values: Vec::new(),
                failed: true,
            }
        }
    }
}
