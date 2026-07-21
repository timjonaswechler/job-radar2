use super::TransformDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NormalizeWhitespace {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NormalizeWhitespacePlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor {
    key: "normalize_whitespace",
};
pub(super) const fn compile(_: &NormalizeWhitespace) -> NormalizeWhitespacePlan {
    NormalizeWhitespacePlan
}
pub(super) fn execute(_: &NormalizeWhitespacePlan, value: String) -> String {
    normalize(&value)
}
pub(crate) fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
