use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::extract::FieldExpression;
use crate::profile_dsl::documents::fetch::Fetch;
use crate::profile_dsl::documents::select::Select;
use crate::profile_dsl::documents::strategy::Acceptance;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_overrides: Option<Vec<StrategyOverride>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategyOverride {
    pub step: OverridableStep,
    pub strategy_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch: Option<Fetch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<Select>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract: Option<BTreeMap<String, FieldExpression>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OverridableStep {
    #[serde(rename = "postingDiscovery")]
    PostingDiscovery,
    #[serde(rename = "postingDetail")]
    PostingDetail,
}
