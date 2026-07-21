use serde::{Deserialize, Serialize};

use super::values::{compile_field_expression, compile_list_field_expression, compile_predicates};
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::discovery::{
    DiscoveryExtraction, DiscoveryStep, DiscoveryStrategy,
};
use crate::profile_dsl::documents::strategy::Acceptance;
use crate::profile_dsl::documents::PhaseLimits;
use crate::profile_dsl::occurrence::HintUse;
use crate::profile_dsl::policy::StrategyPolicy;
use crate::profile_dsl::primitives::capture::{
    compile_captures, CaptureCompileError, CompiledCapturePlan,
};
use crate::profile_dsl::primitives::parse::{compile_parse, CompiledParse, ParseInputKind};
use crate::profile_dsl::primitives::predicate::{
    CompiledPredicate, PredicateCompileContext, PredicateCompileError, PredicatePlacement,
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

use super::capabilities::{
    compile_fetch, compile_pagination, ExecutionPlanBuildError, ExecutionPlanFetch,
    ExecutionPlanPagination,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<ExecutionPlanDiscoveryStrategy>,
    pub limits: PhaseLimits,
    pub limits_authored: bool,
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
    pub parse: CompiledParse,
    pub select: CompiledSelect,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<CompiledPredicate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<CompiledCapturePlan>,
    pub extract: ExecutionPlanDiscoveryExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryExtraction {
    pub output: ExecutionPlanDiscoveryOutput,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryOutput {
    pub reference: ExecutionPlanDiscoveryReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_values: Option<ExecutionPlanProviderValues>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<std::collections::BTreeMap<String, ExecutionPlanDiscoveryHint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<std::collections::BTreeMap<String, CompiledValue>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryReference {
    pub url: CompiledValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_posting_id: Option<CompiledValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanProviderValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<CompiledValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<CompiledValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<CompiledListValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<CompiledValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanDiscoveryHint {
    pub value: CompiledValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint_use: Option<HintUse>,
}

pub(crate) fn compile_discovery_step(
    step: &DiscoveryStep,
    path: &str,
    source_config_keys: &[String],
) -> Result<ExecutionPlanDiscoveryStep, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDiscoveryStep {
        policy: step.policy,
        strategies: step
            .strategies
            .iter()
            .enumerate()
            .map(|(index, strategy)| {
                compile_discovery_strategy(
                    strategy,
                    &format!("{path}/strategies/{index}"),
                    source_config_keys,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        limits: step.limits.unwrap_or(PhaseLimits::BACKEND),
        limits_authored: step.limits.is_some(),
        accept_when: step.accept_when.clone(),
    })
}

fn compile_discovery_strategy(
    strategy: &DiscoveryStrategy,
    path: &str,
    source_config_keys: &[String],
) -> Result<ExecutionPlanDiscoveryStrategy, ExecutionPlanBuildError> {
    let keys = TemplateAdmissionKeys {
        source_config: source_config_keys.iter().cloned().collect(),
        captures: strategy
            .captures
            .as_ref()
            .into_iter()
            .flat_map(|values| values.keys().cloned())
            .collect(),
        posting_meta: Default::default(),
    };
    let field_context = ValueCompileContext {
        placement: ValuePlacement::DiscoveryFilterOutput,
        document_type: Some(strategy.parse.parse_type()),
        source_config_keys: keys.source_config.clone(),
        posting_meta_keys: Default::default(),
        capture_keys: keys.captures.clone(),
    };
    let capture_context = ValueCompileContext {
        placement: ValuePlacement::DiscoveryCaptureSource,
        document_type: Some(strategy.parse.parse_type()),
        source_config_keys: keys.source_config.clone(),
        posting_meta_keys: Default::default(),
        capture_keys: Default::default(),
    };
    Ok(ExecutionPlanDiscoveryStrategy {
        key: strategy.key.clone(),
        description: strategy.description.clone(),
        fetch: compile_fetch(
            &strategy.fetch,
            &format!("{path}/fetch"),
            match strategy.fetch {
                crate::profile_dsl::documents::Fetch::Http { .. } => {
                    TemplatePlacement::DiscoveryHttpUrl
                }
                crate::profile_dsl::documents::Fetch::Browser { .. } => {
                    TemplatePlacement::DiscoveryBrowserUrl
                }
            },
            &keys,
        )?,
        pagination: strategy
            .pagination
            .as_ref()
            .map(|pagination| {
                compile_pagination(
                    pagination,
                    &format!("{path}/pagination"),
                    strategy.parse.parse_type(),
                )
            })
            .transpose()?,
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
                phase: SelectPhase::Discovery,
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
        extract: compile_discovery_extraction(&strategy.extract, &field_context, path)?,
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

fn compile_discovery_extraction(
    extraction: &DiscoveryExtraction,
    context: &ValueCompileContext,
    path: &str,
) -> Result<ExecutionPlanDiscoveryExtraction, ExecutionPlanBuildError> {
    let compile_optional = |value: Option<&crate::profile_dsl::documents::FieldExpression>,
                            field_path: String| {
        value
            .map(|value| {
                compile_field_expression(value, context)
                    .map_err(|error| field_expression_error(field_path, error))
            })
            .transpose()
    };
    let compile_map = |values: Option<
        &std::collections::BTreeMap<String, crate::profile_dsl::documents::FieldExpression>,
    >,
                       section: &str| {
        values
            .map(|values| {
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.clone(),
                            compile_field_expression(value, context).map_err(|error| {
                                field_expression_error(
                                    format!(
                                        "{path}/extract/{section}/{}",
                                        json_pointer_segment(key)
                                    ),
                                    error,
                                )
                            })?,
                        ))
                    })
                    .collect::<Result<_, ExecutionPlanBuildError>>()
            })
            .transpose()
    };
    let provider_values = extraction.provider_values.as_ref();
    Ok(ExecutionPlanDiscoveryExtraction {
        output: ExecutionPlanDiscoveryOutput {
            reference: ExecutionPlanDiscoveryReference {
                url: compile_field_expression(&extraction.reference.url, context).map_err(
                    |error| field_expression_error(format!("{path}/extract/reference/url"), error),
                )?,
                provider_posting_id: compile_optional(
                    extraction.reference.provider_posting_id.as_ref(),
                    format!("{path}/extract/reference/providerPostingId"),
                )?,
            },
            provider_values: provider_values
                .map(|values| {
                    Ok(ExecutionPlanProviderValues {
                        title: compile_optional(
                            values.title.as_ref(),
                            format!("{path}/extract/providerValues/title"),
                        )?,
                        company: compile_optional(
                            values.company.as_ref(),
                            format!("{path}/extract/providerValues/company"),
                        )?,
                        locations: values
                            .locations
                            .as_ref()
                            .map(|value| {
                                compile_list_field_expression(value, context).map_err(|error| {
                                    field_expression_error(
                                        format!("{path}/extract/providerValues/locations"),
                                        error,
                                    )
                                })
                            })
                            .transpose()?,
                        description_text: compile_optional(
                            values.description_text.as_ref(),
                            format!("{path}/extract/providerValues/descriptionText"),
                        )?,
                    })
                })
                .transpose()?,
            hints: extraction
                .hints
                .as_ref()
                .map(|hints| {
                    hints
                        .iter()
                        .map(|(key, hint)| {
                            Ok((
                                key.clone(),
                                ExecutionPlanDiscoveryHint {
                                    value: compile_field_expression(&hint.value, context).map_err(
                                        |error| {
                                            field_expression_error(
                                                format!(
                                                    "{path}/extract/hints/{}/value",
                                                    json_pointer_segment(key)
                                                ),
                                                error,
                                            )
                                        },
                                    )?,
                                    hint_use: hint.hint_use,
                                },
                            ))
                        })
                        .collect::<Result<_, ExecutionPlanBuildError>>()
                })
                .transpose()?,
            posting_meta: compile_map(extraction.posting_meta.as_ref(), "postingMeta")?,
        },
    })
}
