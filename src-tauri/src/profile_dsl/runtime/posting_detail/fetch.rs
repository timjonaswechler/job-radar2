use super::support::{push_browser_fetch_diagnostic, render_template, TemplateRuntimeContext};
use super::*;

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
where
    F: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let context = TemplateRuntimeContext {
        source_config,
        source_name,
        posting,
        posting_meta: &posting.posting_meta,
        captures,
    };

    match fetch {
        ExecutionPlanFetch::Http {
            method,
            url,
            headers,
            timeout_ms,
            ..
        } => {
            fetch_http_strategy_document(
                fetcher,
                *method,
                url,
                headers.as_ref(),
                *timeout_ms,
                &context,
                base_path,
                strategy_key,
                diagnostics,
            )
            .await
        }
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
                &context,
                base_path,
                strategy_key,
                diagnostics,
            )
            .await
        }
    }
}

async fn fetch_http_strategy_document<F>(
    fetcher: &F,
    method: Option<HttpMethod>,
    url: &str,
    headers: Option<&BTreeMap<String, String>>,
    timeout_ms: u64,
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
where
    F: PostingDetailFetcher + Sync + ?Sized,
{
    let method = method.unwrap_or(HttpMethod::Get);
    if method != HttpMethod::Get {
        diagnostics.push(runtime_error(
            "unsupported_http_method",
            "postingDetail runtime slice supports only HTTP GET",
            format!("{base_path}/fetch/method"),
            strategy_key,
            json!({ "supportedMethod": "GET" }),
        ));
        return None;
    }

    let rendered_url = match render_template(url, context) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let rendered_headers = match render_headers(headers, context) {
        Ok(headers) => headers,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch header template could not be rendered: {message}"),
                format!("{base_path}/fetch/headers"),
                strategy_key,
                json!({}),
            ));
            return None;
        }
    };

    let request = PostingDetailFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: rendered_headers,
        timeout_ms,
    };

    match fetcher.fetch(request).await {
        Ok(response) => Some(response),
        Err(error) => {
            diagnostics.push(runtime_error(
                "fetch_failed",
                format!(
                    "HTTP GET fetch failed for {rendered_url}: {}",
                    error.message
                ),
                format!("{base_path}/fetch"),
                strategy_key,
                json!({ "url": rendered_url, "error": error.message }),
            ));
            None
        }
    }
}

async fn fetch_browser_strategy_document<B>(
    browser: &B,
    url: &str,
    timeout_ms: u64,
    waits: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserWait],
    interactions: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserInteraction],
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
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
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let request = ProfileBrowserFetchRequest {
        url: rendered_url.clone(),
        timeout_ms,
        waits: waits.to_vec(),
        interactions: interactions.to_vec(),
    };

    match browser.render(request).await {
        Ok(ProfileBrowserFetchResponse { body }) => Some(PostingDetailFetchResponse { body }),
        Err(error) => {
            push_browser_fetch_diagnostic(
                error,
                &rendered_url,
                base_path,
                strategy_key,
                diagnostics,
            );
            None
        }
    }
}

fn render_headers(
    headers: Option<&BTreeMap<String, String>>,
    context: &TemplateRuntimeContext<'_>,
) -> Result<BTreeMap<String, String>, String> {
    let mut rendered = BTreeMap::new();
    for (name, value) in headers.into_iter().flatten() {
        rendered.insert(name.clone(), render_template(value, context)?);
    }
    Ok(rendered)
}
