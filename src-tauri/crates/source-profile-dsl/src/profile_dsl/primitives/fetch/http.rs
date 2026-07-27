use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::documents::{HttpMethod, RequestBody};
use crate::profile_dsl::template::{
    compile_template, json_pointer_segment, CompiledTemplate, TemplateCompileError,
    TemplateCompileErrorKind, TemplateDescriptor,
};

pub const PUBLIC_HTTP_HEADERS: [&str; 6] = [
    "accept",
    "accept-language",
    "content-type",
    "referer",
    "user-agent",
    "x-requested-with",
];

pub fn is_public_http_header(name: &str) -> bool {
    PUBLIC_HTTP_HEADERS.contains(&name)
}

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
    public_headers: &PUBLIC_HTTP_HEADERS,
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
pub struct HttpFetchSecurityBehavior {
    public_headers: &'static [&'static str],
    secret_like_applicability: &'static [&'static str],
}

pub fn http_fetch_security_behavior() -> HttpFetchSecurityBehavior {
    HttpFetchSecurityBehavior {
        public_headers: &PUBLIC_HTTP_HEADERS,
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
    pub fn supports_json_body_overlay(&self) -> bool {
        self.method == HttpMethod::Post
            && matches!(self.body, Some(CompiledHttpRequestBody::Json { .. }))
    }

    pub fn references_source_name(&self) -> bool {
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
        if !is_public_http_header(name) {
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

fn witness_http() {
    fn check(v: &CompiledHttpFetch) {
        let _ = (&v.method, &v.url, &v.headers, &v.body, &v.timeout_ms);
    }
    let _ = check as fn(&CompiledHttpFetch);
}
fn witness_http_get() {
    fn check(v: &CompiledHttpFetch) {
        if let HttpMethod::Get = v.method {}
    }
    let _ = check as fn(&CompiledHttpFetch);
}
fn witness_http_post() {
    fn check(v: &CompiledHttpFetch) {
        if let HttpMethod::Post = v.method {}
    }
    let _ = check as fn(&CompiledHttpFetch);
}
fn witness_body_json() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Json { .. } = v {}
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_body_text() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Text { .. } = v {}
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_body_form() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Form { .. } = v {}
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_body_json_value() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Json { value } = v {
            let _ = value;
        }
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_body_text_value() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Text { value } = v {
            let _ = value;
        }
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_body_form_fields() {
    fn check(v: &CompiledHttpRequestBody) {
        if let CompiledHttpRequestBody::Form { fields } = v {
            let _ = fields;
        }
    }
    let _ = check as fn(&CompiledHttpRequestBody);
}
fn witness_http_url() {
    fn check(v: &CompiledHttpFetch) {
        let _ = &v.url;
    }
    let _ = check as fn(&CompiledHttpFetch);
}
fn witness_http_headers() {
    fn check(v: &CompiledHttpFetch) {
        let _ = &v.headers;
    }
    let _ = check as fn(&CompiledHttpFetch);
}
fn witness_http_timeout() {
    fn check(v: &CompiledHttpFetch) {
        let _ = &v.timeout_ms;
    }
    let _ = check as fn(&CompiledHttpFetch);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::profile_dsl::primitives::completeness::CompiledRegistration> {
    use crate::profile_dsl::primitives::completeness::{
        AuthoredShapeKind::{ParentOption, Tagged},
        CompiledRegistration,
        Family::Fetch,
        Owner::P09,
        PrimitiveContext::{Detail, DetectionHttp, Discovery},
    };
    macro_rules! row {
        ($key:literal,$shape:expr,$identity:literal,$witness:expr) => {
            CompiledRegistration {
                family: Fetch,
                key: $key,
                contexts: &[Discovery, Detail, DetectionHttp],
                owner: P09,
                canonical_file:
                    "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/fetch/http.rs",
                shape: $shape,
                compiled_identity: $identity,
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    vec![
        row!("http", Tagged, "CompiledHttpFetch", witness_http),
        row!(
            "http.method.GET",
            ParentOption,
            "CompiledHttpFetch.method::Get",
            witness_http_get
        ),
        row!(
            "http.method.POST",
            ParentOption,
            "CompiledHttpFetch.method::Post",
            witness_http_post
        ),
        row!(
            "http.body.json",
            ParentOption,
            "CompiledHttpRequestBody::Json",
            witness_body_json
        ),
        row!(
            "http.body.text",
            ParentOption,
            "CompiledHttpRequestBody::Text",
            witness_body_text
        ),
        row!(
            "http.body.form",
            ParentOption,
            "CompiledHttpRequestBody::Form",
            witness_body_form
        ),
        row!(
            "http.body.json.value",
            ParentOption,
            "CompiledHttpRequestBody::Json.value",
            witness_body_json_value
        ),
        row!(
            "http.body.text.value",
            ParentOption,
            "CompiledHttpRequestBody::Text.value",
            witness_body_text_value
        ),
        row!(
            "http.body.form.fields",
            ParentOption,
            "CompiledHttpRequestBody::Form.fields",
            witness_body_form_fields
        ),
        row!(
            "http.url",
            ParentOption,
            "CompiledHttpFetch.url",
            witness_http_url
        ),
        row!(
            "http.headers",
            ParentOption,
            "CompiledHttpFetch.headers",
            witness_http_headers
        ),
        row!(
            "http.timeoutMs",
            ParentOption,
            "CompiledHttpFetch.timeout_ms",
            witness_http_timeout
        ),
    ]
}
