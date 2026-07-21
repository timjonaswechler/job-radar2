use super::TransformDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Trim {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrimPlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "trim" };
pub(super) const fn compile(_: &Trim) -> TrimPlan {
    TrimPlan
}
pub(super) fn execute(_: &TrimPlan, value: String) -> String {
    value.trim().to_string()
}
