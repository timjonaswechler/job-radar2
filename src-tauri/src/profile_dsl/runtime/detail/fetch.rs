use super::support::{push_browser_fetch_diagnostic, render_template, TemplateRuntimeContext};
use super::*;

pub(super) async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<DetailFetchResponse>, TypedCancellation>
where
    F: DetailFetcher + Sync + ?Sized,
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
                &context,
                base_path,
                strategy_key,
                strategy_index,
                diagnostics,
                execution_context,
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
                strategy_index,
                diagnostics,
                execution_context,
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
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<DetailFetchResponse>, TypedCancellation>
where
    F: DetailFetcher + Sync + ?Sized,
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
        return Ok(None);
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
            return Ok(None);
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
            return Ok(None);
        }
    };

    let rendered_body = match render_request_body(body, context) {
        Ok(body) => body,
        Err(message) => {
            diagnostics.push(runtime_error(
                "fetch_body_template_failed",
                format!("Fetch body template could not be rendered: {message}"),
                format!("{base_path}/fetch/body"),
                strategy_key,
                json!({}),
            ));
            return Ok(None);
        }
    };

    let request = DetailFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: rendered_headers,
        body: rendered_body,
        timeout_ms,
    };

    if execution_context.is_cancelled() {
        return Err(TypedCancellation::strategy(
            RuntimePhase::Detail,
            strategy_index,
            strategy_key.expect("compiled strategy has a key"),
            CancellationOperation::Fetch,
        ));
    }

    let result = tokio::select! {
        result = fetcher.fetch(request) => Some(result),
        _ = execution_context.cancelled() => None,
    };
    let Some(result) = result else {
        return Err(TypedCancellation::strategy(
            RuntimePhase::Detail,
            strategy_index,
            strategy_key.expect("compiled strategy has a key"),
            CancellationOperation::Fetch,
        ));
    };

    match result {
        Ok(response) => Ok(Some(response)),
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
            Ok(None)
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
    strategy_index: usize,
    diagnostics: &mut Diagnostics,
    execution_context: RuntimeExecutionContext<'_>,
) -> Result<Option<DetailFetchResponse>, TypedCancellation>
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
            return Ok(None);
        }
    };

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
        Ok(ProfileBrowserFetchResponse { body }) => Ok(Some(DetailFetchResponse { body })),
        Err(error) if error.kind == ProfileBrowserFetchErrorKind::Cancelled => {
            Err(TypedCancellation::strategy(
                RuntimePhase::Detail,
                strategy_index,
                strategy_key.expect("compiled strategy has a key"),
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
            Ok(None)
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

fn render_request_body(
    body: Option<&RequestBody>,
    context: &TemplateRuntimeContext<'_>,
) -> Result<Option<RequestBody>, String> {
    let Some(body) = body else {
        return Ok(None);
    };
    match body {
        RequestBody::Json { value } => Ok(Some(RequestBody::Json {
            value: value
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_json_body_value(value, context)?)))
                .collect::<Result<serde_json::Map<String, Value>, String>>()?,
        })),
        RequestBody::Text { value } => Ok(Some(RequestBody::Text {
            value: render_template(value, context)?,
        })),
        RequestBody::Form { fields } => Ok(Some(RequestBody::Form {
            fields: fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_template(value, context)?)))
                .collect::<Result<BTreeMap<String, String>, String>>()?,
        })),
    }
}

fn render_json_body_value(
    value: &Value,
    context: &TemplateRuntimeContext<'_>,
) -> Result<Value, String> {
    match value {
        Value::String(value) => Ok(Value::String(render_template(value, context)?)),
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|value| render_json_body_value(value, context))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Object(values) => Ok(Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_json_body_value(value, context)?)))
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
