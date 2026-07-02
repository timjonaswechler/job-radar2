use super::support::{push_browser_fetch_diagnostic, render_source_config_template};
use super::*;

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
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
                source_config,
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
                source_config,
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
    source_config: &SourceConfig,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    let method = method.unwrap_or(HttpMethod::Get);
    if method != HttpMethod::Get {
        diagnostics.push(runtime_error(
            "unsupported_http_method",
            "postingDiscovery runtime slice supports only HTTP GET",
            format!("{base_path}/fetch/method"),
            strategy_key,
            json!({ "supportedMethod": "GET" }),
        ));
        return None;
    }

    let rendered_url = match render_source_config_template(url, source_config) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "fetch_url_template_failed",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let request = PostingDiscoveryFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: headers.cloned().unwrap_or_default(),
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
    source_config: &SourceConfig,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let rendered_url = match render_source_config_template(url, source_config) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "fetch_url_template_failed",
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
        Ok(ProfileBrowserFetchResponse { body }) => Some(PostingDiscoveryFetchResponse { body }),
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
