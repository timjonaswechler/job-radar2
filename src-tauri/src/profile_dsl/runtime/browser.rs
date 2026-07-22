use std::{future::Future, path::PathBuf, pin::Pin};

use crate::profile_dsl::execution_plan::capabilities::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
};

use super::cancellation::RuntimeExecutionContext;

pub type BoxedProfileBrowserFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
            + Send
            + 'a,
    >,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileBrowserFetchRequest {
    pub url: String,
    pub timeout_ms: u64,
    pub waits: Vec<ExecutionPlanBrowserWait>,
    pub interactions: Vec<ExecutionPlanBrowserInteraction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileBrowserFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileBrowserFetchError {
    pub kind: ProfileBrowserFetchErrorKind,
    pub message: String,
}

impl ProfileBrowserFetchError {
    pub fn new(kind: ProfileBrowserFetchErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileBrowserFetchErrorKind {
    RuntimeUnavailable,
    NavigationFailed,
    WaitTimeout { wait_index: Option<usize> },
    InteractionFailed { interaction_index: Option<usize> },
    Cancelled,
    RenderTimeout,
    ContentReadFailed,
}

pub trait ProfileBrowserClient {
    fn render<'a>(&'a self, request: ProfileBrowserFetchRequest) -> BoxedProfileBrowserFuture<'a>;

    fn render_with_context<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> BoxedProfileBrowserFuture<'a>
    where
        Self: Sync,
    {
        Box::pin(async move {
            if context.is_cancelled() {
                return Err(profile_browser_cancelled_error());
            }
            tokio::select! {
                biased;
                _ = context.cancelled() => Err(profile_browser_cancelled_error()),
                result = self.render(request) => {
                    if context.is_cancelled() { Err(profile_browser_cancelled_error()) } else { result }
                },
                _ = context.deadline_reached() => {
                    if context.is_cancelled() {
                        Err(profile_browser_cancelled_error())
                    } else {
                        context.mark_deadline();
                        Err(ProfileBrowserFetchError::new(ProfileBrowserFetchErrorKind::RenderTimeout, "Profile DSL browser execution exceeded the cumulative phase deadline"))
                    }
                }
            }
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct UnavailableProfileBrowserClient;

impl ProfileBrowserClient for UnavailableProfileBrowserClient {
    fn render<'a>(&'a self, _request: ProfileBrowserFetchRequest) -> BoxedProfileBrowserFuture<'a> {
        Box::pin(async move {
            Err(ProfileBrowserFetchError::new(
                ProfileBrowserFetchErrorKind::RuntimeUnavailable,
                "browser runtime client is not configured",
            ))
        })
    }
}

#[derive(Clone, Debug)]
pub struct ManagedProfileBrowserClient {
    runtime_dir: PathBuf,
}

impl ManagedProfileBrowserClient {
    pub fn new(runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            runtime_dir: runtime_dir.into(),
        }
    }
}

impl ProfileBrowserClient for ManagedProfileBrowserClient {
    fn render<'a>(&'a self, request: ProfileBrowserFetchRequest) -> BoxedProfileBrowserFuture<'a> {
        self.render_with_context(request, RuntimeExecutionContext::uncancellable())
    }

    fn render_with_context<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> BoxedProfileBrowserFuture<'a> {
        Box::pin(async move {
            if context.is_cancelled() {
                return Err(profile_browser_cancelled_error());
            }
            let spec = crate::browser_runtime::current_runtime_spec();
            let status = crate::browser_runtime::status_for_runtime_dir(
                &self.runtime_dir,
                spec.as_ref(),
                false,
            );
            if status.status != crate::browser_runtime::BrowserRuntimeState::Installed {
                let status_detail = status
                    .error
                    .as_deref()
                    .unwrap_or("managed browser runtime is not installed and ready");
                return Err(ProfileBrowserFetchError::new(
                    ProfileBrowserFetchErrorKind::RuntimeUnavailable,
                    format!(
                        "browser runtime unavailable: status {:?}: {status_detail}",
                        status.status
                    ),
                ));
            }

            let executable_path = status.executable_path.as_deref().ok_or_else(|| {
                ProfileBrowserFetchError::new(
                    ProfileBrowserFetchErrorKind::RuntimeUnavailable,
                    "browser runtime unavailable: installed managed browser runtime has no executable path",
                )
            })?;
            let executable_path = PathBuf::from(executable_path);
            if context.is_cancelled() {
                return Err(profile_browser_cancelled_error());
            }
            let runtime_request = crate::browser_runtime::BrowserRuntimeRenderRequest {
                url: request.url,
                timeout_ms: request.timeout_ms,
                waits: request
                    .waits
                    .into_iter()
                    .map(|wait| match wait {
                        ExecutionPlanBrowserWait::Selector {
                            selector,
                            timeout_ms,
                        } => crate::browser_runtime::BrowserRuntimeWait::Selector {
                            selector: Some(selector),
                            timeout_ms,
                        },
                        ExecutionPlanBrowserWait::NetworkIdle { timeout_ms } => {
                            crate::browser_runtime::BrowserRuntimeWait::NetworkIdle {
                                selector: None,
                                timeout_ms,
                            }
                        }
                    })
                    .collect(),
                interactions: request
                    .interactions
                    .into_iter()
                    .map(|interaction| match interaction {
                        ExecutionPlanBrowserInteraction::ClickIfVisible {
                            selector,
                            max_count,
                            wait_after_ms,
                        } => crate::browser_runtime::BrowserRuntimeInteraction::ClickIfVisible {
                            selector,
                            max_count,
                            wait_after_ms,
                        },
                        ExecutionPlanBrowserInteraction::ClickUntilGone {
                            selector,
                            max_count,
                            wait_after_ms,
                        } => crate::browser_runtime::BrowserRuntimeInteraction::ClickUntilGone {
                            selector,
                            max_count,
                            wait_after_ms,
                        },
                    })
                    .collect(),
            };

            crate::browser_runtime::render_page_html_with_actions_and_context(
                &executable_path,
                &self.runtime_dir,
                runtime_request,
                context,
            )
            .await
            .map(|body| ProfileBrowserFetchResponse { body })
            .map_err(map_browser_runtime_error)
        })
    }
}

fn profile_browser_cancelled_error() -> ProfileBrowserFetchError {
    ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::Cancelled,
        "Profile DSL browser execution cancelled",
    )
}

fn map_browser_runtime_error(
    error: crate::browser_runtime::BrowserRuntimeRenderError,
) -> ProfileBrowserFetchError {
    let kind = match error.kind {
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::RuntimeUnavailable => {
            ProfileBrowserFetchErrorKind::RuntimeUnavailable
        }
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::NavigationFailed => {
            ProfileBrowserFetchErrorKind::NavigationFailed
        }
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::WaitTimeout { wait_index } => {
            ProfileBrowserFetchErrorKind::WaitTimeout { wait_index }
        }
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::InteractionFailed {
            interaction_index,
        } => ProfileBrowserFetchErrorKind::InteractionFailed { interaction_index },
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::Cancelled => {
            ProfileBrowserFetchErrorKind::Cancelled
        }
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::RenderTimeout => {
            ProfileBrowserFetchErrorKind::RenderTimeout
        }
        crate::browser_runtime::BrowserRuntimeRenderErrorKind::ContentReadFailed => {
            ProfileBrowserFetchErrorKind::ContentReadFailed
        }
    };
    ProfileBrowserFetchError::new(kind, error.message)
}
