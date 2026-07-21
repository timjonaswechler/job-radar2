use super::{type_mismatch, TransformDescriptor, TransformError, TransformShape, TransformValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Split {
    pub separator: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub trim_parts: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub drop_empty: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitPlan {
    separator: String,
    trim_parts: bool,
    drop_empty: bool,
}
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "split" };
pub(super) fn compile(value: &Split) -> Result<SplitPlan, String> {
    if value.separator.is_empty() {
        return Err("split transform separator must not be empty".to_string());
    }
    Ok(SplitPlan {
        separator: value.separator.clone(),
        trim_parts: value.trim_parts,
        drop_empty: value.drop_empty,
    })
}
pub(super) fn execute<'doc, 'body>(
    plan: &SplitPlan,
    shape: TransformShape<'doc, 'body>,
    transform_index: usize,
) -> Result<TransformShape<'doc, 'body>, TransformError> {
    let mut output = Vec::new();
    for (index, value) in shape.into_values().into_iter().enumerate() {
        let TransformValue::Text(value) = value else {
            return Err(type_mismatch(transform_index, Some(index)));
        };
        for part in value.split(&plan.separator) {
            let part = if plan.trim_parts { part.trim() } else { part };
            if !plan.drop_empty || !part.is_empty() {
                output.push(TransformValue::Text(part.to_string()));
            }
        }
    }
    Ok(TransformShape::Sequence(output))
}
