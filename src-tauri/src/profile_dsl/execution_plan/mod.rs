use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::SourceOverrides;
use crate::source::documents::SourceConfig;

pub(crate) mod capabilities;
pub(crate) mod posting_detail;
pub(crate) mod posting_discovery;

use posting_detail::ExecutionPlanPostingDetailStep;
use posting_discovery::ExecutionPlanPostingDiscoveryStep;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceExecutionPlan {
    pub source: ExecutionPlanSource,
    pub selected_access_path: ExecutionPlanAccessPath,
    pub source_config: SourceConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_overrides: Option<SourceOverrides>,
    pub posting_discovery: ExecutionPlanPostingDiscoveryStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<ExecutionPlanPostingDetailStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanSource {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanAccessPath {
    #[serde(rename_all = "camelCase")]
    ProfileAccessPath {
        profile_key: String,
        profile_name: String,
        path_key: String,
        path_name: String,
    },
    #[serde(rename_all = "camelCase")]
    SourceOwnedAccessPath { key: String, name: String },
}
