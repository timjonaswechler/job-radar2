use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchRuleTarget {
    Title,
}

impl TryFrom<&str> for SearchRuleTarget {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "title" => Ok(Self::Title),
            _ => Err("must be title".to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchRuleKind {
    Text,
    Regex,
}

impl TryFrom<&str> for SearchRuleKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "text" => Ok(Self::Text),
            "regex" => Ok(Self::Regex),
            _ => Err("must be text or regex".to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRule {
    pub target: SearchRuleTarget,
    pub kind: SearchRuleKind,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRuleInput {
    pub target: String,
    pub kind: String,
    pub value: String,
}
