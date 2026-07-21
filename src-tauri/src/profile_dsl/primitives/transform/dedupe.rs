use super::{type_mismatch, TransformDescriptor, TransformError, TransformShape, TransformValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dedupe {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DedupePlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "dedupe" };
pub(super) const fn compile(_: &Dedupe) -> DedupePlan {
    DedupePlan
}
pub(super) fn execute<'doc, 'body>(
    _: &DedupePlan,
    shape: TransformShape<'doc, 'body>,
    transform_index: usize,
) -> Result<TransformShape<'doc, 'body>, TransformError> {
    let scalar = matches!(shape, TransformShape::Scalar(_));
    let mut output = Vec::new();
    for (index, value) in shape.into_values().into_iter().enumerate() {
        let TransformValue::Text(value) = value else {
            return Err(type_mismatch(transform_index, Some(index)));
        };
        if !output.iter().any(
            |existing| matches!(existing, TransformValue::Text(existing) if existing == &value),
        ) {
            output.push(TransformValue::Text(value));
        }
    }
    Ok(if scalar {
        TransformShape::Scalar(output.pop().expect("scalar transform has one value"))
    } else {
        TransformShape::Sequence(output)
    })
}
