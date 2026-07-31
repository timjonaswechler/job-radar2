use std::collections::{BTreeMap, BTreeSet};

use crate::definition::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::definition::documents::{
    DetectionStrategy, DetectionStrategyKind, DetectionUrlInput, DetectionUrlInputKind,
};
use crate::definition::execution_plan::capabilities::{
    compile_browser_fetch_with_descriptor, ExecutionPlanBrowserInteraction,
    ExecutionPlanBrowserWait, ExecutionPlanFetch,
};
use crate::definition::policy::StrategyPolicy;
use crate::definition::primitives::capture::{compile_named_pattern, CompiledNamedPattern};
use crate::definition::primitives::fetch::http::{compile_http_fetch, CompiledHttpFetch};
use crate::definition::primitives::predicate::{compile_regex, CompiledRegex};
use crate::definition::profile::SourceProfileDocument;
use crate::definition::template::{compile_template, CompiledTemplate, TemplateDescriptor};

use super::reconciliation::DetectionProfileContext;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectionDescriptorShape {
    Tagged,
    Entry,
    ParentOption,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionOptionDescriptor {
    pub key: &'static str,
    pub required: bool,
    pub non_empty: bool,
    pub minimum: Option<u64>,
    pub maximum: Option<u64>,
    pub shape: DetectionDescriptorShape,
    pub compiled_identity: &'static str,
}

const fn option(
    key: &'static str,
    required: bool,
    compiled_identity: &'static str,
) -> DetectionOptionDescriptor {
    constrained_option(key, required, false, None, None, compiled_identity)
}

const fn non_empty_option(
    key: &'static str,
    required: bool,
    compiled_identity: &'static str,
) -> DetectionOptionDescriptor {
    constrained_option(key, required, true, None, None, compiled_identity)
}

const fn bounded_option(
    key: &'static str,
    required: bool,
    minimum: Option<u64>,
    maximum: Option<u64>,
    compiled_identity: &'static str,
) -> DetectionOptionDescriptor {
    constrained_option(key, required, false, minimum, maximum, compiled_identity)
}

const fn constrained_option(
    key: &'static str,
    required: bool,
    non_empty: bool,
    minimum: Option<u64>,
    maximum: Option<u64>,
    compiled_identity: &'static str,
) -> DetectionOptionDescriptor {
    DetectionOptionDescriptor {
        key,
        required,
        non_empty,
        minimum,
        maximum,
        shape: DetectionDescriptorShape::ParentOption,
        compiled_identity,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionShapeDescriptor {
    pub key: &'static str,
    pub owner: &'static str,
    pub canonical_file: &'static str,
    pub shape: DetectionDescriptorShape,
    pub compiled_identity: &'static str,
    pub options: &'static [DetectionOptionDescriptor],
}

const URL_OPTIONS: &[DetectionOptionDescriptor] = &[
    option("key", true, "CompiledDetectionStrategy::Url.key"),
    option("input", true, "CompiledDetectionStrategy::Url.input"),
];
const URL_PATTERN_ALTERNATIVES_OPTIONS: &[DetectionOptionDescriptor] = &[bounded_option(
    "alternatives",
    true,
    Some(1),
    None,
    "CompiledUrlInput::PatternAlternatives.alternatives",
)];
const INPUT_URL_PATTERN_OPTIONS: &[DetectionOptionDescriptor] = &[
    non_empty_option("pattern", true, "CompiledUrlAlternative.pattern"),
    option("captures", false, "CompiledUrlAlternative.pattern.keys"),
];
const URL_ABSOLUTE_OPTIONS: &[DetectionOptionDescriptor] = &[];
const HTTP_OPTIONS: &[DetectionOptionDescriptor] = &[
    option("key", true, "CompiledDetectionStrategy::Http.key"),
    option("fetch", true, "CompiledDetectionStrategy::Http.fetch"),
    bounded_option(
        "expectStatus",
        false,
        Some(100),
        Some(599),
        "CompiledDetectionStrategy::Http.expect_status",
    ),
    non_empty_option(
        "contains",
        false,
        "CompiledDetectionStrategy::Http.contains",
    ),
    non_empty_option(
        "regex",
        false,
        "CompiledDetectionStrategy::Http.acceptance_regex",
    ),
    option(
        "captures",
        false,
        "CompiledDetectionStrategy::Http.captures",
    ),
    non_empty_option(
        "evidence",
        false,
        "CompiledDetectionStrategy::Http.evidence",
    ),
];
const BROWSER_OPTIONS: &[DetectionOptionDescriptor] = &[
    option("key", true, "CompiledDetectionStrategy::Browser.key"),
    option("fetch", true, "ExecutionPlanFetch::Browser"),
    non_empty_option(
        "contains",
        false,
        "CompiledDetectionStrategy::Browser.contains",
    ),
    non_empty_option(
        "regex",
        false,
        "CompiledDetectionStrategy::Browser.acceptance_regex",
    ),
    option(
        "captures",
        false,
        "CompiledDetectionStrategy::Browser.captures",
    ),
    non_empty_option(
        "evidence",
        false,
        "CompiledDetectionStrategy::Browser.evidence",
    ),
];

pub const DETECTION_URL_DESCRIPTOR: DetectionShapeDescriptor = DetectionShapeDescriptor {
    key: "url",
    owner: "D02",
    canonical_file: file!(),
    shape: DetectionDescriptorShape::Tagged,
    compiled_identity: "CompiledDetectionStrategy::Url",
    options: URL_OPTIONS,
};
pub const DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR: DetectionShapeDescriptor =
    DetectionShapeDescriptor {
        key: "pattern_alternatives",
        owner: "D02",
        canonical_file: file!(),
        shape: DetectionDescriptorShape::Tagged,
        compiled_identity: "CompiledUrlInput::PatternAlternatives",
        options: URL_PATTERN_ALTERNATIVES_OPTIONS,
    };
pub const DETECTION_INPUT_URL_PATTERN_DESCRIPTOR: DetectionShapeDescriptor =
    DetectionShapeDescriptor {
        key: "input_url_pattern",
        owner: "D02",
        canonical_file: file!(),
        shape: DetectionDescriptorShape::Entry,
        compiled_identity: "CompiledUrlAlternative",
        options: INPUT_URL_PATTERN_OPTIONS,
    };
pub const DETECTION_URL_ABSOLUTE_DESCRIPTOR: DetectionShapeDescriptor = DetectionShapeDescriptor {
    key: "absolute_url",
    owner: "D02",
    canonical_file: file!(),
    shape: DetectionDescriptorShape::Tagged,
    compiled_identity: "CompiledUrlInput::AbsoluteUrl",
    options: URL_ABSOLUTE_OPTIONS,
};
pub const DETECTION_HTTP_DESCRIPTOR: DetectionShapeDescriptor = DetectionShapeDescriptor {
    key: "http",
    owner: "D02",
    canonical_file: file!(),
    shape: DetectionDescriptorShape::Tagged,
    compiled_identity: "CompiledDetectionStrategy::Http",
    options: HTTP_OPTIONS,
};
pub const DETECTION_BROWSER_DESCRIPTOR: DetectionShapeDescriptor = DetectionShapeDescriptor {
    key: "browser",
    owner: "D03",
    canonical_file: file!(),
    shape: DetectionDescriptorShape::Tagged,
    compiled_identity: "CompiledDetectionStrategy::Browser",
    options: BROWSER_OPTIONS,
};

const DETECTION_DESCRIPTORS: [DetectionShapeDescriptor; 6] = [
    DETECTION_URL_DESCRIPTOR,
    DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR,
    DETECTION_INPUT_URL_PATTERN_DESCRIPTOR,
    DETECTION_URL_ABSOLUTE_DESCRIPTOR,
    DETECTION_HTTP_DESCRIPTOR,
    DETECTION_BROWSER_DESCRIPTOR,
];

pub fn detection_shape_descriptors() -> &'static [DetectionShapeDescriptor] {
    &DETECTION_DESCRIPTORS
}

pub fn validate_detection_shape_descriptors(
    descriptors: &[DetectionShapeDescriptor],
) -> Result<(), &'static str> {
    let mut actual = descriptors.to_vec();
    actual.sort_by_key(|descriptor| descriptor.key);
    if actual.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err("duplicate Detection shape descriptor key");
    }
    let mut expected = DETECTION_DESCRIPTORS.to_vec();
    expected.sort_by_key(|descriptor| descriptor.key);
    if actual != expected {
        return Err("Detection shape descriptors conflict with the canonical catalogue");
    }
    Ok(())
}

pub fn detection_descriptor_for_authored_kind(
    kind: DetectionStrategyKind,
) -> &'static DetectionShapeDescriptor {
    match kind {
        DetectionStrategyKind::Url => &DETECTION_URL_DESCRIPTOR,
        DetectionStrategyKind::Http => &DETECTION_HTTP_DESCRIPTOR,
        DetectionStrategyKind::Browser => &DETECTION_BROWSER_DESCRIPTOR,
    }
}

pub fn detection_descriptor_for_url_input_kind(
    kind: DetectionUrlInputKind,
) -> &'static DetectionShapeDescriptor {
    match kind {
        DetectionUrlInputKind::PatternAlternatives => {
            &DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR
        }
        DetectionUrlInputKind::AbsoluteUrl => &DETECTION_URL_ABSOLUTE_DESCRIPTOR,
    }
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct CompiledDetectionPlan {
    pub profile_key: String,
    pub context: DetectionProfileContext,
    pub strategies: Vec<CompiledDetectionStrategy>,
    pub proposal_source_config: Option<BTreeMap<String, CompiledDetectionJsonValue>>,
    pub key_candidates: Vec<CompiledTemplate>,
    pub name_candidates: Vec<CompiledTemplate>,
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub enum CompiledDetectionJsonValue {
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

    pub fn strategy_descriptors(
        &self,
    ) -> impl Iterator<Item = &'static DetectionShapeDescriptor> + '_ {
        self.strategies
            .iter()
            .map(CompiledDetectionStrategy::descriptor)
    }

    pub fn url_input_descriptors(
        &self,
    ) -> impl Iterator<Item = &'static DetectionShapeDescriptor> + '_ {
        self.strategies
            .iter()
            .filter_map(|strategy| match strategy {
                CompiledDetectionStrategy::Url { input, .. } => Some(input.descriptor()),
                CompiledDetectionStrategy::Http { .. }
                | CompiledDetectionStrategy::Browser { .. } => None,
            })
    }

    pub fn input_url_pattern_descriptors(
        &self,
    ) -> impl Iterator<Item = &'static DetectionShapeDescriptor> + '_ {
        self.strategies.iter().flat_map(|strategy| match strategy {
            CompiledDetectionStrategy::Url {
                input: CompiledUrlInput::PatternAlternatives(alternatives),
                ..
            } => alternatives
                .iter()
                .map(CompiledUrlAlternative::descriptor)
                .collect::<Vec<_>>(),
            CompiledDetectionStrategy::Url {
                input: CompiledUrlInput::AbsoluteUrl,
                ..
            }
            | CompiledDetectionStrategy::Http { .. }
            | CompiledDetectionStrategy::Browser { .. } => Vec::new(),
        })
    }
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub enum CompiledDetectionStrategy {
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
    pub fn key(&self) -> &str {
        match self {
            Self::Url { key, .. } | Self::Http { key, .. } | Self::Browser { key, .. } => key,
        }
    }

    const fn descriptor(&self) -> &'static DetectionShapeDescriptor {
        match self {
            Self::Url { .. } => &DETECTION_URL_DESCRIPTOR,
            Self::Http { .. } => &DETECTION_HTTP_DESCRIPTOR,
            Self::Browser { .. } => &DETECTION_BROWSER_DESCRIPTOR,
        }
    }
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub enum CompiledUrlInput {
    PatternAlternatives(Vec<CompiledUrlAlternative>),
    AbsoluteUrl,
}

impl CompiledUrlInput {
    const fn descriptor(&self) -> &'static DetectionShapeDescriptor {
        match self {
            Self::PatternAlternatives(_) => &DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR,
            Self::AbsoluteUrl => &DETECTION_URL_ABSOLUTE_DESCRIPTOR,
        }
    }
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct CompiledUrlAlternative {
    pub pattern: CompiledNamedPattern,
}

impl CompiledUrlAlternative {
    const fn descriptor(&self) -> &'static DetectionShapeDescriptor {
        &DETECTION_INPUT_URL_PATTERN_DESCRIPTOR
    }
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

macro_rules! detection_variant_witness {
    ($name:ident,$variant:ident) => {
        fn $name() {
            fn check(v: &CompiledDetectionStrategy) {
                if let CompiledDetectionStrategy::$variant { .. } = v {}
            }
            let _ = check as fn(&CompiledDetectionStrategy);
        }
    };
    ($name:ident,$variant:ident,$field:ident) => {
        fn $name() {
            fn check(v: &CompiledDetectionStrategy) {
                if let CompiledDetectionStrategy::$variant { $field, .. } = v {
                    let _ = $field;
                }
            }
            let _ = check as fn(&CompiledDetectionStrategy);
        }
    };
}
detection_variant_witness!(witness_detection_url, Url);
detection_variant_witness!(witness_detection_url_key, Url, key);
detection_variant_witness!(witness_detection_url_input, Url, input);
fn witness_url_patterns() {
    fn check(v: &CompiledUrlInput) {
        if let CompiledUrlInput::PatternAlternatives(values) = v {
            let _ = values;
        }
    }
    let _ = check as fn(&CompiledUrlInput);
}
fn witness_url_alternatives() {
    fn check(v: &CompiledUrlInput) {
        if let CompiledUrlInput::PatternAlternatives(alternatives) = v {
            let _ = alternatives;
        }
    }
    let _ = check as fn(&CompiledUrlInput);
}
fn witness_url_pattern_entry() {
    fn check(v: &CompiledUrlAlternative) {
        let _ = &v.pattern;
    }
    let _ = check as fn(&CompiledUrlAlternative);
}
fn witness_url_pattern_pattern() {
    fn check(v: &CompiledUrlAlternative) {
        let _ = v.pattern.authored_pattern();
    }
    let _ = check as fn(&CompiledUrlAlternative);
}
fn witness_url_pattern_captures() {
    fn check(v: &CompiledUrlAlternative) {
        let _ = v.pattern.capture_keys();
    }
    let _ = check as fn(&CompiledUrlAlternative);
}
fn witness_absolute_url() {
    fn check(v: &CompiledUrlInput) {
        if let CompiledUrlInput::AbsoluteUrl = v {}
    }
    let _ = check as fn(&CompiledUrlInput);
}
detection_variant_witness!(witness_detection_http, Http);
detection_variant_witness!(witness_detection_http_key, Http, key);
detection_variant_witness!(witness_detection_http_fetch, Http, fetch);
detection_variant_witness!(witness_detection_http_status, Http, expect_status);
detection_variant_witness!(witness_detection_http_contains, Http, contains);
detection_variant_witness!(witness_detection_http_regex, Http, acceptance_regex);
detection_variant_witness!(witness_detection_http_captures, Http, captures);
detection_variant_witness!(witness_detection_http_evidence, Http, evidence);
detection_variant_witness!(witness_detection_browser, Browser);
detection_variant_witness!(witness_detection_browser_key, Browser, key);
fn witness_detection_browser_fetch() {
    fn check(v: &CompiledDetectionStrategy) {
        if let CompiledDetectionStrategy::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
            ..
        } = v
        {
            let _ = (url, timeout_ms, waits, interactions);
        }
    }
    let _ = check as fn(&CompiledDetectionStrategy);
}
detection_variant_witness!(witness_detection_browser_contains, Browser, contains);
detection_variant_witness!(witness_detection_browser_regex, Browser, acceptance_regex);
detection_variant_witness!(witness_detection_browser_captures, Browser, captures);
detection_variant_witness!(witness_detection_browser_evidence, Browser, evidence);

pub(crate) fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::{Keyed, ParentOption, Tagged},
        CompiledRegistration,
        Family::Detection,
        Owner::{D02, D03},
        PrimitiveContext::Detection as DetectionContext,
    };
    macro_rules! row {
        ($key:literal,$owner:expr,$shape:expr,$identity:literal,$witness:expr) => {
            CompiledRegistration {
                family: Detection,
                key: $key,
                contexts: &[DetectionContext],
                owner: $owner,
                canonical_file: "src-tauri/crates/source-profile-dsl/src/detection/plan.rs",
                shape: $shape,
                compiled_identity: $identity,
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    vec![
        row!(
            "url",
            D02,
            Tagged,
            "CompiledDetectionStrategy::Url",
            witness_detection_url
        ),
        row!(
            "url.key",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Url.key",
            witness_detection_url_key
        ),
        row!(
            "url.input",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Url.input",
            witness_detection_url_input
        ),
        row!(
            "pattern_alternatives",
            D02,
            Tagged,
            "CompiledUrlInput::PatternAlternatives",
            witness_url_patterns
        ),
        row!(
            "pattern_alternatives.alternatives",
            D02,
            ParentOption,
            "CompiledUrlInput::PatternAlternatives.0",
            witness_url_alternatives
        ),
        row!(
            "input_url_pattern",
            D02,
            Keyed,
            "CompiledUrlAlternative.pattern",
            witness_url_pattern_entry
        ),
        row!(
            "input_url_pattern.pattern",
            D02,
            ParentOption,
            "CompiledUrlAlternative.pattern.authored_pattern",
            witness_url_pattern_pattern
        ),
        row!(
            "input_url_pattern.captures",
            D02,
            ParentOption,
            "CompiledUrlAlternative.pattern.capture_keys",
            witness_url_pattern_captures
        ),
        row!(
            "absolute_url",
            D02,
            Tagged,
            "CompiledUrlInput::AbsoluteUrl",
            witness_absolute_url
        ),
        row!(
            "http",
            D02,
            Tagged,
            "CompiledDetectionStrategy::Http",
            witness_detection_http
        ),
        row!(
            "http.key",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.key",
            witness_detection_http_key
        ),
        row!(
            "http.fetch",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.fetch",
            witness_detection_http_fetch
        ),
        row!(
            "http.expectStatus",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.expect_status",
            witness_detection_http_status
        ),
        row!(
            "http.contains",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.contains",
            witness_detection_http_contains
        ),
        row!(
            "http.regex",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.acceptance_regex",
            witness_detection_http_regex
        ),
        row!(
            "http.captures",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.captures",
            witness_detection_http_captures
        ),
        row!(
            "http.evidence",
            D02,
            ParentOption,
            "CompiledDetectionStrategy::Http.evidence",
            witness_detection_http_evidence
        ),
        row!(
            "browser",
            D03,
            Tagged,
            "CompiledDetectionStrategy::Browser",
            witness_detection_browser
        ),
        row!(
            "browser.key",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.key",
            witness_detection_browser_key
        ),
        row!(
            "browser.fetch",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.{url,timeout_ms,waits,interactions}",
            witness_detection_browser_fetch
        ),
        row!(
            "browser.contains",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.contains",
            witness_detection_browser_contains
        ),
        row!(
            "browser.regex",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.acceptance_regex",
            witness_detection_browser_regex
        ),
        row!(
            "browser.captures",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.captures",
            witness_detection_browser_captures
        ),
        row!(
            "browser.evidence",
            D03,
            ParentOption,
            "CompiledDetectionStrategy::Browser.evidence",
            witness_detection_browser_evidence
        ),
    ]
}
