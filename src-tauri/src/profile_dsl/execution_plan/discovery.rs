use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::discovery::{
    DiscoveryExtraction, DiscoveryStep, DiscoveryStrategy,
};
use crate::profile_dsl::documents::extract::{FieldExpression, ListFieldExpression};
use crate::profile_dsl::documents::select::{Captures, Filter, Select};
use crate::profile_dsl::documents::strategy::Acceptance;
use crate::profile_dsl::documents::Parse;
use crate::profile_dsl::policy::StrategyPolicy;

use super::capabilities::{
    clone_parse, clone_select, compile_fetch, compile_pagination, ExecutionPlanBuildError,
    ExecutionPlanFetch, ExecutionPlanPagination,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<ExecutionPlanDiscoveryStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryStrategy {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fetch: ExecutionPlanFetch,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<ExecutionPlanPagination>,
    pub parse: Parse,
    pub select: Select,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Filter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Captures>,
    pub extract: ExecutionPlanDiscoveryExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryExtraction {
    pub fields: ExecutionPlanDiscoveryFields,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryFields {
    pub title: FieldExpression,
    pub company: FieldExpression,
    pub url: FieldExpression,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<ListFieldExpression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<std::collections::BTreeMap<String, FieldExpression>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<FieldExpression>,
}

pub(crate) fn compile_discovery_step(
    step: &DiscoveryStep,
    path: &str,
) -> Result<ExecutionPlanDiscoveryStep, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDiscoveryStep {
        policy: step.policy,
        strategies: step
            .strategies
            .iter()
            .enumerate()
            .map(|(index, strategy)| {
                compile_discovery_strategy(strategy, &format!("{path}/strategies/{index}"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        accept_when: step.accept_when.clone(),
    })
}

fn compile_discovery_strategy(
    strategy: &DiscoveryStrategy,
    path: &str,
) -> Result<ExecutionPlanDiscoveryStrategy, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDiscoveryStrategy {
        key: strategy.key.clone(),
        description: strategy.description.clone(),
        fetch: compile_fetch(&strategy.fetch, &format!("{path}/fetch"))?,
        pagination: strategy
            .pagination
            .as_ref()
            .map(|pagination| compile_pagination(pagination, &format!("{path}/pagination")))
            .transpose()?,
        parse: clone_parse(&strategy.parse),
        select: clone_select(&strategy.select),
        conditions: strategy.conditions.clone(),
        captures: strategy.captures.clone(),
        extract: compile_discovery_extraction(&strategy.extract),
        accept_when: strategy.accept_when.clone(),
        diagnostics: strategy.diagnostics.clone(),
    })
}

fn compile_discovery_extraction(
    extraction: &DiscoveryExtraction,
) -> ExecutionPlanDiscoveryExtraction {
    ExecutionPlanDiscoveryExtraction {
        fields: ExecutionPlanDiscoveryFields {
            title: extraction.fields.title.clone(),
            company: extraction.fields.company.clone(),
            url: extraction.fields.url.clone(),
            locations: extraction.fields.locations.clone(),
            posting_meta: extraction.fields.posting_meta.clone(),
            description_text: extraction.fields.description_text.clone(),
        },
    }
}
