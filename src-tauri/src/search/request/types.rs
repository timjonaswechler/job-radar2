use serde::{Deserialize, Serialize};

use crate::search::run::SearchRunStatus;

use super::{SearchRequestStatus, SearchRule, SearchRuleInput};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub id: i64,
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRule>,
    pub exclude_rules: Vec<SearchRule>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_keys: Vec<String>,
    pub validation_error: Option<String>,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<SearchRunStatus>,
    pub last_run_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSearchRequestInput {
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRuleInput>,
    pub exclude_rules: Vec<SearchRuleInput>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSearchRequestInput {
    pub status: SearchRequestStatus,
    pub include_rules: Vec<SearchRuleInput>,
    pub exclude_rules: Vec<SearchRuleInput>,
    pub locations: Vec<String>,
    pub radius_km: Option<i64>,
    pub source_keys: Vec<String>,
}
