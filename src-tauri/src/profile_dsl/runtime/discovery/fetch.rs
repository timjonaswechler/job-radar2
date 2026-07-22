use super::support::{
    push_browser_fetch_diagnostic, render_source_config_template, DiscoveryTemplateValues,
};
use super::*;
use crate::profile_dsl::primitives::{
    fetch::http::{execute_http_fetch, HttpFetchExecutionError, HttpFetchOverlay},
    pagination::PaginationOverlay,
};

pub(super) enum DiscoveryFetchOutcome {
    Complete(CompleteParseText),
    ExecutionFailed,
}

pub(super) enum DiscoveryBrowserBackend<'a, B: ?Sized> {
    Legacy(&'a B),
    Canonical(DiscoveryBrowserAdapter<'a>),
    BrowserFree,
}

impl<B: ?Sized> Clone for DiscoveryBrowserBackend<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: ?Sized> Copy for DiscoveryBrowserBackend<'_, B> {}

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_, B>,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<DiscoveryFetchOutcome, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fetch_strategy_document_with_overlay(
        fetcher,
        browser,
        fetch,
        authored_charset,
        source_config,
        source_name,
        &PaginationOverlay::default(),
        base_path,
        strategy_key,
        strategy_index,
        diagnostics,
        context,
    )
    .await
}

pub(super) async fn fetch_strategy_document_with_overlay<F, B>(
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_, B>,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    overlay: &PaginationOverlay,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<DiscoveryFetchOutcome, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fetch_strategy_document_with_options(
        fetcher,
        browser,
        fetch,
        authored_charset,
        source_config,
        source_name,
        None,
        overlay,
        base_path,
        strategy_key,
        strategy_index,
        diagnostics,
        context,
    )
    .await
}

pub(super) async fn fetch_strategy_document_at_url<F, B>(
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_, B>,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    url_override: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<DiscoveryFetchOutcome, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fetch_strategy_document_with_options(
        fetcher,
        browser,
        fetch,
        authored_charset,
        source_config,
        source_name,
        Some(url_override),
        &PaginationOverlay::default(),
        base_path,
        strategy_key,
        strategy_index,
        diagnostics,
        context,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn fetch_strategy_document_with_options<F, B>(
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_, B>,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    url_override: Option<&str>,
    overlay: &PaginationOverlay,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<DiscoveryFetchOutcome, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let query_params = overlay
        .query
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let json_body_params = overlay
        .json_body
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect::<Vec<_>>();
    match fetch {
        ExecutionPlanFetch::Http(fetch) => {
            let values = DiscoveryTemplateValues {
                source_config,
                source_name,
            };
            match execute_http_fetch(
                fetcher,
                fetch,
                &values,
                HttpFetchOverlay {
                    url_override,
                    query_params: &query_params,
                    json_body_params: &json_body_params,
                },
                authored_charset,
                context,
            )
            .await
            {
                Ok(response) => Ok(DiscoveryFetchOutcome::Complete(
                    CompleteParseText::DecodedHttp(response.body),
                )),
                Err(HttpFetchExecutionError::Cancelled) => Err(fetch_cancellation(
                    strategy_index,
                    strategy_key,
                    CancellationOperation::Fetch,
                )),
                Err(HttpFetchExecutionError::BudgetExhausted) => {
                    Ok(DiscoveryFetchOutcome::ExecutionFailed)
                }
                Err(HttpFetchExecutionError::NonSuccessStatus { status }) => {
                    diagnostics.push(runtime_error(
                        "http_fetch_non_success_status",
                        "HTTP fetch returned a non-success status",
                        format!("{base_path}/fetch"),
                        strategy_key,
                        json!({ "method": fetch.method.label(), "status": status }),
                    ));
                    Ok(DiscoveryFetchOutcome::ExecutionFailed)
                }
                Err(HttpFetchExecutionError::Acquisition(error))
                    if error.kind == ProfileHttpFailureKind::Cancelled =>
                {
                    Err(fetch_cancellation(
                        strategy_index,
                        strategy_key,
                        CancellationOperation::Fetch,
                    ))
                }
                Err(HttpFetchExecutionError::Acquisition(error)) => {
                    diagnostics.push(runtime_error(
                        "fetch_failed",
                        "HTTP fetch failed",
                        format!("{base_path}/fetch"),
                        strategy_key,
                        json!({ "method": fetch.method.label(), "kind": format!("{:?}", error.kind), "admittedBytes": error.admitted_bytes }),
                    ));
                    Ok(DiscoveryFetchOutcome::ExecutionFailed)
                }
                Err(HttpFetchExecutionError::Render(error)) => {
                    diagnostics.push(runtime_error(
                        error.code,
                        error.message,
                        format!("{base_path}/fetch{}", error.path),
                        strategy_key,
                        json!({}),
                    ));
                    Ok(DiscoveryFetchOutcome::ExecutionFailed)
                }
            }
        }
        ExecutionPlanFetch::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
        } => {
            if !json_body_params.is_empty() {
                diagnostics.push(runtime_error(
                    "invalid_json_body_overlay",
                    "json_body overlay is unavailable for Browser Fetch",
                    format!("{base_path}/fetch"),
                    strategy_key,
                    json!({}),
                ));
                return Ok(DiscoveryFetchOutcome::ExecutionFailed);
            }
            fetch_browser_strategy_document(
                browser,
                url,
                *timeout_ms,
                waits,
                interactions,
                source_config,
                source_name,
                url_override,
                &query_params,
                base_path,
                strategy_key,
                strategy_index,
                diagnostics,
                context,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn fetch_browser_strategy_document<B>(
    browser: &DiscoveryBrowserBackend<'_, B>,
    url: &crate::profile_dsl::template::CompiledTemplate,
    timeout_ms: u64,
    waits: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserWait],
    interactions: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserInteraction],
    source_config: &SourceConfig,
    source_name: &str,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<DiscoveryFetchOutcome, TypedCancellation>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let rendered_url = match url_override {
        Some(url) => append_query_params(url.to_string(), query_params),
        None => match render_source_config_template(url, source_config, source_name) {
            Ok(url) => append_query_params(url, query_params),
            Err(message) => {
                diagnostics.push(runtime_error(
                    "fetch_url_template_failed",
                    format!("Fetch URL template could not be rendered: {message}"),
                    format!("{base_path}/fetch/url"),
                    strategy_key,
                    json!({}),
                ));
                return Ok(DiscoveryFetchOutcome::ExecutionFailed);
            }
        },
    };
    match browser {
        DiscoveryBrowserBackend::Legacy(browser) => {
            if context.is_cancelled() {
                return Err(fetch_cancellation(
                    strategy_index,
                    strategy_key,
                    CancellationOperation::Browser,
                ));
            }
            if context
                .debit(AllowanceCharge {
                    requests: 1,
                    pages: u64::from(context.page_request()),
                    ..AllowanceCharge::default()
                })
                .is_err()
            {
                return Ok(DiscoveryFetchOutcome::ExecutionFailed);
            }
            if context.is_cancelled() {
                return Err(fetch_cancellation(
                    strategy_index,
                    strategy_key,
                    CancellationOperation::Browser,
                ));
            }
            let request = ProfileBrowserFetchRequest {
                url: rendered_url.clone(),
                timeout_ms,
                waits: waits.to_vec(),
                interactions: interactions.to_vec(),
            };
            match browser.render_with_context(request, context).await {
                Ok(ProfileBrowserFetchResponse { body }) => Ok(DiscoveryFetchOutcome::Complete(
                    CompleteParseText::BrowserRendered(body),
                )),
                Err(error) if error.kind == ProfileBrowserFetchErrorKind::Cancelled => {
                    Err(fetch_cancellation(
                        strategy_index,
                        strategy_key,
                        CancellationOperation::Browser,
                    ))
                }
                Err(error) => {
                    push_browser_fetch_diagnostic(
                        error,
                        &rendered_url,
                        base_path,
                        strategy_key,
                        diagnostics,
                    );
                    Ok(DiscoveryFetchOutcome::ExecutionFailed)
                }
            }
        }
        DiscoveryBrowserBackend::Canonical(adapter) => {
            let strategy_key = strategy_key
                .expect("compiled strategy has a key")
                .to_string();
            project_browser_fetch(
                adapter
                    .fetch(BrowserPhaseFetchInput {
                        target: rendered_url,
                        timeout_ms,
                        waits: waits.to_vec(),
                        interactions: interactions.to_vec(),
                        base_path: base_path.to_string(),
                        strategy_key,
                        strategy_index,
                        control: context,
                    })
                    .await,
                diagnostics,
            )
        }
        DiscoveryBrowserBackend::BrowserFree => {
            unreachable!("Browser-free phase construction was validated before execution")
        }
    }
}

fn project_browser_fetch(
    projection: BrowserPhaseFetchProjection,
    diagnostics: &mut Diagnostics,
) -> Result<DiscoveryFetchOutcome, TypedCancellation> {
    match projection {
        BrowserPhaseFetchProjection::Rendered(body) => Ok(DiscoveryFetchOutcome::Complete(
            CompleteParseText::BrowserRendered(body),
        )),
        BrowserPhaseFetchProjection::AttemptFailed(diagnostic)
        | BrowserPhaseFetchProjection::PhaseFatal(diagnostic) => {
            diagnostics.push(diagnostic);
            Ok(DiscoveryFetchOutcome::ExecutionFailed)
        }
        BrowserPhaseFetchProjection::AllowanceStopped => Ok(DiscoveryFetchOutcome::ExecutionFailed),
        BrowserPhaseFetchProjection::Cancelled(cancellation) => Err(cancellation),
    }
}

fn fetch_cancellation(
    strategy_index: usize,
    strategy_key: Option<&str>,
    operation: CancellationOperation,
) -> TypedCancellation {
    TypedCancellation::strategy(
        RuntimePhase::Discovery,
        strategy_index,
        strategy_key.expect("compiled strategy has a key"),
        operation,
    )
}

fn append_query_params(url: String, query_params: &[(&str, String)]) -> String {
    if query_params.is_empty() {
        return url;
    }

    let (without_fragment, fragment) = match url.split_once('#') {
        Some((prefix, suffix)) => (prefix, Some(suffix)),
        None => (url.as_str(), None),
    };
    let (path, query) = match without_fragment.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (without_fragment, None),
    };

    let replaced_names = query_params
        .iter()
        .map(|(name, _)| *name)
        .collect::<std::collections::BTreeSet<_>>();
    let mut pairs = query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let name = pair.split_once('=').map(|(name, _)| name).unwrap_or(pair);
            !replaced_names.contains(name)
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    pairs.extend(
        query_params
            .iter()
            .map(|(name, value)| format!("{name}={value}")),
    );

    let mut rendered = path.to_string();
    if !pairs.is_empty() {
        rendered.push('?');
        rendered.push_str(&pairs.join("&"));
    }
    if let Some(fragment) = fragment {
        rendered.push('#');
        rendered.push_str(fragment);
    }
    rendered
}
