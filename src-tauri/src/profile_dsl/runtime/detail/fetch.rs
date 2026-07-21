use super::support::{push_browser_fetch_diagnostic, render_template, TemplateRuntimeContext};
use super::*;
use crate::profile_dsl::primitives::fetch::http::{
    execute_http_fetch, HttpFetchExecutionError, HttpFetchOverlay,
};

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    authored_charset: Option<&str>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<CompleteParseText>, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
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
async fn fetch_browser_strategy_document<B>(
    browser: &B,
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
    B: ProfileBrowserClient + Sync + ?Sized,
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
    if execution_context.is_cancelled() {
        return Err(fetch_cancellation(
            strategy_index,
            strategy_key,
            CancellationOperation::Browser,
        ));
    }
    if execution_context
        .debit(AllowanceCharge {
            requests: 1,
            ..AllowanceCharge::default()
        })
        .is_err()
    {
        return Ok(None);
    }
    if execution_context.is_cancelled() {
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
    match browser
        .render_with_context(request, execution_context)
        .await
    {
        Ok(ProfileBrowserFetchResponse { body }) => {
            Ok(Some(CompleteParseText::BrowserRendered(body)))
        }
        Err(error) if error.kind == ProfileBrowserFetchErrorKind::Cancelled => Err(
            fetch_cancellation(strategy_index, strategy_key, CancellationOperation::Browser),
        ),
        Err(error) => {
            push_browser_fetch_diagnostic(
                error,
                &rendered_url,
                base_path,
                strategy_key,
                diagnostics,
            );
            Ok(None)
        }
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
