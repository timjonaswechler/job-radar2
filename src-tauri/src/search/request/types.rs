use serde::{Deserialize, Serialize};

use crate::search::run::SearchRunStatus;

use super::{SearchRequestStatus, SearchRule, SearchRuleInput};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationIssueCode {
    InvalidRegex,
    IncludeRuleRequired,
    SourceKeyRequired,
    DuplicateSourceKey,
    IssuesTruncated,
}

impl ValidationIssueCode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRegex => "invalid_regex",
            Self::IncludeRuleRequired => "include_rule_required",
            Self::SourceKeyRequired => "source_key_required",
            Self::DuplicateSourceKey => "duplicate_source_key",
            Self::IssuesTruncated => "issues_truncated",
        }
    }
}

impl std::fmt::Display for ValidationIssueCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: ValidationIssueCode,
    pub path: String,
    pub message: String,
}

impl ValidationIssue {
    pub(super) fn new(
        code: ValidationIssueCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

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
    pub validation_issues: Vec<ValidationIssue>,
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
