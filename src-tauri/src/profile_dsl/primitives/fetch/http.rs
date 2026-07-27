use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::profile_dsl::documents::HttpMethod;
use crate::profile_dsl::runtime::allowance::AllowanceCharge;
use crate::profile_dsl::runtime::http::{
    ProfileHttpClient, ProfileHttpError, ProfileHttpRequest, ProfileHttpResponse,
    SensitiveRequestBody,
};
use crate::profile_dsl::runtime::RuntimeExecutionContext;
use crate::profile_dsl::template::{render_template, TemplateValueView};
pub use source_profile_dsl::profile_dsl::primitives::fetch::http::*;

#[derive(Clone, Copy, Default)]
pub struct HttpFetchOverlay<'a> {
    pub url_override: Option<&'a str>,
    pub query_params: &'a [(&'a str, String)],
    pub json_body_params: &'a [(&'a str, String)],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpFetchRenderError {
    pub path: &'static str,
    pub code: &'static str,
    pub message: String,
}

impl HttpFetchRenderError {
    fn new(path: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            path,
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HttpStatusPolicy {
    RequireSuccess,
    PreserveResponse,
}

pub enum HttpFetchExecutionError {
    Render(HttpFetchRenderError),
    Cancelled,
    BudgetExhausted,
    NonSuccessStatus { status: u16 },
    Acquisition(ProfileHttpError),
}

pub async fn execute_http_fetch<
    C: ProfileHttpClient + Sync + ?Sized,
    V: TemplateValueView + Sync,
>(
    client: &C,
    fetch: &CompiledHttpFetch,
    values: &V,
    overlay: HttpFetchOverlay<'_>,
    authored_charset: Option<&str>,
    status_policy: HttpStatusPolicy,
    context: RuntimeExecutionContext<'_>,
) -> Result<ProfileHttpResponse, HttpFetchExecutionError> {
    let request = render_http_request(fetch, values, overlay, authored_charset)
        .map_err(HttpFetchExecutionError::Render)?;
    if context.is_cancelled() {
        return Err(HttpFetchExecutionError::Cancelled);
    }
    if context
        .debit(AllowanceCharge {
            requests: 1,
            pages: u64::from(context.page_request()),
            ..AllowanceCharge::default()
        })
        .is_err()
    {
        return Err(HttpFetchExecutionError::BudgetExhausted);
    }
    if context.is_cancelled() {
        return Err(HttpFetchExecutionError::Cancelled);
    }
    let response = client
        .fetch(request, context)
        .await
        .map_err(HttpFetchExecutionError::Acquisition)?;
    if status_policy == HttpStatusPolicy::RequireSuccess
        && !(200..=299).contains(&response.status())
    {
        return Err(HttpFetchExecutionError::NonSuccessStatus {
            status: response.status(),
        });
    }
    Ok(response)
}

fn render_http_request<V: TemplateValueView + Sync>(
    fetch: &CompiledHttpFetch,
    values: &V,
    overlay: HttpFetchOverlay<'_>,
    authored_charset: Option<&str>,
) -> Result<ProfileHttpRequest, HttpFetchRenderError> {
    let base_url = match overlay.url_override {
        Some(url) => url.to_string(),
        None => render_template(&fetch.url, values).map_err(|error| {
            HttpFetchRenderError::new("/url", "fetch_url_template_failed", error.to_string())
        })?,
    };
    let url = render_url(
        &base_url,
        overlay.query_params,
        overlay.url_override.is_some(),
    )?;

    let headers = fetch
        .headers
        .iter()
        .map(|(name, value)| {
            let value = render_template(value, values).map_err(|error| {
                HttpFetchRenderError::new(
                    "/headers",
                    "fetch_header_template_failed",
                    error.to_string(),
                )
            })?;
            reqwest::header::HeaderValue::from_str(&value).map_err(|_| {
                HttpFetchRenderError::new(
                    "/headers",
                    "invalid_rendered_header_value",
                    "rendered HTTP header value is invalid",
                )
            })?;
            Ok((name.clone(), value.into_bytes()))
        })
        .collect::<Result<Vec<_>, HttpFetchRenderError>>()?;

    let body = render_body(fetch, values, overlay.json_body_params)?;
    Ok(ProfileHttpRequest {
        method: fetch.method,
        url,
        headers,
        body,
        timeout_ms: fetch.timeout_ms,
        authored_charset: authored_charset.map(ToString::to_string),
    })
}

fn render_url(
    value: &str,
    query_params: &[(&str, String)],
    require_safe_absolute_url: bool,
) -> Result<String, HttpFetchRenderError> {
    if query_params.is_empty() && !require_safe_absolute_url {
        return Ok(value.to_string());
    }
    let mut url = url::Url::parse(value).map_err(|_| {
        HttpFetchRenderError::new(
            "/url",
            "invalid_rendered_fetch_url",
            "rendered HTTP Fetch URL must be absolute",
        )
    })?;
    if require_safe_absolute_url
        && (!matches!(url.scheme(), "http" | "https")
            || !url.has_host()
            || !url.username().is_empty()
            || url.password().is_some())
    {
        return Err(HttpFetchRenderError::new(
            "/url",
            "invalid_rendered_fetch_url",
            "rendered HTTP Fetch URL must use http(s) and contain no userinfo",
        ));
    }
    if !query_params.is_empty() {
        let replacements = query_params
            .iter()
            .map(|(key, _)| *key)
            .collect::<BTreeSet<_>>();
        let mut pairs = url
            .query_pairs()
            .filter(|(key, _)| !replacements.contains(key.as_ref()))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        pairs.extend(
            query_params
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone())),
        );
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(pairs);
    }
    Ok(url.into())
}

fn render_body<V: TemplateValueView + Sync>(
    fetch: &CompiledHttpFetch,
    values: &V,
    json_body_params: &[(&str, String)],
) -> Result<Option<SensitiveRequestBody>, HttpFetchRenderError> {
    if !json_body_params.is_empty()
        && (fetch.method != HttpMethod::Post
            || !matches!(fetch.body, Some(CompiledHttpRequestBody::Json { .. })))
    {
        return Err(HttpFetchRenderError::new(
            "/body",
            "invalid_json_body_overlay",
            "json_body overlay requires an HTTP POST JSON body",
        ));
    }
    match &fetch.body {
        None => Ok(None),
        Some(CompiledHttpRequestBody::Text { value }) => render_template(value, values)
            .map(SensitiveRequestBody::text)
            .map(Some)
            .map_err(|error| {
                HttpFetchRenderError::new("/body", "fetch_body_template_failed", error.to_string())
            }),
        Some(CompiledHttpRequestBody::Form { fields }) => {
            let rendered = fields
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        render_template(value, values).map_err(|error| {
                            HttpFetchRenderError::new(
                                "/body",
                                "fetch_body_template_failed",
                                error.to_string(),
                            )
                        })?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, HttpFetchRenderError>>()?;
            Ok(Some(SensitiveRequestBody::form(&rendered)))
        }
        Some(CompiledHttpRequestBody::Json { value }) => {
            let mut rendered = value
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_json_value(value, values)?)))
                .collect::<Result<serde_json::Map<_, _>, HttpFetchRenderError>>()?;
            for (key, value) in json_body_params {
                if !rendered.contains_key(*key) {
                    return Err(HttpFetchRenderError::new(
                        "/body",
                        "invalid_json_body_overlay",
                        "json_body overlay may only replace an authored top-level key",
                    ));
                }
                rendered.insert((*key).to_string(), pagination_json_value(value));
            }
            SensitiveRequestBody::json(&rendered)
                .map(Some)
                .map_err(|()| {
                    HttpFetchRenderError::new(
                        "/body",
                        "fetch_body_render_failed",
                        "rendered HTTP request body could not be encoded",
                    )
                })
        }
    }
}

fn render_json_value<V: TemplateValueView + Sync>(
    value: &CompiledHttpJsonValue,
    values: &V,
) -> Result<Value, HttpFetchRenderError> {
    match value {
        CompiledHttpJsonValue::Template(value) => render_template(value, values)
            .map(Value::String)
            .map_err(|error| {
                HttpFetchRenderError::new("/body", "fetch_body_template_failed", error.to_string())
            }),
        CompiledHttpJsonValue::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|item| render_json_value(item, values))
                .collect::<Result<_, _>>()?,
        )),
        CompiledHttpJsonValue::Object(object) => Ok(Value::Object(
            object
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_json_value(value, values)?)))
                .collect::<Result<_, _>>()?,
        )),
        CompiledHttpJsonValue::Scalar(value) => Ok(value.clone()),
    }
}

fn pagination_json_value(value: &str) -> Value {
    value
        .parse::<u64>()
        .map(serde_json::Number::from)
        .map(Value::Number)
        .unwrap_or_else(|_| Value::String(value.to_string()))
}
