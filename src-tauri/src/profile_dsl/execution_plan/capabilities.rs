use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::fetch::{
    deserialize_browser_fetch_timeout, deserialize_browser_interaction_count,
    deserialize_browser_wait_after, deserialize_browser_wait_timeout,
    deserialize_non_empty_selector, BrowserInteraction, BrowserWait, MAX_BROWSER_FETCH_TIMEOUT_MS,
    MAX_BROWSER_INTERACTION_COUNT, MAX_BROWSER_WAIT_AFTER_MS, MAX_BROWSER_WAIT_TIMEOUT_MS,
};
use crate::profile_dsl::documents::Fetch;
use crate::profile_dsl::primitives::fetch::http::{compile_http_fetch, CompiledHttpFetch};
use crate::profile_dsl::template::{
    compile_template, descriptor_for_placement, CompiledTemplate, TemplateAdmissionKeys,
    TemplatePlacement,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionPlanFetch {
    Http(CompiledHttpFetch),
    Browser {
        url: CompiledTemplate,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_fetch_timeout"
        )]
        timeout_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        waits: Vec<ExecutionPlanBrowserWait>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        interactions: Vec<ExecutionPlanBrowserInteraction>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionPlanBrowserWait {
    Selector {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
    NetworkIdle {
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionPlanBrowserInteraction {
    ClickIfVisible {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionPlanBuildError {
    pub path: String,
    pub code: &'static str,
    pub message: String,
    pub details: serde_json::Value,
}

impl ExecutionPlanBuildError {
    pub(super) fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code: "compiled_execution_plan_invariant_violation",
            message: message.into(),
            details: serde_json::json!({ "invariant": "strict_execution_plan" }),
        }
    }

    pub(super) fn transform(
        path: impl Into<String>,
        error: crate::profile_dsl::primitives::transform::CompileTransformError,
    ) -> Self {
        let code = match error.kind {
            crate::profile_dsl::primitives::transform::CompileTransformErrorKind::EmptySeparator => {
                "transform_empty_separator"
            }
            crate::profile_dsl::primitives::transform::CompileTransformErrorKind::InvalidRegex => {
                "transform_invalid_regex"
            }
        };
        Self {
            path: format!("{}/transforms/{}", path.into(), error.transform_index),
            code,
            message: error.message,
            details: serde_json::json!({ "transformIndex": error.transform_index }),
        }
    }
}

pub(crate) fn compile_fetch(
    fetch: &Fetch,
    path: &str,
    placement: TemplatePlacement,
    keys: &TemplateAdmissionKeys,
) -> Result<ExecutionPlanFetch, ExecutionPlanBuildError> {
    let descriptor = descriptor_for_placement(placement, keys);
    let (header_descriptor, body_descriptor) = match placement {
        TemplatePlacement::DiscoveryHttpUrl => (
            descriptor_for_placement(TemplatePlacement::DiscoveryHttpHeader, keys),
            descriptor_for_placement(TemplatePlacement::DiscoveryHttpBody, keys),
        ),
        TemplatePlacement::DetailHttpUrl => (
            descriptor_for_placement(TemplatePlacement::DetailHttpHeader, keys),
            descriptor_for_placement(TemplatePlacement::DetailHttpBody, keys),
        ),
        _ => (descriptor.clone(), descriptor.clone()),
    };
    match fetch {
        Fetch::Http {
            method,
            url,
            headers,
            body,
            timeout_ms,
        } => compile_http_fetch(
            *method,
            url,
            headers.as_ref(),
            body.as_ref(),
            *timeout_ms,
            &descriptor,
            &header_descriptor,
            &body_descriptor,
        )
        .map(ExecutionPlanFetch::Http)
        .map_err(|error| ExecutionPlanBuildError {
            path: format!("{path}{}", error.path),
            code: error.code,
            message: error.message,
            details: serde_json::json!({ "invariant": "canonical_http_fetch" }),
        }),
        Fetch::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
        } => Ok(ExecutionPlanFetch::Browser {
            url: compile_template(url, &descriptor).map_err(|error| {
                ExecutionPlanBuildError::new(format!("{path}/url"), error.to_string())
            })?,
            timeout_ms: require_bounded(
                *timeout_ms,
                MAX_BROWSER_FETCH_TIMEOUT_MS,
                &format!("{path}/timeoutMs"),
            )?,
            waits: waits
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(index, wait)| compile_browser_wait(wait, &format!("{path}/waits/{index}")))
                .collect::<Result<Vec<_>, _>>()?,
            interactions: interactions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(index, interaction)| {
                    compile_browser_interaction(
                        interaction,
                        &format!("{path}/interactions/{index}"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn compile_browser_wait(
    wait: &BrowserWait,
    path: &str,
) -> Result<ExecutionPlanBrowserWait, ExecutionPlanBuildError> {
    match wait {
        BrowserWait::Selector {
            selector,
            timeout_ms,
        } => Ok(ExecutionPlanBrowserWait::Selector {
            selector: require_non_empty(selector, &format!("{path}/selector"))?,
            timeout_ms: require_bounded(
                *timeout_ms,
                MAX_BROWSER_WAIT_TIMEOUT_MS,
                &format!("{path}/timeoutMs"),
            )?,
        }),
        BrowserWait::NetworkIdle { timeout_ms } => Ok(ExecutionPlanBrowserWait::NetworkIdle {
            timeout_ms: require_bounded(
                *timeout_ms,
                MAX_BROWSER_WAIT_TIMEOUT_MS,
                &format!("{path}/timeoutMs"),
            )?,
        }),
    }
}

fn compile_browser_interaction(
    interaction: &BrowserInteraction,
    path: &str,
) -> Result<ExecutionPlanBrowserInteraction, ExecutionPlanBuildError> {
    let compile_fields = |selector: &str, max_count: u64, wait_after_ms: Option<u64>| {
        Ok((
            require_non_empty(selector, &format!("{path}/selector"))?,
            require_bounded(
                max_count,
                MAX_BROWSER_INTERACTION_COUNT,
                &format!("{path}/maxCount"),
            )?,
            require_optional_max(
                wait_after_ms,
                MAX_BROWSER_WAIT_AFTER_MS,
                &format!("{path}/waitAfterMs"),
            )?,
        ))
    };
    match interaction {
        BrowserInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => {
            let (selector, max_count, wait_after_ms) =
                compile_fields(selector, *max_count, *wait_after_ms)?;
            Ok(ExecutionPlanBrowserInteraction::ClickIfVisible {
                selector,
                max_count,
                wait_after_ms,
            })
        }
        BrowserInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => {
            let (selector, max_count, wait_after_ms) =
                compile_fields(selector, *max_count, *wait_after_ms)?;
            Ok(ExecutionPlanBrowserInteraction::ClickUntilGone {
                selector,
                max_count,
                wait_after_ms,
            })
        }
    }
}

fn require_bounded(value: u64, max: u64, path: &str) -> Result<u64, ExecutionPlanBuildError> {
    if (1..=max).contains(&value) {
        Ok(value)
    } else {
        Err(ExecutionPlanBuildError::new(
            path,
            format!("bound must be between 1 and {max}"),
        ))
    }
}

fn require_optional_max(
    value: Option<u64>,
    max: u64,
    path: &str,
) -> Result<Option<u64>, ExecutionPlanBuildError> {
    if value.map_or(true, |value| value <= max) {
        Ok(value)
    } else {
        Err(ExecutionPlanBuildError::new(
            path,
            format!("bound must not exceed {max}"),
        ))
    }
}

fn require_non_empty(value: &str, path: &str) -> Result<String, ExecutionPlanBuildError> {
    if value.trim().is_empty() {
        Err(ExecutionPlanBuildError::new(
            path,
            "selector must not be empty",
        ))
    } else {
        Ok(value.to_string())
    }
}
