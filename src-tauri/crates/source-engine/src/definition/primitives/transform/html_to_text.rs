use super::{normalize_whitespace, TransformDescriptor};
use dom_query::Document as HtmlDocument;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HtmlToText {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HtmlToTextPlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor {
    key: "html_to_text",
};
pub(super) const fn compile(_: &HtmlToText) -> HtmlToTextPlan {
    HtmlToTextPlan
}
pub(super) fn execute(_: &HtmlToTextPlan, value: String) -> String {
    normalize_whitespace::normalize(&HtmlDocument::fragment(value).formatted_text().to_string())
}
