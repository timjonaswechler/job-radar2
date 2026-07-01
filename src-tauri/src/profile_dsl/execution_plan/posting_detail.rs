use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::extract::FieldExpression;
use crate::profile_dsl::documents::posting_detail::{
    PostingDetailExtraction, PostingDetailStep, PostingDetailStrategy,
};
use crate::profile_dsl::documents::select::{Captures, Filter, Select};
use crate::profile_dsl::documents::strategy::{Acceptance, FieldMatch};
use crate::profile_dsl::documents::Parse;

use super::capabilities::{
    clone_parse, clone_select, compile_fetch, ExecutionPlanBuildError, ExecutionPlanFetch,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanPostingDetailStep {
    pub strategies: Vec<ExecutionPlanPostingDetailStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanPostingDetailStrategy {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fetch: ExecutionPlanFetch,
    pub parse: Parse,
    pub select: Select,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Filter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Captures>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub field_match: Option<FieldMatch>,
    pub extract: ExecutionPlanPostingDetailExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanPostingDetailExtraction {
    pub fields: ExecutionPlanPostingDetailFields,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanPostingDetailFields {
    pub description_text: FieldExpression,
}

pub(crate) fn compile_posting_detail_step(
    step: &PostingDetailStep,
    path: &str,
) -> Result<ExecutionPlanPostingDetailStep, ExecutionPlanBuildError> {
    Ok(ExecutionPlanPostingDetailStep {
        strategies: step
            .strategies
            .iter()
            .enumerate()
            .map(|(index, strategy)| {
                compile_posting_detail_strategy(strategy, &format!("{path}/strategies/{index}"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        accept_when: step.accept_when.clone(),
    })
}

fn compile_posting_detail_strategy(
    strategy: &PostingDetailStrategy,
    path: &str,
) -> Result<ExecutionPlanPostingDetailStrategy, ExecutionPlanBuildError> {
    Ok(ExecutionPlanPostingDetailStrategy {
        key: strategy.key.clone(),
        description: strategy.description.clone(),
        fetch: compile_fetch(&strategy.fetch, &format!("{path}/fetch"))?,
        parse: clone_parse(&strategy.parse),
        select: clone_select(&strategy.select),
        conditions: strategy.conditions.clone(),
        captures: strategy.captures.clone(),
        field_match: strategy.field_match.clone(),
        extract: compile_posting_detail_extraction(&strategy.extract),
        accept_when: strategy.accept_when.clone(),
        diagnostics: strategy.diagnostics.clone(),
    })
}

fn compile_posting_detail_extraction(
    extraction: &PostingDetailExtraction,
) -> ExecutionPlanPostingDetailExtraction {
    ExecutionPlanPostingDetailExtraction {
        fields: ExecutionPlanPostingDetailFields {
            description_text: extraction.fields.description_text.clone(),
        },
    }
}
