use super::{type_mismatch, TransformDescriptor, TransformError, TransformShape, TransformValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub separator: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JoinPlan {
    separator: String,
}
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "join" };
pub(super) fn compile(value: &Join) -> JoinPlan {
    JoinPlan {
        separator: value.separator.clone(),
    }
}
pub(super) fn execute<'doc, 'body>(
    plan: &JoinPlan,
    shape: TransformShape<'doc, 'body>,
    transform_index: usize,
) -> Result<TransformShape<'doc, 'body>, TransformError> {
    let mut strings = Vec::new();
    for (index, value) in shape.into_values().into_iter().enumerate() {
        let TransformValue::Text(value) = value else {
            return Err(type_mismatch(transform_index, Some(index)));
        };
        strings.push(value);
    }
    Ok(TransformShape::Scalar(TransformValue::Text(
        strings.join(&plan.separator),
    )))
}
