use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::extract::FieldExpression;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Acceptance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_description_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_results: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_error_ratio: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldMatch {
    pub left: FieldExpression,
    pub right: FieldExpression,
}
