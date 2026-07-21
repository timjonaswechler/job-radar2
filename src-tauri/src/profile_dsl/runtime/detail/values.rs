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
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) = select_relative_html(node, selector, path, strategy_key, diagnostics)
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
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) = select_relative_html(node, selector, path, strategy_key, diagnostics)
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
                json!({ "selector": selector, "error": format!("{error:?}") }),
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
        cardinality: CompiledCardinality,
        transforms: &CompiledTransformPipeline,
    ) -> RawFieldValues<'_> {
        RawFieldValues {
            values: self.values.into_iter().map(TransformValue::Text).collect(),
            failed: self.failed,
            cardinality,
            transforms,
        }
    }
}

pub(super) struct JsonTransformValuesResult {
    pub(super) values: Vec<TransformValue<'static, 'static>>,
}

impl JsonTransformValuesResult {
    pub(super) fn into_raw(
        self,
        cardinality: CompiledCardinality,
        transforms: &CompiledTransformPipeline,
    ) -> RawFieldValues<'_> {
        RawFieldValues {
            values: self.values,
            failed: false,
            cardinality,
            transforms,
        }
    }
}

pub(super) fn json_value_to_transform_values(value: &Value) -> JsonTransformValuesResult {
    let values = match value {
        Value::Array(values) => values.iter().cloned().map(TransformValue::Json).collect(),
        value => vec![TransformValue::Json(value.clone())],
    };
    JsonTransformValuesResult { values }
}
