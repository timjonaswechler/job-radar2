use serde::{Deserialize, Serialize};

use super::values::{compile_field_expression, compile_list_field_expression, compile_predicates};
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::detail::{DetailExtraction, DetailStep, DetailStrategy};
use crate::profile_dsl::documents::strategy::Acceptance;
use crate::profile_dsl::documents::PhaseLimits;
use crate::profile_dsl::policy::StrategyPolicy;
use crate::profile_dsl::primitives::capture::{
    compile_captures, CaptureCompileError, CompiledCapturePlan,
};
use crate::profile_dsl::primitives::parse::{compile_parse, CompiledParse, ParseInputKind};
use crate::profile_dsl::primitives::predicate::{
    compile_predicate, CompiledPredicate, PredicateCompileContext, PredicateCompileError,
    PredicatePlacement,
};
use crate::profile_dsl::primitives::select::{
    compile_select, CompiledSelect, SelectCompileContext, SelectPhase, SelectPlacement,
};
use crate::profile_dsl::primitives::value::{
    CompiledListValue, CompiledValue, ValueCompileContext, ValueCompileError, ValuePlacement,
};
use crate::profile_dsl::template::{
    json_pointer_segment, TemplateAdmissionKeys, TemplatePlacement,
};

use super::capabilities::{compile_fetch, ExecutionPlanBuildError, ExecutionPlanFetch};

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
    pub parse: CompiledParse,
    pub select: CompiledSelect,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<CompiledPredicate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<CompiledCapturePlan>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub field_match: Option<CompiledPredicate>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<CompiledValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<CompiledValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<CompiledListValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<CompiledValue>,
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
    let field_context = ValueCompileContext {
        placement: ValuePlacement::DetailMatchFilterOutput,
        document_type: Some(strategy.parse.parse_type()),
        source_config_keys: keys.source_config.clone(),
        posting_meta_keys: keys.posting_meta.clone(),
        capture_keys: keys.captures.clone(),
    };
    let capture_context = ValueCompileContext {
        placement: ValuePlacement::DetailCaptureSource,
        document_type: None,
        source_config_keys: keys.source_config.clone(),
        posting_meta_keys: keys.posting_meta.clone(),
        capture_keys: Default::default(),
    };
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
        parse: compile_parse(
            &strategy.parse,
            match strategy.fetch {
                crate::profile_dsl::documents::Fetch::Http { .. } => ParseInputKind::DecodedHttp,
                crate::profile_dsl::documents::Fetch::Browser { .. } => {
                    ParseInputKind::BrowserRendered
                }
            },
        )
        .map_err(|error| ExecutionPlanBuildError::new(format!("{path}/parse"), error.message))?,
        select: compile_select(
            &strategy.select,
            SelectCompileContext {
                document_type: strategy.parse.parse_type(),
                phase: SelectPhase::Detail,
                placement: SelectPlacement::Strategy,
            },
        )
        .map_err(|error| ExecutionPlanBuildError::new(format!("{path}/select"), error.message))?,
        conditions: compile_predicates(
            strategy.conditions.as_deref(),
            &PredicateCompileContext {
                placement: PredicatePlacement::Where,
                value: field_context.clone(),
            },
        )
        .map_err(|error| predicate_error(format!("{path}/where/{}", error.index), error.source))?,

        captures: strategy
            .captures
            .as_ref()
            .map(|captures| compile_captures(captures, &capture_context))
            .transpose()
            .map_err(|error| capture_error(format!("{path}/captures"), error))?,
        field_match: strategy
            .field_match
            .as_ref()
            .map(|predicate| {
                compile_predicate(
                    predicate,
                    &PredicateCompileContext {
                        placement: PredicatePlacement::DetailMatch,
                        value: field_context.clone(),
                    },
                )
                .map_err(|error| predicate_error(format!("{path}/match"), error))
            })
            .transpose()?,

        extract: compile_detail_extraction(&strategy.extract, &field_context, path)?,
        accept_when: strategy.accept_when.clone(),
        diagnostics: strategy.diagnostics.clone(),
    })
}

fn capture_error(path: String, error: CaptureCompileError) -> ExecutionPlanBuildError {
    ExecutionPlanBuildError::new(
        format!(
            "{path}/{}{}",
            json_pointer_segment(&error.capture_key),
            error.path
        ),
        error.message,
    )
}

fn field_expression_error(path: String, error: ValueCompileError) -> ExecutionPlanBuildError {
    ExecutionPlanBuildError::new(format!("{path}{}", error.path), error.message)
}

fn predicate_error(path: String, error: PredicateCompileError) -> ExecutionPlanBuildError {
    ExecutionPlanBuildError::new(format!("{path}{}", error.path), error.message)
}

fn compile_detail_extraction(
    extraction: &DetailExtraction,
    context: &ValueCompileContext,
    path: &str,
) -> Result<ExecutionPlanDetailExtraction, ExecutionPlanBuildError> {
    if extraction.fields.title.is_none()
        && extraction.fields.company.is_none()
        && extraction.fields.locations.is_none()
        && extraction.fields.description_text.is_none()
    {
        return Err(ExecutionPlanBuildError::new(
            format!("{path}/extract/fields"),
            "Detail extraction must define at least one canonical field",
        ));
    }
    let compile_optional = |expression: Option<
        &crate::profile_dsl::documents::extract::FieldExpression,
    >,
                            name: &str| {
        expression
            .map(|expression| compile_field_expression(expression, context))
            .transpose()
            .map_err(|error| field_expression_error(format!("{path}/extract/fields/{name}"), error))
    };
    Ok(ExecutionPlanDetailExtraction {
        fields: ExecutionPlanDetailFields {
            title: compile_optional(extraction.fields.title.as_ref(), "title")?,
            company: compile_optional(extraction.fields.company.as_ref(), "company")?,
            locations: extraction
                .fields
                .locations
                .as_ref()
                .map(|expression| compile_list_field_expression(expression, context))
                .transpose()
                .map_err(|error| {
                    field_expression_error(format!("{path}/extract/fields/locations"), error)
                })?,
            description_text: compile_optional(
                extraction.fields.description_text.as_ref(),
                "descriptionText",
            )?,
        },
    })
}
