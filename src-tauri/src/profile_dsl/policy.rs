//! Dormant final Strategy Policy documents and compiled plan boundary.
//!
//! Complete authored and compiled Discovery/Detail Strategy Sets always carry
//! one closed, typed [`StrategyPolicy`]. Partial direct-Source fragments may
//! omit the field only to inherit it from a complete base Strategy Set. These
//! types are intentionally disconnected from schema-v2 registry loading and
//! productive callers until A01 activates the final Source format.

use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    Acceptance, JsonObject, JsonSchemaObject, PostingDetailStep, PostingDetailStrategy,
    PostingDiscoveryStep, PostingDiscoveryStrategy, SupportMetadata,
};
use crate::profile_dsl::execution_plan::posting_detail::ExecutionPlanPostingDetailStep;
use crate::profile_dsl::execution_plan::posting_discovery::ExecutionPlanPostingDiscoveryStep;
use crate::profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
use crate::source::documents::{SourceConfig, SourceStatus};
use crate::source_profile::documents::{
    ProfileDetectionDocument, SourceProfileDocument, SourceProfileKind,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyPolicy {
    FirstAccepted,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyPostingDiscoveryStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<PostingDiscoveryStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyPostingDetailStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<PostingDetailStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

impl PolicyPostingDiscoveryStep {
    pub(crate) fn legacy(&self) -> PostingDiscoveryStep {
        PostingDiscoveryStep {
            strategies: self.strategies.clone(),
            accept_when: self.accept_when.clone(),
        }
    }
}

impl PolicyPostingDetailStep {
    pub(crate) fn legacy(&self) -> PostingDetailStep {
        PostingDetailStep {
            strategies: self.strategies.clone(),
            accept_when: self.accept_when.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyReusableAccessPathDocument {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_issues: Option<Vec<crate::profile_dsl::documents::SupportNote>>,
    pub posting_discovery: PolicyPostingDiscoveryStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<PolicyPostingDetailStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicySourceProfileDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub kind: SourceProfileKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub support: SupportMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect: Option<ProfileDetectionDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    pub access_paths: Vec<PolicyReusableAccessPathDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

impl PolicySourceProfileDocument {
    pub(crate) fn legacy(&self) -> SourceProfileDocument {
        SourceProfileDocument {
            schema_version: self.schema_version,
            key: self.key.clone(),
            name: self.name.clone(),
            kind: self.kind,
            description: self.description.clone(),
            support: self.support.clone(),
            detect: self.detect.clone(),
            source_config_schema: self.source_config_schema.clone(),
            access_paths: self
                .access_paths
                .iter()
                .map(
                    |path| crate::profile_dsl::documents::ReusableAccessPathDocument {
                        key: path.key.clone(),
                        name: path.name.clone(),
                        description: path.description.clone(),
                        source_config_schema: path.source_config_schema.clone(),
                        known_issues: path.known_issues.clone(),
                        posting_discovery: path.posting_discovery.legacy(),
                        posting_detail: path
                            .posting_detail
                            .as_ref()
                            .map(PolicyPostingDetailStep::legacy),
                        diagnostics: path.diagnostics.clone(),
                    },
                )
                .collect(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyPostingDiscoveryStepFragment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<StrategyPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategies: Option<Vec<crate::profile_dsl::documents::PostingDiscoveryStrategyFragment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyPostingDetailStepFragment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<StrategyPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategies: Option<Vec<crate::profile_dsl::documents::PostingDetailStrategyFragment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyAccessPathFragment {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posting_discovery: Option<PolicyPostingDiscoveryStepFragment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<PolicyPostingDetailStepFragment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PolicyAccessPathFragmentInput {
    key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    source_config_schema: Option<JsonSchemaObject>,
    #[serde(default)]
    posting_discovery: Option<PolicyPostingDiscoveryStepFragment>,
    #[serde(default)]
    posting_detail: Option<PolicyPostingDetailStepFragment>,
}

impl<'de> Deserialize<'de> for PolicyAccessPathFragment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        crate::profile_dsl::documents::fragments::reject_structural_null(&value, &mut Vec::new())
            .map_err(D::Error::custom)?;
        if let Some((name, _)) = value
            .get("sourceConfigSchema")
            .and_then(|schema| schema.get("properties"))
            .and_then(serde_json::Value::as_object)
            .and_then(|properties| {
                properties
                    .iter()
                    .find(|(_, schema)| schema.get("title").is_some())
            })
        {
            return Err(D::Error::custom(format!(
                "title is not authorable in direct Source Config schema fragments at /sourceConfigSchema/properties/{name}/title"
            )));
        }
        let input: PolicyAccessPathFragmentInput =
            serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(Self {
            key: input.key,
            name: input.name,
            source_config_schema: input.source_config_schema,
            posting_discovery: input.posting_discovery,
            posting_detail: input.posting_detail,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicySelectedAccessPath {
    ProfileAccessPath {
        #[serde(rename = "profileKey")]
        profile_key: String,
        #[serde(rename = "pathKey")]
        path_key: String,
    },
    SourceOwnedAccessPath {
        key: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "sourceConfigSchema", skip_serializing_if = "Option::is_none")]
        source_config_schema: Option<JsonSchemaObject>,
        #[serde(rename = "postingDiscovery")]
        posting_discovery: PolicyPostingDiscoveryStep,
        #[serde(rename = "postingDetail", skip_serializing_if = "Option::is_none")]
        posting_detail: Option<PolicyPostingDetailStep>,
        #[serde(skip_serializing_if = "Option::is_none")]
        diagnostics: Option<Diagnostics>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicySourceDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub status: SourceStatus,
    pub source_config: JsonObject,
    pub selected_access_path: PolicySelectedAccessPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_paths: Option<Vec<PolicyAccessPathFragment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_support: Option<SupportMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PolicySourceProfileRegistrySnapshot {
    pub profiles: Vec<PolicySourceProfileDocument>,
    pub sources: Vec<PolicySourceDocument>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyExecutionPlanPostingDiscoveryStep {
    pub policy: StrategyPolicy,
    #[serde(flatten)]
    pub execution: ExecutionPlanPostingDiscoveryStep,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyExecutionPlanPostingDetailStep {
    pub policy: StrategyPolicy,
    #[serde(flatten)]
    pub execution: ExecutionPlanPostingDetailStep,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySourceExecutionPlan {
    pub source: ExecutionPlanSource,
    pub selected_access_path: ExecutionPlanAccessPath,
    pub source_config: SourceConfig,
    pub posting_discovery: PolicyExecutionPlanPostingDiscoveryStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<PolicyExecutionPlanPostingDetailStep>,
}

impl PolicySourceExecutionPlan {
    pub(crate) fn from_legacy(
        plan: SourceExecutionPlan,
        discovery_policy: StrategyPolicy,
        detail_policy: Option<StrategyPolicy>,
    ) -> Self {
        Self {
            source: plan.source,
            selected_access_path: plan.selected_access_path,
            source_config: plan.source_config,
            posting_discovery: PolicyExecutionPlanPostingDiscoveryStep {
                policy: discovery_policy,
                execution: plan.posting_discovery,
            },
            posting_detail: plan.posting_detail.map(|execution| {
                PolicyExecutionPlanPostingDetailStep {
                    policy: detail_policy.expect("compiled detail must retain its authored policy"),
                    execution,
                }
            }),
        }
    }

    pub(crate) fn legacy(&self) -> SourceExecutionPlan {
        SourceExecutionPlan {
            source: self.source.clone(),
            selected_access_path: self.selected_access_path.clone(),
            source_config: self.source_config.clone(),
            posting_discovery: self.posting_discovery.execution.clone(),
            posting_detail: self
                .posting_detail
                .as_ref()
                .map(|step| step.execution.clone()),
        }
    }
}
