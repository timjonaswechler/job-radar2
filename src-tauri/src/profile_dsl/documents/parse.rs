use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parse {
    #[serde(rename = "type")]
    pub parse_type: ParseType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseType {
    Json,
    Xml,
    Html,
    Text,
}
