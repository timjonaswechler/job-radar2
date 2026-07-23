use super::support::{render_template, TemplateRuntimeContext};
use super::*;
use crate::profile_dsl::primitives::fetch::http::{
    execute_http_fetch, HttpFetchExecutionError, HttpFetchOverlay,
};

#[derive(Clone, Copy)]
pub(super) enum DetailBrowserBackend<'a> {
    Canonical(DetailBrowserAdapter<'a>),
    BrowserFree,
}

pub(super) async fn fetch_strategy_document<F>(
    fetcher: &F,
    browser: &DetailBrowserBackend<'_>,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<CompleteParseText>, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    let values = TemplateRuntimeContext {
        source_config,
        source_name,
        posting,
        posting_meta: &posting.posting_meta,
        captures,
    };
    match fetch {
        ExecutionPlanFetch::Http(fetch) => match execute_http_fetch(
            fetcher,
            fetch,
            &values,
            HttpFetchOverlay::default(),
            authored_charset,
            crate::profile_dsl::primitives::fetch::http::HttpStatusPolicy::RequireSuccess,
            execution_context,
        )
        .await
        {
            Ok(response) => Ok(Some(CompleteParseText::DecodedHttp(response.body))),
            Err(HttpFetchExecutionError::Cancelled) => Err(fetch_cancellation(
                strategy_index,
                strategy_key,
                CancellationOperation::Fetch,
            )),
            Err(HttpFetchExecutionError::BudgetExhausted) => Ok(None),
            Err(HttpFetchExecutionError::NonSuccessStatus { status }) => {
                diagnostics.push(runtime_error(
                    "http_fetch_non_success_status",
                    "HTTP fetch returned a non-success status",
                    format!("{base_path}/fetch"),
                    strategy_key,
                    json!({ "method": fetch.method.label(), "status": status }),
                ));
                Ok(None)
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
                Ok(None)
            }
            Err(HttpFetchExecutionError::Render(error)) => {
                diagnostics.push(runtime_error(
                    error.code,
                    error.message,
                    format!("{base_path}/fetch{}", error.path),
                    strategy_key,
                    json!({}),
                ));
                Ok(None)
            }
        },
        ExecutionPlanFetch::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
        } => {
            fetch_browser_strategy_document(
                browser,
                url,
                *timeout_ms,
                waits,
                interactions,
                &values,
                base_path,
                strategy_key,
                strategy_index,
                diagnostics,
                execution_context,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn fetch_browser_strategy_document(
    browser: &DetailBrowserBackend<'_>,
    url: &crate::profile_dsl::template::CompiledTemplate,
    timeout_ms: u64,
    waits: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserWait],
    interactions: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserInteraction],
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<CompleteParseText>, TypedCancellation>
where
{
    let rendered_url = match render_template(url, context) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({}),
            ));
            return Ok(None);
        }
    };
    match browser {
        DetailBrowserBackend::Canonical(adapter) => {
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
                        control: execution_context,
                    })
                    .await,
                diagnostics,
            )
        }
        DetailBrowserBackend::BrowserFree => {
            unreachable!("Browser-free phase construction was validated before execution")
        }
    }
}

fn project_browser_fetch(
    projection: BrowserPhaseFetchProjection,
    diagnostics: &mut Diagnostics,
) -> Result<Option<CompleteParseText>, TypedCancellation> {
    match projection {
        BrowserPhaseFetchProjection::Rendered(body) => {
            Ok(Some(CompleteParseText::BrowserRendered(body)))
        }
        BrowserPhaseFetchProjection::AttemptFailed { diagnostic, .. }
        | BrowserPhaseFetchProjection::PhaseFatal(diagnostic) => {
            diagnostics.push(diagnostic);
            Ok(None)
        }
        BrowserPhaseFetchProjection::AllowanceStopped => Ok(None),
        BrowserPhaseFetchProjection::Cancelled(cancellation) => Err(cancellation),
    }
}

fn fetch_cancellation(
    strategy_index: usize,
    strategy_key: Option<&str>,
    operation: CancellationOperation,
) -> TypedCancellation {
    TypedCancellation::strategy(
        RuntimePhase::Detail,
        strategy_index,
        strategy_key.expect("compiled strategy has a key"),
        operation,
    )
}
