use serde::{Deserialize, Serialize};

use crate::source::documents::SourceConfig;

pub(crate) mod capabilities;
pub(crate) mod detail;
pub(crate) mod discovery;
pub(crate) mod values;

use detail::ExecutionPlanDetailStep;
use discovery::ExecutionPlanDiscoveryStep;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceExecutionPlan {
    pub source: ExecutionPlanSource,
    pub selected_access_path: ExecutionPlanAccessPath,
    pub source_config: SourceConfig,
    pub discovery: ExecutionPlanDiscoveryStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ExecutionPlanDetailStep>,
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
