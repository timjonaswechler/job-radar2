use super::{normalize_whitespace, TransformDescriptor};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlugToTitle {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlugToTitlePlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor {
    key: "slug_to_title",
};
pub(super) const fn compile(_: &SlugToTitle) -> SlugToTitlePlan {
    SlugToTitlePlan
}
pub(super) fn execute(_: &SlugToTitlePlan, value: String) -> String {
    let words = normalize_whitespace::normalize(&value.replace(['-', '_'], " "));
    words
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
