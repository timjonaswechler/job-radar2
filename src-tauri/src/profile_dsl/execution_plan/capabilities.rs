use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::Fetch;
use crate::profile_dsl::primitives::fetch::browser::{
    compile_browser_fetch, deserialize_browser_fetch_timeout, BrowserCompileError,
    BrowserPrimitiveDescriptor,
};
pub use crate::profile_dsl::primitives::fetch::browser::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
};
use crate::profile_dsl::primitives::fetch::http::{compile_http_fetch, CompiledHttpFetch};
use crate::profile_dsl::template::{
    descriptor_for_placement, CompiledTemplate, TemplateAdmissionKeys, TemplatePlacement,
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

    fn browser(error: BrowserCompileError) -> Self {
        Self::new(error.path, error.message)
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
        } => browser_plan(
            compile_browser_fetch(
                url,
                *timeout_ms,
                waits.as_deref(),
                interactions.as_deref(),
                path,
                &descriptor,
            )
            .map_err(ExecutionPlanBuildError::browser)?,
        ),
    }
}

pub(crate) fn compile_browser_fetch_with_descriptor(
    fetch: &Fetch,
    path: &str,
    descriptor: &crate::profile_dsl::template::TemplateDescriptor,
) -> Result<ExecutionPlanFetch, ExecutionPlanBuildError> {
    let Fetch::Browser {
        url,
        timeout_ms,
        waits,
        interactions,
    } = fetch
    else {
        return Err(ExecutionPlanBuildError::new(
            path,
            "Detection Browser Strategy requires Browser Fetch",
        ));
    };
    browser_plan(
        compile_browser_fetch(
            url,
            *timeout_ms,
            waits.as_deref(),
            interactions.as_deref(),
            path,
            descriptor,
        )
        .map_err(ExecutionPlanBuildError::browser)?,
    )
}

fn browser_plan(
    (url, timeout_ms, waits, interactions): (
        CompiledTemplate,
        u64,
        Vec<ExecutionPlanBrowserWait>,
        Vec<ExecutionPlanBrowserInteraction>,
    ),
) -> Result<ExecutionPlanFetch, ExecutionPlanBuildError> {
    Ok(ExecutionPlanFetch::Browser {
        url,
        timeout_ms,
        waits,
        interactions,
    })
}

impl ExecutionPlanFetch {
    pub const fn browser_descriptor(&self) -> Option<&'static BrowserPrimitiveDescriptor> {
        match self {
            Self::Http(_) => None,
            Self::Browser { .. } => {
                Some(&crate::profile_dsl::primitives::fetch::browser::BROWSER_FETCH_DESCRIPTOR)
            }
        }
    }
}
