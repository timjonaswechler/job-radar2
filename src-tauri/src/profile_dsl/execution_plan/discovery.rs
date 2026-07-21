use serde::{Deserialize, Serialize};

use super::values::{
    compile_captures, compile_field_expression, compile_filters, compile_list_field_expression,
    ExecutionPlanCaptures, ExecutionPlanFieldExpression, ExecutionPlanFilter,
    ExecutionPlanListFieldExpression, FieldExpressionCompileError,
};
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::discovery::{
    DiscoveryExtraction, DiscoveryStep, DiscoveryStrategy,
};
use crate::profile_dsl::documents::strategy::Acceptance;
use crate::profile_dsl::documents::PhaseLimits;
use crate::profile_dsl::policy::StrategyPolicy;
use crate::profile_dsl::primitives::parse::{compile_parse, CompiledParse, ParseInputKind};
use crate::profile_dsl::primitives::select::{
    compile_select, CompiledSelect, SelectCompileContext, SelectPhase, SelectPlacement,
};
use crate::profile_dsl::template::{
    descriptor_for_placement, json_pointer_segment, TemplateAdmissionKeys, TemplateDescriptor,
    TemplatePlacement,
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
    pub conditions: Option<Vec<ExecutionPlanFilter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<ExecutionPlanCaptures>,
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
    pub title: ExecutionPlanFieldExpression,
    pub company: ExecutionPlanFieldExpression,
    pub url: ExecutionPlanFieldExpression,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<ExecutionPlanListFieldExpression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<std::collections::BTreeMap<String, ExecutionPlanFieldExpression>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<ExecutionPlanFieldExpression>,
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
    let field_descriptor = descriptor_for_placement(TemplatePlacement::DiscoveryValue, &keys);
    let capture_descriptor = descriptor_for_placement(
        TemplatePlacement::DiscoveryValue,
        &TemplateAdmissionKeys {
            source_config: keys.source_config.clone(),
            captures: Default::default(),
            posting_meta: Default::default(),
        },
    );
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
        conditions: compile_filters(strategy.conditions.as_ref(), &field_descriptor)
            .map_err(|error| field_expression_error(format!("{path}/where"), error))?,
        captures: compile_captures(strategy.captures.as_ref(), &capture_descriptor)
            .map_err(|error| field_expression_error(format!("{path}/captures"), error))?,
        extract: compile_discovery_extraction(&strategy.extract, &field_descriptor, path)?,
        accept_when: strategy.accept_when.clone(),
        diagnostics: strategy.diagnostics.clone(),
    })
}

fn field_expression_error(
    path: String,
    error: FieldExpressionCompileError,
) -> ExecutionPlanBuildError {
    match error {
        FieldExpressionCompileError::Template(error) => {
            ExecutionPlanBuildError::new(path, error.to_string())
        }
        FieldExpressionCompileError::Transform(error) => {
            ExecutionPlanBuildError::transform(path, error)
        }
    }
}

fn compile_discovery_extraction(
    extraction: &DiscoveryExtraction,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<ExecutionPlanDiscoveryExtraction, ExecutionPlanBuildError> {
    Ok(ExecutionPlanDiscoveryExtraction {
        fields: ExecutionPlanDiscoveryFields {
            title: compile_field_expression(&extraction.fields.title, descriptor).map_err(
                |error| field_expression_error(format!("{path}/extract/fields/title"), error),
            )?,
            company: compile_field_expression(&extraction.fields.company, descriptor).map_err(
                |error| field_expression_error(format!("{path}/extract/fields/company"), error),
            )?,
            url: compile_field_expression(&extraction.fields.url, descriptor).map_err(|error| {
                field_expression_error(format!("{path}/extract/fields/url"), error)
            })?,
            locations: extraction
                .fields
                .locations
                .as_ref()
                .map(|value| {
                    compile_list_field_expression(value, descriptor).map_err(|error| {
                        field_expression_error(format!("{path}/extract/fields/locations"), error)
                    })
                })
                .transpose()?,
            posting_meta: extraction
                .fields
                .posting_meta
                .as_ref()
                .map(|values| {
                    values
                        .iter()
                        .map(|(key, value)| {
                            Ok((
                                key.clone(),
                                compile_field_expression(value, descriptor).map_err(|error| {
                                    field_expression_error(
                                        format!(
                                            "{path}/extract/fields/postingMeta/{}",
                                            json_pointer_segment(key)
                                        ),
                                        error,
                                    )
                                })?,
                            ))
                        })
                        .collect::<Result<_, ExecutionPlanBuildError>>()
                })
                .transpose()?,
            description_text: extraction
                .fields
                .description_text
                .as_ref()
                .map(|value| {
                    compile_field_expression(value, descriptor).map_err(|error| {
                        field_expression_error(
                            format!("{path}/extract/fields/descriptionText"),
                            error,
                        )
                    })
                })
                .transpose()?,
        },
    })
}
