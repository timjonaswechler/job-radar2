use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::documents::{HttpMethod, RequestBody};
use crate::profile_dsl::runtime::allowance::AllowanceCharge;
use crate::profile_dsl::runtime::http::{
    ProfileHttpClient, ProfileHttpError, ProfileHttpRequest, ProfileHttpResponse,
    SensitiveRequestBody,
};
use crate::profile_dsl::runtime::RuntimeExecutionContext;
use crate::profile_dsl::template::{
    compile_template, json_pointer_segment, render_template, CompiledTemplate,
    TemplateCompileError, TemplateCompileErrorKind, TemplateDescriptor, TemplateValueView,
};

const PUBLIC_HEADERS: [&str; 6] = [
    "accept",
    "accept-language",
    "content-type",
    "referer",
    "user-agent",
    "x-requested-with",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpFetchDescriptor {
    pub mode: &'static str,
    pub methods: &'static [&'static str],
    pub body_types: &'static [&'static str],
    pub public_headers: &'static [&'static str],
    pub timeout_ms_minimum: u64,
    pub timeout_ms_maximum: u64,
}

pub const HTTP_FETCH_DESCRIPTOR: HttpFetchDescriptor = HttpFetchDescriptor {
    mode: "http",
    methods: &["GET", "POST"],
    body_types: &["json", "text", "form"],
    public_headers: &PUBLIC_HEADERS,
    timeout_ms_minimum: 1,
    timeout_ms_maximum: 60_000,
};

pub fn http_fetch_descriptors() -> &'static [HttpFetchDescriptor] {
    std::slice::from_ref(&HTTP_FETCH_DESCRIPTOR)
}

pub fn validate_http_fetch_descriptors(
    descriptors: &[HttpFetchDescriptor],
) -> Result<(), &'static str> {
    if descriptors.len() != 1 {
        return Err("HTTP Fetch must have exactly one canonical owner");
    }
    if descriptors[0] != HTTP_FETCH_DESCRIPTOR {
        return Err("HTTP Fetch descriptor does not match the canonical authored catalogue");
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpFetchSecurityBehavior {
    public_headers: &'static [&'static str],
    secret_like_applicability: &'static [&'static str],
}

pub(crate) fn http_fetch_security_behavior() -> HttpFetchSecurityBehavior {
    HttpFetchSecurityBehavior {
        public_headers: &PUBLIC_HEADERS,
        secret_like_applicability: &["form_body_field_names", "json_object_keys"],
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledHttpFetch {
    pub method: HttpMethod,
    pub url: CompiledTemplate,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, CompiledTemplate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<CompiledHttpRequestBody>,
    pub timeout_ms: u64,
}

impl CompiledHttpFetch {
    pub(crate) fn references_source_name(&self) -> bool {
        self.url.references(Some("source"), "name")
            || self
                .headers
                .values()
                .any(|value| value.references(Some("source"), "name"))
            || self
                .body
                .as_ref()
                .is_some_and(CompiledHttpRequestBody::references_source_name)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledHttpRequestBody {
    Json {
        value: BTreeMap<String, CompiledHttpJsonValue>,
    },
    Text {
        value: CompiledTemplate,
    },
    Form {
        fields: BTreeMap<String, CompiledTemplate>,
    },
}

impl CompiledHttpRequestBody {
    fn references_source_name(&self) -> bool {
        match self {
            Self::Text { value } => value.references(Some("source"), "name"),
            Self::Form { fields } => fields
                .values()
                .any(|value| value.references(Some("source"), "name")),
            Self::Json { value } => value
                .values()
                .any(CompiledHttpJsonValue::references_source_name),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CompiledHttpJsonValue {
    Template(CompiledTemplate),
    Array(Vec<CompiledHttpJsonValue>),
    Object(BTreeMap<String, CompiledHttpJsonValue>),
    Scalar(Value),
}

impl CompiledHttpJsonValue {
    fn references_source_name(&self) -> bool {
        match self {
            Self::Template(value) => value.references(Some("source"), "name"),
            Self::Array(values) => values.iter().any(Self::references_source_name),
            Self::Object(values) => values.values().any(Self::references_source_name),
            Self::Scalar(_) => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpFetchCompileError {
    pub path: String,
    pub code: &'static str,
    pub message: String,
}

impl HttpFetchCompileError {
    fn new(path: impl Into<String>, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code,
            message: message.into(),
        }
    }
}

pub fn compile_http_fetch(
    method: Option<HttpMethod>,
    url: &str,
    headers: Option<&BTreeMap<String, String>>,
    body: Option<&RequestBody>,
    timeout_ms: u64,
    url_descriptor: &TemplateDescriptor,
    header_descriptor: &TemplateDescriptor,
    body_descriptor: &TemplateDescriptor,
) -> Result<CompiledHttpFetch, HttpFetchCompileError> {
    if !(1..=60_000).contains(&timeout_ms) {
        return Err(HttpFetchCompileError::new(
            "/timeoutMs",
            "http_fetch_timeout_out_of_bounds",
            "HTTP timeoutMs must be between 1 and 60000",
        ));
    }
    let method = method.unwrap_or(HttpMethod::Get);
    if method == HttpMethod::Get && body.is_some() {
        return Err(HttpFetchCompileError::new(
            "/body",
            "unsupported_http_body_for_method",
            "HTTP GET fetch requests cannot declare a request body",
        ));
    }

    let url = compile_template(url, url_descriptor)
        .map_err(|error| template_compile_error("/url", error))?;

    let mut compiled_headers = BTreeMap::new();
    for (name, value) in headers.into_iter().flatten() {
        if !PUBLIC_HEADERS.contains(&name.as_str()) {
            return Err(HttpFetchCompileError::new(
                format!("/headers/{}", json_pointer_segment(name)),
                "forbidden_request_header",
                format!("HTTP header `{name}` is not in the public header allowlist"),
            ));
        }
        let value = compile_template(value, header_descriptor).map_err(|error| {
            template_compile_error(format!("/headers/{}", json_pointer_segment(name)), error)
        })?;
        compiled_headers.insert(name.clone(), value);
    }

    let body = body
        .map(|body| compile_body(body, body_descriptor))
        .transpose()?;

    Ok(CompiledHttpFetch {
        method,
        url,
        headers: compiled_headers,
        body,
        timeout_ms,
    })
}

fn compile_body(
    body: &RequestBody,
    descriptor: &TemplateDescriptor,
) -> Result<CompiledHttpRequestBody, HttpFetchCompileError> {
    match body {
        RequestBody::Json { value } => Ok(CompiledHttpRequestBody::Json {
            value: value
                .iter()
                .map(|(key, value)| {
                    if is_secret_like_key(key) {
                        return Err(secret_body_error(format!(
                            "/body/value/{}",
                            json_pointer_segment(key)
                        )));
                    }
                    Ok((
                        key.clone(),
                        compile_json_value(
                            value,
                            descriptor,
                            &format!("/body/value/{}", json_pointer_segment(key)),
                        )?,
                    ))
                })
                .collect::<Result<_, _>>()?,
        }),
        RequestBody::Text { value } => Ok(CompiledHttpRequestBody::Text {
            value: compile_template(value, descriptor)
                .map_err(|error| template_compile_error("/body/value", error))?,
        }),
        RequestBody::Form { fields } => Ok(CompiledHttpRequestBody::Form {
            fields: fields
                .iter()
                .map(|(key, value)| {
                    if is_secret_like_key(key) {
                        return Err(secret_body_error(format!(
                            "/body/fields/{}",
                            json_pointer_segment(key)
                        )));
                    }
                    Ok((
                        key.clone(),
                        compile_template(value, descriptor).map_err(|error| {
                            template_compile_error(
                                format!("/body/fields/{}", json_pointer_segment(key)),
                                error,
                            )
                        })?,
                    ))
                })
                .collect::<Result<_, _>>()?,
        }),
    }
}

fn compile_json_value(
    value: &Value,
    descriptor: &TemplateDescriptor,
    path: &str,
) -> Result<CompiledHttpJsonValue, HttpFetchCompileError> {
    match value {
        Value::String(value) => Ok(CompiledHttpJsonValue::Template(
            compile_template(value, descriptor)
                .map_err(|error| template_compile_error(path, error))?,
        )),
        Value::Array(values) => Ok(CompiledHttpJsonValue::Array(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    compile_json_value(value, descriptor, &format!("{path}/{index}"))
                })
                .collect::<Result<_, _>>()?,
        )),
        Value::Object(values) => Ok(CompiledHttpJsonValue::Object(
            values
                .iter()
                .map(|(key, value)| {
                    let child_path = format!("{path}/{}", json_pointer_segment(key));
                    if is_secret_like_key(key) {
                        return Err(secret_body_error(child_path));
                    }
                    Ok((
                        key.clone(),
                        compile_json_value(value, descriptor, &child_path)?,
                    ))
                })
                .collect::<Result<_, _>>()?,
        )),
        _ => Ok(CompiledHttpJsonValue::Scalar(value.clone())),
    }
}

fn template_compile_error(
    path: impl Into<String>,
    error: TemplateCompileError,
) -> HttpFetchCompileError {
    let code = match error.kind {
        TemplateCompileErrorKind::TransformPipeUnsupported => {
            "template_transform_pipes_unsupported"
        }
        TemplateCompileErrorKind::UnknownNamespace
            if error
                .reference
                .as_ref()
                .and_then(|reference| reference.namespace.as_deref())
                .is_some_and(|namespace| {
                    matches!(namespace, "posting" | "postingMeta" | "captures")
                }) =>
        {
            "template_namespace_unavailable"
        }
        TemplateCompileErrorKind::UnknownNamespace => "invalid_template_namespace",
        TemplateCompileErrorKind::UnknownKey => "unknown_template_key",
        _ => "invalid_template_reference",
    };
    HttpFetchCompileError::new(path, code, error.to_string())
}

fn secret_body_error(path: String) -> HttpFetchCompileError {
    HttpFetchCompileError::new(
        path,
        "secret_like_request_body_field",
        "Request body field looks like a secret or credential",
    )
}

fn is_secret_like_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "password",
        "token",
        "apikey",
        "auth",
        "session",
        "credential",
    ]
    .iter()
    .any(|part| normalized.contains(part))
}

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
    if !(200..=299).contains(&response.status()) {
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
