use super::{
    normalize_whitespace, type_mismatch, TransformDescriptor, TransformError, TransformShape,
    TransformValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToStringTransform {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToStringTransformPlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "to_string" };
pub(super) const fn compile(_: &ToStringTransform) -> ToStringTransformPlan {
    ToStringTransformPlan
}
pub(super) fn execute<'doc, 'body>(
    _: &ToStringTransformPlan,
    shape: TransformShape<'doc, 'body>,
    transform_index: usize,
) -> Result<TransformShape<'doc, 'body>, TransformError> {
    let scalar = matches!(shape, TransformShape::Scalar(_));
    let mut output = Vec::new();
    for (index, value) in shape.into_values().into_iter().enumerate() {
        let text = match value {
            TransformValue::Text(value) => value,
            TransformValue::Json(Value::String(value)) => value,
            TransformValue::Json(Value::Number(value)) => value.to_string(),
            TransformValue::Json(Value::Bool(value)) => value.to_string(),
            TransformValue::Json(Value::Null | Value::Array(_) | Value::Object(_)) => {
                return Err(type_mismatch(transform_index, Some(index)))
            }
            TransformValue::Xml(node) => node
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>(),
            TransformValue::Html(node) => {
                normalize_whitespace::normalize(&node.formatted_text().to_string())
            }
        };
        output.push(TransformValue::Text(text));
    }
    Ok(if scalar {
        TransformShape::Scalar(output.pop().expect("scalar transform has one value"))
    } else {
        TransformShape::Sequence(output)
    })
}
