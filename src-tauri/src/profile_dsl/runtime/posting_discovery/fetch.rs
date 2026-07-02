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
    fetch_strategy_document_with_query_params(
        fetcher,
        browser,
        fetch,
        source_config,
        &[],
        base_path,
        strategy_key,
        diagnostics,
    )
    .await
}

pub(super) async fn fetch_strategy_document_with_query_params<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    query_params: &[(&str, String)],
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fetch_strategy_document_with_url_options(
        fetcher,
        browser,
        fetch,
        source_config,
        None,
        query_params,
        base_path,
        strategy_key,
        diagnostics,
    )
    .await
}

pub(super) async fn fetch_strategy_document_at_url<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    url_override: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fetch_strategy_document_with_url_options(
        fetcher,
        browser,
        fetch,
        source_config,
        Some(url_override),
        &[],
        base_path,
        strategy_key,
        diagnostics,
    )
    .await
}

async fn fetch_strategy_document_with_url_options<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
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
                url_override,
                query_params,
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
                url_override,
                query_params,
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
    url_override: Option<&str>,
    query_params: &[(&str, String)],
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

    let rendered_url = match render_fetch_url(url, source_config, url_override, query_params) {
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
    url_override: Option<&str>,
    query_params: &[(&str, String)],
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let rendered_url = match render_fetch_url(url, source_config, url_override, query_params) {
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

fn render_fetch_url(
    url: &str,
    source_config: &SourceConfig,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
) -> Result<String, String> {
    let rendered = match url_override {
        Some(url) => url.to_string(),
        None => render_source_config_template(url, source_config)?,
    };
    Ok(append_query_params(rendered, query_params))
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
