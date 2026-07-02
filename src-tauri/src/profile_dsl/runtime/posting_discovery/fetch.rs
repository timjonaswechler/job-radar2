use super::support::{push_browser_fetch_diagnostic, render_source_config_template};
use super::*;

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    source_name: &str,
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
        source_name,
        &[],
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
    source_name: &str,
    query_params: &[(&str, String)],
    json_body_params: &[(&str, String)],
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
        source_name,
        None,
        query_params,
        json_body_params,
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
    source_name: &str,
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
        source_name,
        Some(url_override),
        &[],
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
    source_name: &str,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
    json_body_params: &[(&str, String)],
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
            body,
            timeout_ms,
            ..
        } => {
            fetch_http_strategy_document(
                fetcher,
                *method,
                url,
                headers.as_ref(),
                body.as_ref(),
                *timeout_ms,
                source_config,
                source_name,
                url_override,
                query_params,
                json_body_params,
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
                source_name,
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
    body: Option<&RequestBody>,
    timeout_ms: u64,
    source_config: &SourceConfig,
    source_name: &str,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
    json_body_params: &[(&str, String)],
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    let method = method.unwrap_or(HttpMethod::Get);
    if method == HttpMethod::Get && body.is_some() {
        diagnostics.push(runtime_error(
            "unsupported_http_body_for_method",
            "HTTP GET fetch requests cannot declare a request body",
            format!("{base_path}/fetch/body"),
            strategy_key,
            json!({ "method": "GET" }),
        ));
        return None;
    }

    let rendered_url =
        match render_fetch_url(url, source_config, source_name, url_override, query_params) {
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

    let rendered_headers = match render_headers(headers, source_config, source_name) {
        Ok(headers) => headers,
        Err(message) => {
            diagnostics.push(runtime_error(
                "fetch_header_template_failed",
                format!("Fetch header template could not be rendered: {message}"),
                format!("{base_path}/fetch/headers"),
                strategy_key,
                json!({}),
            ));
            return None;
        }
    };

    let rendered_body =
        match render_request_body(body, source_config, source_name, json_body_params) {
            Ok(body) => body,
            Err(message) => {
                diagnostics.push(runtime_error(
                    "fetch_body_template_failed",
                    format!("Fetch body template could not be rendered: {message}"),
                    format!("{base_path}/fetch/body"),
                    strategy_key,
                    json!({}),
                ));
                return None;
            }
        };

    let request = PostingDiscoveryFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: rendered_headers,
        body: rendered_body,
        timeout_ms,
    };

    match fetcher.fetch(request).await {
        Ok(response) => Some(response),
        Err(error) => {
            diagnostics.push(runtime_error(
                "fetch_failed",
                format!(
                    "HTTP {} fetch failed for {rendered_url}: {}",
                    http_method_label(method),
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
    source_name: &str,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let rendered_url =
        match render_fetch_url(url, source_config, source_name, url_override, query_params) {
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
    source_name: &str,
    url_override: Option<&str>,
    query_params: &[(&str, String)],
) -> Result<String, String> {
    let rendered = match url_override {
        Some(url) => url.to_string(),
        None => render_source_config_template(url, source_config, source_name)?,
    };
    Ok(append_query_params(rendered, query_params))
}

fn render_headers(
    headers: Option<&BTreeMap<String, String>>,
    source_config: &SourceConfig,
    source_name: &str,
) -> Result<BTreeMap<String, String>, String> {
    let mut rendered = BTreeMap::new();
    for (name, value) in headers.into_iter().flatten() {
        rendered.insert(
            name.clone(),
            render_source_config_template(value, source_config, source_name)?,
        );
    }
    Ok(rendered)
}

fn render_request_body(
    body: Option<&RequestBody>,
    source_config: &SourceConfig,
    source_name: &str,
    json_body_params: &[(&str, String)],
) -> Result<Option<RequestBody>, String> {
    let Some(body) = body else {
        return Ok(None);
    };
    match body {
        RequestBody::Json { value } => {
            let mut rendered = value
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        render_json_body_value(value, source_config, source_name)?,
                    ))
                })
                .collect::<Result<serde_json::Map<String, Value>, String>>()?;
            for (key, value) in json_body_params {
                rendered.insert((*key).to_string(), render_pagination_json_value(value));
            }
            Ok(Some(RequestBody::Json { value: rendered }))
        }
        RequestBody::Text { value } => Ok(Some(RequestBody::Text {
            value: render_source_config_template(value, source_config, source_name)?,
        })),
        RequestBody::Form { fields } => Ok(Some(RequestBody::Form {
            fields: fields
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        render_source_config_template(value, source_config, source_name)?,
                    ))
                })
                .collect::<Result<BTreeMap<String, String>, String>>()?,
        })),
    }
}

fn render_pagination_json_value(value: &str) -> Value {
    value
        .parse::<u64>()
        .map(serde_json::Number::from)
        .map(Value::Number)
        .unwrap_or_else(|_| Value::String(value.to_string()))
}

fn render_json_body_value(
    value: &Value,
    source_config: &SourceConfig,
    source_name: &str,
) -> Result<Value, String> {
    match value {
        Value::String(value) => Ok(Value::String(render_source_config_template(
            value,
            source_config,
            source_name,
        )?)),
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|value| render_json_body_value(value, source_config, source_name))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Object(values) => Ok(Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        render_json_body_value(value, source_config, source_name)?,
                    ))
                })
                .collect::<Result<serde_json::Map<String, Value>, String>>()?,
        )),
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value.clone()),
    }
}

fn http_method_label(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
    }
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
