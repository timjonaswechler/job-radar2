use serde::{Deserialize, Serialize};

use super::values::{
    compile_captures, compile_field_expression, compile_field_match, compile_filters,
    ExecutionPlanCaptures, ExecutionPlanFieldExpression, ExecutionPlanFieldMatch,
    ExecutionPlanFilter,
};
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::detail::{DetailExtraction, DetailStep, DetailStrategy};
use crate::profile_dsl::documents::select::Select;
use crate::profile_dsl::documents::strategy::Acceptance;
use crate::profile_dsl::documents::Parse;
use crate::profile_dsl::documents::PhaseLimits;
use crate::profile_dsl::policy::StrategyPolicy;
use crate::profile_dsl::template::{
    descriptor_for_placement, TemplateAdmissionKeys, TemplateDescriptor, TemplatePlacement,
};

use super::capabilities::{
    clone_parse, clone_select, compile_fetch, ExecutionPlanBuildError, ExecutionPlanFetch,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDetailStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<ExecutionPlanDetailStrategy>,
    pub limits: PhaseLimits,
    pub limits_authored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDetailStrategy {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fetch: ExecutionPlanFetch,
    pub parse: Parse,
    pub select: Select,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<ExecutionPlanFilter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<ExecutionPlanCaptures>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub field_match: Option<ExecutionPlanFieldMatch>,
    pub extract: ExecutionPlanDetailExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDetailExtraction {
    pub fields: ExecutionPlanDetailFields,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDetailFields {
    pub description_text: ExecutionPlanFieldExpression,
}

pub(crate) fn compile_detail_step(
    step: &DetailStep,
    path: &str,
    source_config_keys: &[String],
    posting_meta_keys: &[String],
) -> Result<ExecutionPlanDetailStep, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDetailStep {
        policy: step.policy,
        strategies: step
            .strategies
            .iter()
            .enumerate()
            .map(|(index, strategy)| {
                compile_detail_strategy(
                    strategy,
                    &format!("{path}/strategies/{index}"),
                    source_config_keys,
                    posting_meta_keys,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        limits: step.limits.unwrap_or(PhaseLimits::BACKEND),
        limits_authored: step.limits.is_some(),
        accept_when: step.accept_when.clone(),
    })
}

fn compile_detail_strategy(
    strategy: &DetailStrategy,
    path: &str,
    source_config_keys: &[String],
    posting_meta_keys: &[String],
) -> Result<ExecutionPlanDetailStrategy, ExecutionPlanBuildError> {
    let keys = TemplateAdmissionKeys {
        source_config: source_config_keys.iter().cloned().collect(),
        captures: strategy
            .captures
            .as_ref()
            .into_iter()
            .flat_map(|values| values.keys().cloned())
            .collect(),
        posting_meta: posting_meta_keys.iter().cloned().collect(),
    };
    let field_descriptor = descriptor_for_placement(TemplatePlacement::DetailValue, &keys);
    Ok(ExecutionPlanDetailStrategy {
        key: strategy.key.clone(),
        description: strategy.description.clone(),
        fetch: compile_fetch(
            &strategy.fetch,
            &format!("{path}/fetch"),
            match strategy.fetch {
                crate::profile_dsl::documents::Fetch::Http { .. } => {
                    TemplatePlacement::DetailHttpUrl
                }
                crate::profile_dsl::documents::Fetch::Browser { .. } => {
                    TemplatePlacement::DetailBrowserUrl
                }
            },
            &keys,
        )?,
        parse: clone_parse(&strategy.parse),
        select: clone_select(&strategy.select),
        conditions: compile_filters(strategy.conditions.as_ref(), &field_descriptor).map_err(
            |error| ExecutionPlanBuildError {
                path: format!("{path}/where"),
                message: error.to_string(),
            },
        )?,
        captures: compile_captures(strategy.captures.as_ref(), &field_descriptor).map_err(
            |error| ExecutionPlanBuildError {
                path: format!("{path}/captures"),
                message: error.to_string(),
            },
        )?,
        field_match: compile_field_match(strategy.field_match.as_ref(), &field_descriptor)
            .map_err(|error| ExecutionPlanBuildError {
                path: format!("{path}/match"),
                message: error.to_string(),
            })?,
        extract: compile_detail_extraction(&strategy.extract, &field_descriptor, path)?,
        accept_when: strategy.accept_when.clone(),
        diagnostics: strategy.diagnostics.clone(),
    })
}

fn compile_detail_extraction(
    extraction: &DetailExtraction,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<ExecutionPlanDetailExtraction, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDetailExtraction {
        fields: ExecutionPlanDetailFields {
            description_text: compile_field_expression(
                &extraction.fields.description_text,
                descriptor,
            )
            .map_err(|error| ExecutionPlanBuildError {
                path: format!("{path}/extract/fields/descriptionText"),
                message: error.to_string(),
            })?,
        },
    })
}
