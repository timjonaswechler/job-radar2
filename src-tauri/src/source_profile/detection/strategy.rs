use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::{
    DetectionEvidenceKind, DetectionStrategy, DetectionUrlInput, PhaseLimits,
};
use crate::profile_dsl::execution_plan::capabilities::{
    compile_browser_fetch_with_descriptor, ExecutionPlanBrowserInteraction,
    ExecutionPlanBrowserWait, ExecutionPlanFetch,
};
use crate::profile_dsl::policy::StrategyPolicy;
use crate::profile_dsl::primitives::capture::{
    compile_named_pattern, evaluate_named_pattern, CompiledNamedPattern,
};
use crate::profile_dsl::primitives::fetch::http::{
    compile_http_fetch, execute_http_fetch, CompiledHttpFetch, HttpFetchExecutionError,
    HttpFetchOverlay, HttpStatusPolicy,
};
use crate::profile_dsl::primitives::predicate::{
    compile_regex, literal_contains, values_equal, CompiledRegex,
};
use crate::profile_dsl::runtime::allowance::{
    completion_for_stop, AllowanceStop, InvocationAllowance,
};
use crate::profile_dsl::runtime::browser_phase::{
    execute_canonical_browser_fetch, BrowserPhaseFetchInput, BrowserPhaseFetchProjection,
};
use crate::profile_dsl::runtime::cancellation::{
    CancellationOperation, RuntimePhase, TypedCancellation,
};
use crate::profile_dsl::runtime::strategy_set::{
    execute_strategy_set, policy_unsatisfied_diagnostic, StrategyAttemptCompletion,
    StrategyExecution, StrategySetTerminal,
};
use crate::profile_dsl::runtime::{
    BrowserAcquisition, BrowserAcquisitionFailureKind, PhaseBrowser, PhaseCompletion,
    PhaseExecutionReport, ProfileHttpClient, ProfileHttpFailureKind, RuntimeCancellation,
    RuntimeExecutionContext,
};
use crate::profile_dsl::template::{
    compile_template, render_template, CompiledTemplate, TemplateDescriptor, TemplateReference,
    TemplateValueView,
};
use crate::source_profile::documents::SourceProfileDocument;

use super::reconciliation::{
    aggregate_detection_attempts, DetectionAttempt, DetectionContribution,
    DetectionEvidenceContribution, DetectionOrigin, DetectionProfileContext,
    PreparedDetectionOutput, ReconciledDetectionRunResult, ReconciledDetectionState,
};

#[derive(Clone, Debug)]
pub struct CompiledDetectionPlan {
    profile_key: String,
    context: DetectionProfileContext,
    strategies: Vec<CompiledDetectionStrategy>,
    proposal_source_config: Option<BTreeMap<String, CompiledDetectionJsonValue>>,
    key_candidates: Vec<CompiledTemplate>,
    name_candidates: Vec<CompiledTemplate>,
}

#[derive(Clone, Debug)]
enum CompiledDetectionJsonValue {
    Template(CompiledTemplate),
    Array(Vec<CompiledDetectionJsonValue>),
    Object(BTreeMap<String, CompiledDetectionJsonValue>),
    Literal(serde_json::Value),
}

impl CompiledDetectionPlan {
    pub fn profile_key(&self) -> &str {
        &self.profile_key
    }
    pub fn strategy_keys(&self) -> impl Iterator<Item = &str> {
        self.strategies.iter().map(CompiledDetectionStrategy::key)
    }
}

#[derive(Clone, Debug)]
enum CompiledDetectionStrategy {
    Url {
        key: String,
        input: CompiledUrlInput,
    },
    Http {
        key: String,
        fetch: Box<CompiledHttpFetch>,
        expect_status: Option<u16>,
        contains: Option<String>,
        acceptance_regex: Option<CompiledRegex>,
        captures: Option<CompiledNamedPattern>,
        evidence: Option<String>,
    },
    Browser {
        key: String,
        url: CompiledTemplate,
        timeout_ms: u64,
        waits: Vec<ExecutionPlanBrowserWait>,
        interactions: Vec<ExecutionPlanBrowserInteraction>,
        contains: Option<String>,
        acceptance_regex: Option<CompiledRegex>,
        captures: Option<CompiledNamedPattern>,
        evidence: Option<String>,
    },
}

impl CompiledDetectionStrategy {
    fn key(&self) -> &str {
        match self {
            Self::Url { key, .. } | Self::Http { key, .. } | Self::Browser { key, .. } => key,
        }
    }
}

#[derive(Clone, Debug)]
enum CompiledUrlInput {
    PatternAlternatives(Vec<CompiledUrlAlternative>),
    AbsoluteUrl,
}

#[derive(Clone, Debug)]
struct CompiledUrlAlternative {
    pattern: CompiledNamedPattern,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionProfileRejectionKind {
    Url,
    Status,
    Contains,
    Regex,
    Capture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "kind", rename_all = "snake_case")]
pub enum DetectionProfileExecutionFailureKind {
    Acquisition(ProfileHttpFailureKind),
    BrowserAcquisition(DetectionBrowserFailureKind),
    BrowserInfrastructure,
    Render,
    Reconciliation,
    Proposal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionBrowserFailureKind {
    RuntimeLaunch,
    Navigation,
    Wait,
    Interaction,
    ContentRead,
    Deadline,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectionProfileCompletion {
    Matched,
    Unsupported,
    Rejected {
        strategy_key: String,
        kind: DetectionProfileRejectionKind,
    },
    ExecutionFailed {
        strategy_key: Option<String>,
        kind: DetectionProfileExecutionFailureKind,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionProfileOutcome {
    pub profile_key: String,
    pub completion: DetectionProfileCompletion,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug)]
pub struct DetectionOperationResult {
    pub attempts: Vec<DetectionAttempt>,
    pub profile_outcomes: Vec<DetectionProfileOutcome>,
    pub run_result: ReconciledDetectionRunResult,
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}

pub fn compile_detection_plan(
    profile: &SourceProfileDocument,
) -> Result<CompiledDetectionPlan, Diagnostics> {
    let context = DetectionProfileContext::compile(profile)?;
    let detection = profile.detection.as_ref().ok_or_else(|| {
        vec![compiler_error(
            "missing_detection_plan",
            "Source Profile does not define Detection",
            "/detection",
        )]
    })?;
    if detection.input_url_patterns.is_some()
        || detection.http_checks.is_some()
        || detection.browser_probes.is_some()
    {
        return Err(vec![compiler_error(
            "mixed_detection_execution_shapes",
            "Final Detection strategies cannot be mixed with legacy executable fields",
            "/detection",
        )]);
    }
    if detection.policy != Some(StrategyPolicy::AllRequired) {
        return Err(vec![compiler_error(
            "invalid_detection_policy",
            "Detection requires exact all_required policy",
            "/detection/policy",
        )]);
    }
    let authored = detection
        .strategies
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            vec![compiler_error(
                "missing_detection_strategies",
                "Detection requires a non-empty Strategy Set",
                "/detection/strategies",
            )]
        })?;
    if !matches!(authored.first(), Some(DetectionStrategy::Url { .. }))
        || authored.iter().skip(1).any(|s| {
            !matches!(
                s,
                DetectionStrategy::Http { .. } | DetectionStrategy::Browser { .. }
            )
        })
    {
        return Err(vec![compiler_error(
            "invalid_detection_strategy_order",
            "Detection requires one URL Strategy first followed only by HTTP or Browser Strategies",
            "/detection/strategies",
        )]);
    }
    if authored
        .iter()
        .filter(|strategy| matches!(strategy, DetectionStrategy::Browser { .. }))
        .count()
        > 2
    {
        return Err(vec![compiler_error(
            "detection_browser_navigation_limit_exceeded",
            "A Detection profile may define at most two Browser Strategies",
            "/detection/strategies",
        )]);
    }
    let mut keys = BTreeSet::new();
    let mut available_captures = BTreeSet::new();
    let mut strategies = Vec::with_capacity(authored.len());
    for (index, strategy) in authored.iter().enumerate() {
        let base = format!("/detection/strategies/{index}");
        let key = strategy.key();
        if !is_technical_key(key) || !keys.insert(key.to_string()) {
            return Err(vec![compiler_error(
                "invalid_detection_strategy_key",
                "Detection Strategy keys must be canonical technical keys and unique",
                &format!("{base}/key"),
            )]);
        }
        match strategy {
            DetectionStrategy::Url { key, input } => {
                let input = match input {
                    DetectionUrlInput::AbsoluteUrl => CompiledUrlInput::AbsoluteUrl,
                    DetectionUrlInput::PatternAlternatives { alternatives } => {
                        if alternatives.is_empty() {
                            return Err(vec![compiler_error(
                                "empty_detection_url_alternatives",
                                "URL pattern alternatives must not be empty",
                                &format!("{base}/input/alternatives"),
                            )]);
                        }
                        let mut compiled = Vec::new();
                        let mut guaranteed_captures: Option<BTreeSet<String>> = None;
                        for (alternative_index, alternative) in alternatives.iter().enumerate() {
                            if alternative.pattern.is_empty() {
                                return Err(vec![compiler_error(
                                    "empty_detection_url_pattern",
                                    "URL pattern must not be empty",
                                    &format!(
                                        "{base}/input/alternatives/{alternative_index}/pattern"
                                    ),
                                )]);
                            }
                            let capture_keys = alternative.captures.clone().unwrap_or_default();
                            let pattern = compile_named_pattern(&alternative.pattern, &capture_keys)
                                .map_err(|_| vec![compiler_error(
                                    "invalid_detection_capture_pattern",
                                    "Detection pattern must be valid and contain every selected named group",
                                    &format!("{base}/input/alternatives/{alternative_index}/pattern"),
                                )])?;
                            let capture_set = capture_keys.into_iter().collect::<BTreeSet<_>>();
                            guaranteed_captures = Some(match guaranteed_captures {
                                None => capture_set,
                                Some(current) => {
                                    current.intersection(&capture_set).cloned().collect()
                                }
                            });
                            compiled.push(CompiledUrlAlternative { pattern });
                        }
                        available_captures = guaranteed_captures.unwrap_or_default();
                        CompiledUrlInput::PatternAlternatives(compiled)
                    }
                };
                strategies.push(CompiledDetectionStrategy::Url {
                    key: key.clone(),
                    input,
                });
            }
            DetectionStrategy::Http {
                key,
                fetch,
                expect_status,
                contains,
                regex,
                captures,
                evidence,
            } => {
                let Some((method, url, headers, body, timeout_ms)) = fetch.http_parts() else {
                    return Err(vec![compiler_error(
                        "invalid_detection_http_fetch_mode",
                        "Detection HTTP Strategy requires HTTP Fetch",
                        &format!("{base}/fetch"),
                    )]);
                };
                let descriptor = TemplateDescriptor::new()
                    .allow_bare("inputUrl")
                    .allow_namespace("capture", available_captures.iter().cloned());
                let fetch = compile_http_fetch(
                    method,
                    url,
                    headers,
                    body,
                    timeout_ms,
                    &descriptor,
                    &descriptor,
                    &descriptor,
                )
                .map_err(|error| {
                    vec![compiler_error(
                        error.code,
                        &error.message,
                        &format!("{base}/fetch{}", error.path),
                    )]
                })?;
                if expect_status.is_some_and(|status| !(100..=599).contains(&status)) {
                    return Err(vec![compiler_error(
                        "invalid_detection_expected_status",
                        "expectStatus must be between 100 and 599",
                        &format!("{base}/expectStatus"),
                    )]);
                }
                if contains.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_contains",
                        "contains must not be empty",
                        &format!("{base}/contains"),
                    )]);
                }
                if regex.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_regex",
                        "Detection regex must not be empty",
                        &format!("{base}/regex"),
                    )]);
                }
                if evidence.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_evidence",
                        "Detection evidence must not be empty",
                        &format!("{base}/evidence"),
                    )]);
                }
                let acceptance_regex =
                    regex
                        .as_deref()
                        .map(compile_regex)
                        .transpose()
                        .map_err(|_| {
                            vec![compiler_error(
                                "invalid_detection_regex",
                                "Detection regex is invalid Rust regex syntax",
                                &format!("{base}/regex"),
                            )]
                        })?;
                let capture_keys = captures.clone().unwrap_or_default();
                if captures.is_some() && regex.is_none() {
                    return Err(vec![compiler_error(
                        "detection_captures_require_regex",
                        "HTTP captures require regex",
                        &format!("{base}/captures"),
                    )]);
                }
                let capture_plan = regex
                    .as_deref()
                    .filter(|_| !capture_keys.is_empty())
                    .map(|pattern| compile_named_pattern(pattern, &capture_keys))
                    .transpose()
                    .map_err(|_| {
                        vec![compiler_error(
                            "invalid_detection_capture_pattern",
                            "Detection regex must contain every selected named group",
                            &format!("{base}/regex"),
                        )]
                    })?;
                available_captures.extend(capture_keys);
                strategies.push(CompiledDetectionStrategy::Http {
                    key: key.clone(),
                    fetch: Box::new(fetch),
                    expect_status: *expect_status,
                    contains: contains.clone(),
                    acceptance_regex,
                    captures: capture_plan,
                    evidence: evidence.clone(),
                });
            }
            DetectionStrategy::Browser {
                key,
                fetch,
                contains,
                regex,
                captures,
                evidence,
            } => {
                let descriptor = TemplateDescriptor::new()
                    .allow_bare("inputUrl")
                    .allow_namespace("capture", available_captures.iter().cloned());
                let compiled = compile_browser_fetch_with_descriptor(
                    fetch,
                    &format!("{base}/fetch"),
                    &descriptor,
                )
                .map_err(|error| vec![compiler_error(error.code, &error.message, &error.path)])?;
                let ExecutionPlanFetch::Browser {
                    url,
                    timeout_ms,
                    waits,
                    interactions,
                } = compiled
                else {
                    unreachable!("Browser compiler returns Browser Fetch")
                };
                if !(1..=20_000).contains(&timeout_ms) {
                    return Err(vec![compiler_error(
                        "invalid_detection_browser_timeout",
                        "Detection Browser timeoutMs must be between 1 and 20,000",
                        &format!("{base}/fetch/timeoutMs"),
                    )]);
                }
                if waits.len() > 4 {
                    return Err(vec![compiler_error(
                        "detection_browser_wait_limit_exceeded",
                        "Detection Browser Strategy permits at most four authored waits",
                        &format!("{base}/fetch/waits"),
                    )]);
                }
                for (wait_index, wait) in waits.iter().enumerate() {
                    let wait_timeout = match wait {
                        ExecutionPlanBrowserWait::Selector { timeout_ms, .. }
                        | ExecutionPlanBrowserWait::NetworkIdle { timeout_ms } => *timeout_ms,
                    };
                    if wait_timeout > 5_000 {
                        return Err(vec![compiler_error(
                            "invalid_detection_browser_wait_timeout",
                            "Detection Browser wait timeoutMs must not exceed 5,000",
                            &format!("{base}/fetch/waits/{wait_index}/timeoutMs"),
                        )]);
                    }
                }
                for (interaction_index, interaction) in interactions.iter().enumerate() {
                    let (max_count, wait_after_ms) = match interaction {
                        ExecutionPlanBrowserInteraction::ClickIfVisible {
                            max_count,
                            wait_after_ms,
                            ..
                        }
                        | ExecutionPlanBrowserInteraction::ClickUntilGone {
                            max_count,
                            wait_after_ms,
                            ..
                        } => (*max_count, *wait_after_ms),
                    };
                    if max_count > 5 {
                        return Err(vec![compiler_error(
                            "invalid_detection_browser_action_count",
                            "Detection Browser interaction maxCount must not exceed five",
                            &format!("{base}/fetch/interactions/{interaction_index}/maxCount"),
                        )]);
                    }
                    if wait_after_ms.is_some_and(|duration| duration > 5_000) {
                        return Err(vec![compiler_error(
                            "invalid_detection_browser_wait_after",
                            "Detection Browser waitAfterMs must not exceed 5,000",
                            &format!("{base}/fetch/interactions/{interaction_index}/waitAfterMs"),
                        )]);
                    }
                }
                if contains.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_contains",
                        "contains must not be empty",
                        &format!("{base}/contains"),
                    )]);
                }
                if regex.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_regex",
                        "Detection regex must not be empty",
                        &format!("{base}/regex"),
                    )]);
                }
                if evidence.as_ref().is_some_and(|value| value.is_empty()) {
                    return Err(vec![compiler_error(
                        "empty_detection_evidence",
                        "Detection evidence must not be empty",
                        &format!("{base}/evidence"),
                    )]);
                }
                if contains.is_none() && regex.is_none() {
                    return Err(vec![compiler_error(
                        "missing_detection_browser_acceptance",
                        "Detection Browser Strategy requires contains or regex acceptance",
                        &base,
                    )]);
                }
                let acceptance_regex =
                    regex
                        .as_deref()
                        .map(compile_regex)
                        .transpose()
                        .map_err(|_| {
                            vec![compiler_error(
                                "invalid_detection_regex",
                                "Detection regex is invalid Rust regex syntax",
                                &format!("{base}/regex"),
                            )]
                        })?;
                let capture_keys = captures.clone().unwrap_or_default();
                if captures.is_some() && regex.is_none() {
                    return Err(vec![compiler_error(
                        "detection_captures_require_regex",
                        "Browser captures require regex",
                        &format!("{base}/captures"),
                    )]);
                }
                let capture_plan = regex
                    .as_deref()
                    .filter(|_| !capture_keys.is_empty())
                    .map(|pattern| compile_named_pattern(pattern, &capture_keys))
                    .transpose()
                    .map_err(|_| {
                        vec![compiler_error(
                            "invalid_detection_capture_pattern",
                            "Detection regex must contain every selected named group",
                            &format!("{base}/regex"),
                        )]
                    })?;
                available_captures.extend(capture_keys);
                strategies.push(CompiledDetectionStrategy::Browser {
                    key: key.clone(),
                    url,
                    timeout_ms,
                    waits,
                    interactions,
                    contains: contains.clone(),
                    acceptance_regex,
                    captures: capture_plan,
                    evidence: evidence.clone(),
                });
            }
        }
    }
    let proposal_descriptor = TemplateDescriptor::new()
        .allow_bare("inputUrl")
        .allow_namespace("capture", available_captures.iter().cloned());
    let proposal_source_config = detection
        .source_config
        .as_ref()
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| {
                    compile_detection_json_value(
                        value,
                        &proposal_descriptor,
                        &format!("/detection/sourceConfig/{key}"),
                    )
                    .map(|value| (key.clone(), value))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
        })
        .transpose()?;
    let key_candidates = compile_candidate_templates(
        detection.key_candidates.as_deref().unwrap_or_default(),
        &proposal_descriptor,
        "/detection/keyCandidates",
    )?;
    let name_candidates = compile_candidate_templates(
        detection.name_candidates.as_deref().unwrap_or_default(),
        &proposal_descriptor,
        "/detection/nameCandidates",
    )?;
    Ok(CompiledDetectionPlan {
        profile_key: profile.key.clone(),
        context,
        strategies,
        proposal_source_config,
        key_candidates,
        name_candidates,
    })
}

fn compile_candidate_templates(
    values: &[String],
    descriptor: &TemplateDescriptor,
    base_path: &str,
) -> Result<Vec<CompiledTemplate>, Diagnostics> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            compile_template(value, descriptor).map_err(|error| {
                vec![compiler_error(
                    "invalid_detection_proposal_template",
                    &error.to_string(),
                    &format!("{base_path}/{index}"),
                )]
            })
        })
        .collect()
}

fn compile_detection_json_value(
    value: &serde_json::Value,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<CompiledDetectionJsonValue, Diagnostics> {
    match value {
        serde_json::Value::String(value) => compile_template(value, descriptor)
            .map(CompiledDetectionJsonValue::Template)
            .map_err(|error| {
                vec![compiler_error(
                    "invalid_detection_proposal_template",
                    &error.to_string(),
                    path,
                )]
            }),
        serde_json::Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                compile_detection_json_value(value, descriptor, &format!("{path}/{index}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(CompiledDetectionJsonValue::Array),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                compile_detection_json_value(value, descriptor, &format!("{path}/{key}"))
                    .map(|value| (key.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(CompiledDetectionJsonValue::Object),
        _ => Ok(CompiledDetectionJsonValue::Literal(value.clone())),
    }
}

fn render_detection_json_value(
    value: &CompiledDetectionJsonValue,
    values: &dyn TemplateValueView,
) -> Result<serde_json::Value, ()> {
    match value {
        CompiledDetectionJsonValue::Template(template) => render_template(template, values)
            .map(serde_json::Value::String)
            .map_err(|_| ()),
        CompiledDetectionJsonValue::Array(items) => items
            .iter()
            .map(|item| render_detection_json_value(item, values))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        CompiledDetectionJsonValue::Object(items) => items
            .iter()
            .map(|(key, value)| {
                render_detection_json_value(value, values).map(|value| (key.clone(), value))
            })
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
        CompiledDetectionJsonValue::Literal(value) => Ok(value.clone()),
    }
}

pub async fn execute_detection_operation<C>(
    input_url: &str,
    plans: &[CompiledDetectionPlan],
    client: &C,
    browser: PhaseBrowser<&dyn BrowserAcquisition>,
    cancellation: &dyn RuntimeCancellation,
) -> DetectionOperationResult
where
    C: ProfileHttpClient + Sync + ?Sized,
{
    let canonical_url = match canonical_url(input_url) {
        Ok(url) => url,
        Err(diagnostic) => {
            let diagnostics = vec![diagnostic];
            return DetectionOperationResult {
                attempts: Vec::new(),
                profile_outcomes: Vec::new(),
                run_result: aggregate_detection_attempts(vec![DetectionAttempt::Failed(
                    diagnostics.clone(),
                )]),
                diagnostics,
                report: InvocationAllowance::prestart_failure_report(),
            };
        }
    };
    let allowance = InvocationAllowance::new_inactive_with_logical_wait_limit(
        detection_operation_limits(),
        false,
        None,
        Some(32),
    );
    let base_context = RuntimeExecutionContext::with_cancellation(cancellation);
    let execution_context = base_context.for_detection_http(&allowance);
    let mut attempts = Vec::new();
    let mut profile_outcomes = Vec::new();
    let mut diagnostics = Vec::new();

    for plan in plans {
        if cancellation.is_cancelled() {
            return terminal_result(
                &allowance,
                PhaseCompletion::Cancelled {
                    reason: crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                },
                Vec::new(),
                Vec::new(),
                diagnostics,
            );
        }
        let profile_scope = match allowance.inactive_child_scope(
            InvocationAllowance::ROOT_SCOPE,
            detection_profile_limits(),
            Some(8),
        ) {
            Ok(scope) => scope,
            Err(stop) => {
                return terminal_result(
                    &allowance,
                    completion_for_stop(stop),
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                )
            }
        };
        let state = Mutex::new(plan.context.initial_state());
        let profile_completion = Mutex::new(None);
        let allowance_ref = &allowance;
        let browser_ref = &browser;
        let execution = execute_strategy_set(
            StrategyPolicy::AllRequired,
            &plan.strategies,
            CompiledDetectionStrategy::key,
            |index, strategy| {
                cancellation.is_cancelled().then(|| {
                    TypedCancellation::strategy(
                        RuntimePhase::Detection,
                        index,
                        strategy.key(),
                        CancellationOperation::Phase,
                    )
                })
            },
            |index, strategy| {
                let state = &state;
                let canonical_url = &canonical_url;
                let context = &plan.context;
                let profile_completion = &profile_completion;
                Box::pin(async move {
                    execute_strategy(
                        index,
                        strategy,
                        canonical_url,
                        context,
                        state,
                        client,
                        browser_ref,
                        allowance_ref,
                        profile_scope,
                        execution_context,
                        profile_completion,
                    )
                    .await
                })
            },
        )
        .await;
        let mut attempt_diagnostics = execution
            .attempts
            .iter()
            .flat_map(|attempt| attempt.diagnostics.clone())
            .collect::<Vec<_>>();
        match execution.terminal {
            StrategySetTerminal::Satisfied => {
                if cancellation.is_cancelled() {
                    diagnostics.extend(attempt_diagnostics);
                    return terminal_result(
                        &allowance,
                        PhaseCompletion::Cancelled {
                            reason:
                                crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                        },
                        Vec::new(),
                        Vec::new(),
                        diagnostics,
                    );
                }
                let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
                let values = DetectionTemplateValues::from_state(&canonical_url, &snapshot);
                let rendered_source_config = plan
                    .proposal_source_config
                    .as_ref()
                    .map(|source_config| {
                        source_config
                            .iter()
                            .map(|(key, value)| {
                                render_detection_json_value(value, &values)
                                    .map(|value| (key.clone(), value))
                            })
                            .collect::<Result<serde_json::Map<_, _>, _>>()
                    })
                    .transpose();
                let render_candidates = |templates: &[CompiledTemplate]| {
                    templates
                        .iter()
                        .map(|template| render_template(template, &values).map_err(|_| ()))
                        .collect::<Result<Vec<_>, _>>()
                };
                let prepared = rendered_source_config
                    .and_then(|source_config| {
                        render_candidates(&plan.key_candidates).map(|keys| (source_config, keys))
                    })
                    .and_then(|(source_config, keys)| {
                        render_candidates(&plan.name_candidates)
                            .map(|names| (source_config, keys, names))
                    });
                let Ok((source_config, key_candidates, name_candidates)) = prepared else {
                    let profile_diagnostics = vec![runtime_error(
                        "detection_proposal_template_failed",
                        "Detection proposal Template dependency was unavailable",
                        "/detection",
                        "proposal",
                    )];
                    attempts.push(DetectionAttempt::Failed(profile_diagnostics.clone()));
                    profile_outcomes.push(DetectionProfileOutcome {
                        profile_key: plan.profile_key.clone(),
                        completion: DetectionProfileCompletion::ExecutionFailed {
                            strategy_key: None,
                            kind: DetectionProfileExecutionFailureKind::Proposal,
                        },
                        diagnostics: profile_diagnostics,
                    });
                    continue;
                };
                let prepared_proposal = plan.context.prepare_proposal_with_canonical_url(
                    &snapshot,
                    &canonical_url,
                    source_config,
                    key_candidates,
                    name_candidates,
                );
                if cancellation.is_cancelled() {
                    diagnostics.extend(attempt_diagnostics);
                    return terminal_result(
                        &allowance,
                        PhaseCompletion::Cancelled {
                            reason:
                                crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                        },
                        Vec::new(),
                        Vec::new(),
                        diagnostics,
                    );
                }
                match prepared_proposal {
                    Ok(PreparedDetectionOutput::Proposal(proposal)) => {
                        attempts.push(DetectionAttempt::Matched(proposal));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::Matched,
                            diagnostics: attempt_diagnostics,
                        });
                    }
                    Ok(PreparedDetectionOutput::Unsupported(profile)) => {
                        attempts.push(DetectionAttempt::Unsupported(profile));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::Unsupported,
                            diagnostics: attempt_diagnostics,
                        });
                    }
                    Err(error) => {
                        let profile_diagnostics = error.diagnostics();
                        attempts.push(DetectionAttempt::Failed(profile_diagnostics.clone()));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::ExecutionFailed {
                                strategy_key: None,
                                kind: DetectionProfileExecutionFailureKind::Proposal,
                            },
                            diagnostics: profile_diagnostics,
                        });
                    }
                }
            }
            StrategySetTerminal::PolicyUnsatisfied => {
                attempt_diagnostics.push(policy_unsatisfied_diagnostic(
                    StrategyPolicy::AllRequired,
                    RuntimePhase::Detection,
                ));
                attempts.push(DetectionAttempt::Failed(attempt_diagnostics.clone()));
                profile_outcomes.push(DetectionProfileOutcome {
                    profile_key: plan.profile_key.clone(),
                    completion: profile_completion
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .take()
                        .unwrap_or(DetectionProfileCompletion::ExecutionFailed {
                            strategy_key: None,
                            kind: DetectionProfileExecutionFailureKind::Reconciliation,
                        }),
                    diagnostics: attempt_diagnostics,
                });
            }
            StrategySetTerminal::Cancelled(_) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    PhaseCompletion::Cancelled {
                        reason: crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                    },
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
            StrategySetTerminal::Stopped(AllowanceStop::Exhausted(_)) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    completion_for_stop(allowance.stop().unwrap_or(AllowanceStop::Internal)),
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
            StrategySetTerminal::Stopped(AllowanceStop::Internal) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    PhaseCompletion::ExecutionFailed,
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
        }
    }
    let completion = allowance
        .stop()
        .map(completion_for_stop)
        .unwrap_or(PhaseCompletion::Accepted);
    terminal_result(
        &allowance,
        completion,
        attempts,
        profile_outcomes,
        diagnostics,
    )
}

async fn execute_strategy<C>(
    index: usize,
    strategy: &CompiledDetectionStrategy,
    input_url: &str,
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    client: &C,
    browser: &PhaseBrowser<&dyn BrowserAcquisition>,
    allowance: &InvocationAllowance,
    profile_scope: usize,
    execution_context: RuntimeExecutionContext<'_>,
    profile_completion: &Mutex<Option<DetectionProfileCompletion>>,
) -> StrategyExecution<()>
where
    C: ProfileHttpClient + Sync + ?Sized,
{
    let base = format!("/detection/strategies/{index}");
    match strategy {
        CompiledDetectionStrategy::Url {
            key: _,
            input: CompiledUrlInput::AbsoluteUrl,
        } => StrategyExecution {
            diagnostics: Vec::new(),
            completion: StrategyAttemptCompletion::Accepted(()),
        },
        CompiledDetectionStrategy::Url {
            key,
            input: CompiledUrlInput::PatternAlternatives(alternatives),
        } => {
            for (alternative_index, alternative) in alternatives.iter().enumerate() {
                match evaluate_named_pattern(&alternative.pattern, input_url) {
                    Ok(Some(captures)) => {
                        if let Err(diagnostics) = apply_captures(
                            profile,
                            state,
                            key,
                            &format!("{base}/input/alternatives/{alternative_index}"),
                            captures,
                        ) {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                        let evidence = DetectionEvidenceContribution::new(
                            DetectionEvidenceKind::Url,
                            format!("{base}/input/alternatives/{alternative_index}/pattern"),
                            "Detection input URL matched an authored alternative",
                        )
                        .expect("compiled descriptor path is valid");
                        if let Err(diagnostics) = apply_one(
                            profile,
                            state,
                            DetectionContribution::new(
                                DetectionOrigin::new(
                                    key,
                                    format!(
                                        "{base}/input/alternatives/{alternative_index}/pattern"
                                    ),
                                )
                                .unwrap(),
                            )
                            .with_evidence(evidence),
                        ) {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                        return StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Accepted(()),
                        };
                    }
                    Ok(None) => continue,
                    Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection named capture did not resolve",
                        );
                    }
                }
            }
            set_rejection(profile_completion, key, DetectionProfileRejectionKind::Url);
            rejected(
                key,
                &base,
                "detection_url_not_matched",
                "Detection URL alternatives did not match",
            )
        }
        CompiledDetectionStrategy::Http {
            key,
            fetch,
            expect_status,
            contains,
            acceptance_regex,
            captures,
            evidence,
        } => {
            let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let values = DetectionTemplateValues::from_state(input_url, &snapshot);
            let response = match execute_http_fetch(
                client,
                fetch,
                &values,
                HttpFetchOverlay::default(),
                None,
                HttpStatusPolicy::PreserveResponse,
                execution_context,
            )
            .await
            {
                Ok(response) => response,
                Err(HttpFetchExecutionError::Cancelled) => return cancelled(index, key),
                Err(HttpFetchExecutionError::Acquisition(error))
                    if error.kind == ProfileHttpFailureKind::Cancelled =>
                {
                    return cancelled(index, key)
                }
                Err(HttpFetchExecutionError::Acquisition(error))
                    if error.kind == ProfileHttpFailureKind::ResponseBytesExceeded =>
                {
                    let stop = execution_context.stop().unwrap_or(AllowanceStop::Internal);
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(stop),
                    };
                }
                Err(HttpFetchExecutionError::BudgetExhausted) => {
                    let stop = execution_context.stop().unwrap_or(AllowanceStop::Internal);
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(stop),
                    };
                }
                Err(HttpFetchExecutionError::Render(error)) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Render,
                    );
                    return failed(vec![runtime_error(
                        error.code,
                        &error.message,
                        &format!("{base}/fetch{}", error.path),
                        key,
                    )]);
                }
                Err(HttpFetchExecutionError::Acquisition(error)) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Acquisition(error.kind),
                    );
                    return failed(vec![runtime_error(
                        "detection_http_acquisition_failed",
                        "Detection HTTP acquisition failed",
                        &format!("{base}/fetch"),
                        key,
                    )]);
                }
                Err(HttpFetchExecutionError::NonSuccessStatus { .. }) => {
                    unreachable!("PreserveResponse never projects status failure")
                }
            };
            if expect_status.is_some_and(|expected| !values_equal(&response.status(), &expected)) {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Status,
                );
                return rejected(
                    key,
                    &base,
                    "detection_status_rejected",
                    "Detection HTTP status did not match expectStatus",
                );
            }
            if contains
                .as_ref()
                .is_some_and(|expected| !literal_contains(&response.body, expected))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Contains,
                );
                return rejected(
                    key,
                    &base,
                    "detection_contains_rejected",
                    "Detection HTTP body did not contain the required literal",
                );
            }
            if acceptance_regex
                .as_ref()
                .is_some_and(|regex| !regex.is_match(&response.body))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Regex,
                );
                return rejected(
                    key,
                    &base,
                    "detection_regex_rejected",
                    "Detection HTTP body did not match the required regex",
                );
            }
            if let Some(pattern) = captures {
                match evaluate_named_pattern(pattern, &response.body) {
                    Ok(Some(outputs)) => {
                        if let Err(diagnostics) =
                            apply_captures(profile, state, key, &format!("{base}/regex"), outputs)
                        {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                    }
                    Ok(None) | Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection HTTP named captures did not resolve",
                        );
                    }
                }
            }
            let (evidence_path, evidence_message) = evidence.as_ref().map_or_else(
                || (format!("{base}/fetch"), "Detection HTTP Strategy accepted"),
                |message| (format!("{base}/evidence"), message.as_str()),
            );
            let contribution =
                DetectionContribution::new(DetectionOrigin::new(key, &evidence_path).unwrap())
                    .with_evidence(
                        DetectionEvidenceContribution::new(
                            DetectionEvidenceKind::Http,
                            &evidence_path,
                            evidence_message,
                        )
                        .unwrap(),
                    );
            if let Err(diagnostics) = apply_one(profile, state, contribution) {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::Reconciliation,
                );
                return failed(diagnostics);
            }
            StrategyExecution {
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(()),
            }
        }
        CompiledDetectionStrategy::Browser {
            key,
            url,
            timeout_ms,
            waits,
            interactions,
            contains,
            acceptance_regex,
            captures,
            evidence,
        } => {
            let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let values = DetectionTemplateValues::from_state(input_url, &snapshot);
            let target = match render_template(url, &values) {
                Ok(target) => target,
                Err(_) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Render,
                    );
                    return failed(vec![runtime_error(
                        "detection_browser_template_failed",
                        "Detection Browser target Template dependency was unavailable",
                        &format!("{base}/fetch/url"),
                        key,
                    )]);
                }
            };
            let PhaseBrowser::Browser(acquisition) = browser else {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::BrowserAcquisition(
                        DetectionBrowserFailureKind::RuntimeLaunch,
                    ),
                );
                return failed(vec![runtime_error(
                    "browser_runtime_unavailable",
                    "Detection Browser acquisition is unavailable",
                    &format!("{base}/fetch"),
                    key,
                )]);
            };
            if let Err(stop) = allowance.activate_scope_chain(profile_scope) {
                return StrategyExecution {
                    diagnostics: Vec::new(),
                    completion: StrategyAttemptCompletion::Stopped(stop),
                };
            }
            let strategy_scope =
                match allowance.child_scope(profile_scope, detection_strategy_limits(), Some(4)) {
                    Ok(scope) => scope,
                    Err(stop) => {
                        return StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Stopped(stop),
                        }
                    }
                };
            let control = execution_context.for_allowance_scope(allowance, strategy_scope);
            let rendered = match execute_canonical_browser_fetch(
                *acquisition,
                RuntimePhase::Detection,
                BrowserPhaseFetchInput {
                    target,
                    timeout_ms: *timeout_ms,
                    waits: waits.clone(),
                    interactions: interactions.clone(),
                    base_path: base.clone(),
                    strategy_key: key.clone(),
                    strategy_index: index,
                    control,
                },
            )
            .await
            {
                BrowserPhaseFetchProjection::Rendered(rendered) => rendered,
                BrowserPhaseFetchProjection::AttemptFailed { diagnostic, kind } => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::BrowserAcquisition(
                            browser_failure_kind(&kind),
                        ),
                    );
                    return failed(vec![diagnostic]);
                }
                BrowserPhaseFetchProjection::PhaseFatal(diagnostic) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::BrowserInfrastructure,
                    );
                    return failed(vec![diagnostic]);
                }
                BrowserPhaseFetchProjection::AllowanceStopped => {
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(
                            control.stop().unwrap_or(AllowanceStop::Internal),
                        ),
                    }
                }
                BrowserPhaseFetchProjection::Cancelled(cancellation) => {
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Cancelled(cancellation),
                    }
                }
            };
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            if contains
                .as_ref()
                .is_some_and(|expected| !literal_contains(&rendered, expected))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Contains,
                );
                return rejected(
                    key,
                    &base,
                    "detection_contains_rejected",
                    "Detection Browser content did not contain the required literal",
                );
            }
            if acceptance_regex
                .as_ref()
                .is_some_and(|regex| !regex.is_match(&rendered))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Regex,
                );
                return rejected(
                    key,
                    &base,
                    "detection_regex_rejected",
                    "Detection Browser content did not match the required regex",
                );
            }
            if let Some(pattern) = captures {
                match evaluate_named_pattern(pattern, &rendered) {
                    Ok(Some(outputs)) => {
                        if control.is_cancelled() {
                            return cancelled(index, key);
                        }
                        if let Err(diagnostics) =
                            apply_captures(profile, state, key, &format!("{base}/regex"), outputs)
                        {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                    }
                    Ok(None) | Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection Browser named captures did not resolve",
                        );
                    }
                }
            }
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            let (evidence_path, evidence_message) = evidence.as_ref().map_or_else(
                || {
                    (
                        format!("{base}/fetch"),
                        "Detection Browser Strategy accepted",
                    )
                },
                |message| (format!("{base}/evidence"), message.as_str()),
            );
            let contribution = DetectionContribution::new(
                DetectionOrigin::new(key, &evidence_path).expect("compiled origin"),
            )
            .with_evidence(
                DetectionEvidenceContribution::new(
                    DetectionEvidenceKind::Browser,
                    &evidence_path,
                    evidence_message,
                )
                .expect("compiled evidence"),
            );
            if let Err(diagnostics) = apply_one(profile, state, contribution) {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::Reconciliation,
                );
                return failed(diagnostics);
            }
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            StrategyExecution {
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(()),
            }
        }
    }
}

fn apply_captures(
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    key: &str,
    path: &str,
    captures: Vec<crate::profile_dsl::primitives::capture::CaptureOutput>,
) -> Result<(), Diagnostics> {
    for capture in captures {
        apply_one(
            profile,
            state,
            DetectionContribution::new(DetectionOrigin::new(key, path).unwrap())
                .with_capture(capture.key, capture.value),
        )?;
    }
    Ok(())
}

fn apply_one(
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    contribution: DetectionContribution,
) -> Result<(), Diagnostics> {
    let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let next = profile
        .apply(&snapshot, contribution)
        .map_err(|error| error.diagnostics())?;
    *state.lock().unwrap_or_else(|p| p.into_inner()) = next;
    Ok(())
}

struct DetectionTemplateValues<'a> {
    input_url: &'a str,
    captures: HashMap<String, String>,
}
impl<'a> DetectionTemplateValues<'a> {
    fn from_state(input_url: &'a str, state: &ReconciledDetectionState) -> Self {
        Self {
            input_url,
            captures: state
                .captures()
                .iter()
                .map(|capture| (capture.key().to_string(), capture.value().to_string()))
                .collect(),
        }
    }
}
impl TemplateValueView for DetectionTemplateValues<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            None if reference.key == "inputUrl" => Some(self.input_url.to_string()),
            Some("capture") => self.captures.get(&reference.key).cloned(),
            _ => None,
        }
    }
}

fn detection_operation_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 8,
        max_duration_ms: 60_000,
        max_pages: 32,
        max_browser_actions: 32,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 16_777_216,
        ..PhaseLimits::BACKEND
    }
}

fn detection_profile_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 2,
        max_duration_ms: 30_000,
        max_pages: 8,
        max_browser_actions: 10,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 4_194_304,
        ..PhaseLimits::BACKEND
    }
}

fn detection_strategy_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 1,
        max_duration_ms: 20_000,
        max_pages: 4,
        max_browser_actions: 32,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 2_097_152,
        ..PhaseLimits::BACKEND
    }
}

fn browser_failure_kind(kind: &BrowserAcquisitionFailureKind) -> DetectionBrowserFailureKind {
    match kind {
        BrowserAcquisitionFailureKind::RuntimeLaunch => DetectionBrowserFailureKind::RuntimeLaunch,
        BrowserAcquisitionFailureKind::Navigation => DetectionBrowserFailureKind::Navigation,
        BrowserAcquisitionFailureKind::Wait { .. } => DetectionBrowserFailureKind::Wait,
        BrowserAcquisitionFailureKind::Interaction { .. } => {
            DetectionBrowserFailureKind::Interaction
        }
        BrowserAcquisitionFailureKind::ContentRead => DetectionBrowserFailureKind::ContentRead,
        BrowserAcquisitionFailureKind::Deadline => DetectionBrowserFailureKind::Deadline,
    }
}

fn canonical_url(input: &str) -> Result<String, Diagnostic> {
    let url = Url::parse(input.trim()).map_err(|_| {
        runtime_error(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL",
            "/inputUrl",
            "input",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.has_host()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(runtime_error(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL without userinfo",
            "/inputUrl",
            "input",
        ));
    }
    Ok(url.to_string())
}

fn is_technical_key(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn compiler_error(code: &str, message: &str, path: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Compiler,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.to_string(),
        strategy_key: None,
        details: None,
    }
}
fn runtime_error(code: &str, message: &str, path: &str, key: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.to_string(),
        strategy_key: Some(key.to_string()),
        details: None,
    }
}
fn set_rejection(
    completion: &Mutex<Option<DetectionProfileCompletion>>,
    strategy_key: &str,
    kind: DetectionProfileRejectionKind,
) {
    *completion.lock().unwrap_or_else(|p| p.into_inner()) =
        Some(DetectionProfileCompletion::Rejected {
            strategy_key: strategy_key.to_string(),
            kind,
        });
}

fn set_execution_failure(
    completion: &Mutex<Option<DetectionProfileCompletion>>,
    strategy_key: &str,
    kind: DetectionProfileExecutionFailureKind,
) {
    *completion.lock().unwrap_or_else(|p| p.into_inner()) =
        Some(DetectionProfileCompletion::ExecutionFailed {
            strategy_key: Some(strategy_key.to_string()),
            kind,
        });
}

fn rejected(key: &str, path: &str, code: &str, message: &str) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics: vec![runtime_error(code, message, path, key)],
        completion: StrategyAttemptCompletion::Rejected,
    }
}
fn failed(diagnostics: Diagnostics) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics,
        completion: StrategyAttemptCompletion::Failed,
    }
}
fn cancelled(index: usize, key: &str) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics: Vec::new(),
        completion: StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
            RuntimePhase::Detection,
            index,
            key,
            CancellationOperation::Fetch,
        )),
    }
}
fn terminal_result(
    allowance: &InvocationAllowance,
    completion: PhaseCompletion,
    attempts: Vec<DetectionAttempt>,
    profile_outcomes: Vec<DetectionProfileOutcome>,
    diagnostics: Diagnostics,
) -> DetectionOperationResult {
    let aggregate_attempts = match &completion {
        PhaseCompletion::BudgetExhausted { .. } => {
            vec![DetectionAttempt::BudgetExhausted(diagnostics.clone())]
        }
        PhaseCompletion::Cancelled { .. } => {
            vec![DetectionAttempt::Cancelled(diagnostics.clone())]
        }
        PhaseCompletion::ExecutionFailed if attempts.is_empty() => {
            vec![DetectionAttempt::Failed(diagnostics.clone())]
        }
        PhaseCompletion::Accepted
        | PhaseCompletion::PolicyUnsatisfied
        | PhaseCompletion::ExecutionFailed => attempts.clone(),
    };
    let report = allowance.report(completion);
    DetectionOperationResult {
        attempts,
        profile_outcomes,
        run_result: aggregate_detection_attempts(aggregate_attempts),
        diagnostics,
        report,
    }
}
