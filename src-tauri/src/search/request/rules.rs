use serde::{Deserialize, Serialize};

pub use search_resolution::{SearchRule, SearchRuleKind, SearchRuleTarget};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRuleInput {
    pub target: String,
    pub kind: String,
    pub value: String,
}
