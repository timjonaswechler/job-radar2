use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use encoding_rs::{DecoderResult, Encoding, UTF_16BE, UTF_16LE, UTF_8};
use futures_util::{stream, Stream, StreamExt};

use crate::profile_dsl::documents::HttpMethod;

use super::cancellation::RuntimeExecutionContext;

/// A rendered request body. This boundary type intentionally has no Serde or Debug
/// implementation so authored secrets cannot accidentally cross H01 through generic
/// serialization or diagnostic formatting.
#[derive(Clone, PartialEq)]
pub struct SensitiveRequestBody {
    bytes: Vec<u8>,
    default_content_type: Option<&'static str>,
}

impl SensitiveRequestBody {
    pub(crate) fn json(value: &serde_json::Map<String, serde_json::Value>) -> Result<Self, ()> {
        Ok(Self {
            bytes: serde_json::to_vec(value).map_err(|_| ())?,
            default_content_type: Some("application/json"),
        })
    }

    pub(crate) fn text(value: String) -> Self {
        Self {
            bytes: value.into_bytes(),
            default_content_type: None,
        }
    }

    pub(crate) fn form(fields: &std::collections::BTreeMap<String, String>) -> Self {
        Self {
            bytes: url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(fields)
                .finish()
                .into_bytes(),
            default_content_type: Some("application/x-www-form-urlencoded"),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn default_content_type(&self) -> Option<&str> {
        self.default_content_type
    }
}

/// A fully rendered HTTP request. This type intentionally has no Serde or Debug implementation.
#[derive(Clone)]
pub struct ProfileHttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Option<SensitiveRequestBody>,
    pub timeout_ms: u64,
    pub authored_charset: Option<String>,
}

impl PartialEq for ProfileHttpRequest {
    fn eq(&self, other: &Self) -> bool {
        self.method == other.method
            && self.url == other.url
            && self.headers == other.headers
            && self.body == other.body
            && self.timeout_ms == other.timeout_ms
            && self.authored_charset == other.authored_charset
    }
}

pub struct ProfileHttpHeader {
    name: String,
    value: Vec<u8>,
}

impl ProfileHttpHeader {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

pub struct ProfileHttpResponse {
    pub body: String,
    status: u16,
    headers: Vec<ProfileHttpHeader>,
    final_url: String,
    content_type: Option<String>,
    raw_body: Vec<u8>,
}

impl ProfileHttpResponse {
    pub fn status(&self) -> u16 {
        self.status
    }
    pub fn headers(&self) -> &[ProfileHttpHeader] {
        &self.headers
    }
    pub fn final_url(&self) -> &str {
        &self.final_url
    }
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
    pub fn raw_body(&self) -> &[u8] {
        &self.raw_body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileHttpFailureKind {
    Connect,
    Timeout,
    BodyStream,
    ResponseBytesExceeded,
    Cancelled,
    InvalidCharset,
    MalformedText,
    InvalidRequest,
    Internal,
}

/// A sanitized failure. It contains no raw transport error, URL, header, or body.
pub struct ProfileHttpError {
    pub kind: ProfileHttpFailureKind,
    pub admitted_bytes: u64,
}

pub trait ProfileHttpClient {
    fn fetch<'a>(
        &'a self,
        request: ProfileHttpRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProfileHttpResponse, ProfileHttpError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct ReqwestProfileHttpClient {
    client: reqwest::Client,
}

impl ReqwestProfileHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .expect("static reqwest client configuration is valid");
        Self { client }
    }
}

impl Default for ReqwestProfileHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileHttpClient for ReqwestProfileHttpClient {
    fn fetch<'a>(
        &'a self,
        request: ProfileHttpRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProfileHttpResponse, ProfileHttpError>> + Send + 'a>>
    {
        Box::pin(async move {
            let method = match request.method {
                HttpMethod::Get => reqwest::Method::GET,
                HttpMethod::Post => reqwest::Method::POST,
            };
            let mut builder = self
                .client
                .request(method, &request.url)
                .timeout(Duration::from_millis(request.timeout_ms));
            for (name, value) in &request.headers {
                let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| failure(ProfileHttpFailureKind::InvalidRequest, 0))?;
                let value = reqwest::header::HeaderValue::from_bytes(value)
                    .map_err(|_| failure(ProfileHttpFailureKind::InvalidRequest, 0))?;
                builder = builder.header(name, value);
            }
            if let Some(body) = &request.body {
                if let Some(content_type) = body.default_content_type() {
                    if !request
                        .headers
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                    {
                        builder = builder.header("content-type", content_type);
                    }
                }
                builder = builder.body(body.bytes().to_vec());
            }
            let response = tokio::select! {
                biased;
                _ = context.cancelled() => return Err(failure(ProfileHttpFailureKind::Cancelled, 0)),
                response = builder.send() => response.map_err(|error| {
                    if error.is_timeout() { failure(ProfileHttpFailureKind::Timeout, 0) }
                    else if error.is_connect() { failure(ProfileHttpFailureKind::Connect, 0) }
                    else { failure(ProfileHttpFailureKind::InvalidRequest, 0) }
                })?,
                _ = context.deadline_reached() => {
                    context.mark_deadline();
                    return Err(failure(ProfileHttpFailureKind::Timeout, 0));
                },
            };
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let headers = copy_headers(response.headers());
            let content_length = strict_content_length(&headers);
            let body_stream = response.bytes_stream().map(|item| {
                item.map(|b| b.to_vec())
                    .map_err(|_| ProfileHttpFailureKind::BodyStream)
            });
            collect_and_decode(
                body_stream,
                status,
                final_url,
                headers,
                content_length,
                request.authored_charset.as_deref(),
                context,
            )
            .await
        })
    }
}

pub enum ScriptedHttpEvent {
    Response {
        status: u16,
        final_url: String,
        headers: Vec<(String, Vec<u8>)>,
        body: Vec<ScriptedHttpBodyEvent>,
        /// Independent length evidence. It may be absent, accurate, or inaccurate.
        content_length: Option<u64>,
    },
}

pub enum ScriptedHttpBodyEvent {
    Chunk(Vec<u8>),
    Gate(String),
    Failure(ProfileHttpFailureKind),
}

pub struct ScriptedProfileHttpClient {
    requests: Mutex<Vec<ProfileHttpRequest>>,
    expected: Mutex<VecDeque<ProfileHttpRequest>>,
    validate_requests: bool,
    events: Mutex<VecDeque<ScriptedHttpEvent>>,
    gates: Mutex<BTreeMap<String, Arc<tokio::sync::Notify>>>,
}

impl ScriptedProfileHttpClient {
    pub fn new(events: impl IntoIterator<Item = ScriptedHttpEvent>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            expected: Mutex::new(VecDeque::new()),
            validate_requests: false,
            events: Mutex::new(events.into_iter().collect()),
            gates: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn expecting(
        expected: impl IntoIterator<Item = ProfileHttpRequest>,
        events: impl IntoIterator<Item = ScriptedHttpEvent>,
    ) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            expected: Mutex::new(expected.into_iter().collect()),
            validate_requests: true,
            events: Mutex::new(events.into_iter().collect()),
            gates: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn gate_is_waiting(&self, name: &str) -> bool {
        self.gates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(name)
    }

    pub fn release_gate(&self, name: &str) -> bool {
        let notify = self
            .gates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(name)
            .cloned();
        if let Some(notify) = notify {
            notify.notify_one();
            true
        } else {
            false
        }
    }

    pub fn request_count(&self) -> usize {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    pub fn requests(&self) -> Vec<ProfileHttpRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn take_requests(&self) -> Vec<ProfileHttpRequest> {
        std::mem::take(&mut *self.requests.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn expectations_satisfied(&self) -> bool {
        self.expected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
            && self
                .events
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty()
    }
}

impl ProfileHttpClient for ScriptedProfileHttpClient {
    fn fetch<'a>(
        &'a self,
        request: ProfileHttpRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProfileHttpResponse, ProfileHttpError>> + Send + 'a>>
    {
        Box::pin(async move {
            let authored_charset = request.authored_charset.clone();
            if self.validate_requests {
                let expected = self
                    .expected
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .pop_front()
                    .ok_or_else(|| failure(ProfileHttpFailureKind::InvalidRequest, 0))?;
                if expected != request {
                    return Err(failure(ProfileHttpFailureKind::InvalidRequest, 0));
                }
            }
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(request);
            let event = self
                .events
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pop_front()
                .ok_or_else(|| failure(ProfileHttpFailureKind::Internal, 0))?;
            match event {
                ScriptedHttpEvent::Response {
                    status,
                    final_url,
                    headers,
                    body,
                    content_length,
                } => {
                    let headers = headers
                        .into_iter()
                        .map(|(name, value)| ProfileHttpHeader {
                            name: name.to_ascii_lowercase(),
                            value,
                        })
                        .collect();
                    let body = stream::iter(body).then(|event| async move {
                        match event {
                            ScriptedHttpBodyEvent::Chunk(chunk) => Ok(chunk),
                            ScriptedHttpBodyEvent::Failure(kind) => Err(kind),
                            ScriptedHttpBodyEvent::Gate(gate) => {
                                let notify = {
                                    let mut gates =
                                        self.gates.lock().unwrap_or_else(|p| p.into_inner());
                                    gates
                                        .entry(gate)
                                        .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
                                        .clone()
                                };
                                tokio::select! {
                                    biased;
                                    _ = context.cancelled() => Err(ProfileHttpFailureKind::Cancelled),
                                    _ = notify.notified() => Ok(Vec::new()),
                                    _ = context.deadline_reached() => {
                                        context.mark_deadline();
                                        Err(ProfileHttpFailureKind::Timeout)
                                    },
                                }
                            }
                        }
                    });
                    collect_and_decode(
                        body,
                        status,
                        final_url,
                        headers,
                        content_length,
                        authored_charset.as_deref(),
                        context,
                    )
                    .await
                }
            }
        })
    }
}

fn copy_headers(map: &reqwest::header::HeaderMap) -> Vec<ProfileHttpHeader> {
    let mut headers = Vec::new();
    for name in map.keys() {
        for value in map.get_all(name).iter() {
            headers.push(ProfileHttpHeader {
                name: name.as_str().to_ascii_lowercase(),
                value: value.as_bytes().to_vec(),
            });
        }
    }
    headers.sort_by(|left, right| left.name.cmp(&right.name));
    headers
}

fn strict_content_length(headers: &[ProfileHttpHeader]) -> Option<u64> {
    let values = headers
        .iter()
        .filter(|h| h.name == "content-length")
        .map(|h| {
            std::str::from_utf8(&h.value)
                .ok()?
                .trim()
                .parse::<u64>()
                .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    if values.is_empty() || values.iter().any(|v| *v != values[0]) {
        None
    } else {
        Some(values[0])
    }
}

async fn collect_and_decode<S>(
    stream: S,
    status: u16,
    final_url: String,
    headers: Vec<ProfileHttpHeader>,
    content_length: Option<u64>,
    authored_charset: Option<&str>,
    context: RuntimeExecutionContext<'_>,
) -> Result<ProfileHttpResponse, ProfileHttpError>
where
    S: Stream<Item = Result<Vec<u8>, ProfileHttpFailureKind>> + Send,
{
    let raw_body = collect_bytes(stream, content_length, context).await?;
    let content_type = normalized_content_type(&headers);
    let body = decode_strict(&raw_body, authored_charset, &headers)?;
    Ok(ProfileHttpResponse {
        body,
        status,
        headers,
        final_url,
        content_type,
        raw_body,
    })
}

async fn collect_bytes<S>(
    stream: S,
    content_length: Option<u64>,
    context: RuntimeExecutionContext<'_>,
) -> Result<Vec<u8>, ProfileHttpError>
where
    S: Stream<Item = Result<Vec<u8>, ProfileHttpFailureKind>> + Send,
{
    if context.is_cancelled() {
        context.commit_response_bytes(0, None);
        return Err(failure(ProfileHttpFailureKind::Cancelled, 0));
    }
    if context.deadline_is_expired() {
        context.mark_deadline();
        context.commit_response_bytes(0, None);
        return Err(failure(ProfileHttpFailureKind::Timeout, 0));
    }
    let allowance = context.remaining_response_bytes();
    if let Some(length) = content_length.filter(|length| *length > allowance) {
        context.commit_response_bytes(0, Some(length));
        return Err(failure(ProfileHttpFailureKind::ResponseBytesExceeded, 0));
    }
    let mut stream = Box::pin(stream);
    let mut bytes = Vec::new();
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancelled() => {
                context.commit_response_bytes(bytes.len() as u64, None);
                return Err(failure(ProfileHttpFailureKind::Cancelled, bytes.len() as u64));
            },
            item = stream.next() => item,
            _ = context.deadline_reached() => {
                context.mark_deadline();
                context.commit_response_bytes(bytes.len() as u64, None);
                return Err(failure(ProfileHttpFailureKind::Timeout, bytes.len() as u64));
            },
        };
        match next {
            None => {
                context.commit_response_bytes(bytes.len() as u64, None);
                return Ok(bytes);
            }
            Some(Err(kind)) => {
                context.commit_response_bytes(bytes.len() as u64, None);
                return Err(failure(kind, bytes.len() as u64));
            }
            Some(Ok(chunk)) => {
                let remaining = allowance.saturating_sub(bytes.len() as u64);
                if chunk.len() as u64 > remaining {
                    bytes.extend_from_slice(&chunk[..remaining as usize]);
                    let admitted = bytes.len() as u64;
                    context.commit_response_bytes(admitted, Some(1));
                    return Err(failure(
                        ProfileHttpFailureKind::ResponseBytesExceeded,
                        admitted,
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
        }
    }
}

fn failure(kind: ProfileHttpFailureKind, admitted_bytes: u64) -> ProfileHttpError {
    ProfileHttpError {
        kind,
        admitted_bytes,
    }
}

fn normalized_content_type(headers: &[ProfileHttpHeader]) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name == "content-type")
        .and_then(|h| std::str::from_utf8(&h.value).ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|v| !v.is_empty())
}

fn decode_strict(
    bytes: &[u8],
    authored: Option<&str>,
    headers: &[ProfileHttpHeader],
) -> Result<String, ProfileHttpError> {
    let authored = authored.map(resolve_encoding).transpose()?;
    let bom = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some((UTF_8, 3))
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some((UTF_16LE, 2))
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some((UTF_16BE, 2))
    } else {
        None
    };
    let mut http = Vec::new();
    for header in headers.iter().filter(|h| h.name == "content-type") {
        let value = std::str::from_utf8(&header.value)
            .map_err(|_| failure(ProfileHttpFailureKind::InvalidCharset, bytes.len() as u64))?;
        for parameter in value.split(';').skip(1) {
            let parameter = parameter.trim();
            let Some((name, value)) = parameter.split_once('=') else {
                if parameter.eq_ignore_ascii_case("charset")
                    || parameter
                        .get(..7)
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("charset"))
                        && parameter.as_bytes().get(7).is_some_and(|separator| {
                            separator.is_ascii_whitespace() || *separator == b':'
                        })
                {
                    return Err(failure(
                        ProfileHttpFailureKind::InvalidCharset,
                        bytes.len() as u64,
                    ));
                }
                continue;
            };
            if name.trim().eq_ignore_ascii_case("charset") {
                let value = value.trim();
                let label = if value.starts_with('"') || value.ends_with('"') {
                    if value.len() < 2
                        || !value.starts_with('"')
                        || !value.ends_with('"')
                        || value[1..value.len() - 1].contains('"')
                    {
                        return Err(failure(
                            ProfileHttpFailureKind::InvalidCharset,
                            bytes.len() as u64,
                        ));
                    }
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                http.push(resolve_encoding(label)?);
            }
        }
    }
    let mut identities = authored
        .into_iter()
        .chain(bom.map(|b| b.0))
        .chain(http.iter().copied());
    if let Some(first) = identities.next() {
        if identities.any(|encoding| encoding.name() != first.name()) {
            return Err(failure(
                ProfileHttpFailureKind::InvalidCharset,
                bytes.len() as u64,
            ));
        }
    }
    let encoding = authored
        .or_else(|| bom.map(|b| b.0))
        .or_else(|| http.first().copied())
        .unwrap_or(UTF_8);
    let skip = bom
        .filter(|(bom_encoding, _)| bom_encoding.name() == encoding.name())
        .map_or(0, |(_, skip)| skip);
    decode_incrementally(encoding, &bytes[skip..], bytes.len() as u64)
}

fn decode_incrementally(
    encoding: &'static Encoding,
    input: &[u8],
    observed_bytes: u64,
) -> Result<String, ProfileHttpError> {
    const DECODE_CHUNK_BYTES: usize = 4_096;
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let mut output = String::new();
    for chunk in input.chunks(DECODE_CHUNK_BYTES) {
        let maximum = decoder
            .max_utf8_buffer_length_without_replacement(chunk.len())
            .ok_or_else(|| failure(ProfileHttpFailureKind::Internal, observed_bytes))?;
        output
            .try_reserve_exact(maximum)
            .map_err(|_| failure(ProfileHttpFailureKind::Internal, observed_bytes))?;
        let (result, read) =
            decoder.decode_to_string_without_replacement(chunk, &mut output, false);
        if read != chunk.len() {
            return Err(failure(ProfileHttpFailureKind::Internal, observed_bytes));
        }
        match result {
            DecoderResult::InputEmpty => {}
            DecoderResult::Malformed(_, _) => {
                return Err(failure(
                    ProfileHttpFailureKind::MalformedText,
                    observed_bytes,
                ));
            }
            DecoderResult::OutputFull => {
                return Err(failure(ProfileHttpFailureKind::Internal, observed_bytes));
            }
        }
    }
    let maximum = decoder
        .max_utf8_buffer_length_without_replacement(0)
        .ok_or_else(|| failure(ProfileHttpFailureKind::Internal, observed_bytes))?;
    output
        .try_reserve_exact(maximum)
        .map_err(|_| failure(ProfileHttpFailureKind::Internal, observed_bytes))?;
    match decoder
        .decode_to_string_without_replacement(b"", &mut output, true)
        .0
    {
        DecoderResult::InputEmpty => Ok(output),
        DecoderResult::Malformed(_, _) => Err(failure(
            ProfileHttpFailureKind::MalformedText,
            observed_bytes,
        )),
        DecoderResult::OutputFull => Err(failure(ProfileHttpFailureKind::Internal, observed_bytes)),
    }
}

fn resolve_encoding(label: &str) -> Result<&'static Encoding, ProfileHttpError> {
    let trimmed = label.trim();
    if trimmed.is_empty()
        || !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(failure(ProfileHttpFailureKind::InvalidCharset, 0));
    }
    Encoding::for_label(trimmed.as_bytes())
        .filter(|encoding| encoding != &encoding_rs::REPLACEMENT)
        .ok_or_else(|| failure(ProfileHttpFailureKind::InvalidCharset, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_dsl::{
        documents::PhaseLimits,
        runtime::{
            allowance::{
                AllowanceDimension, AllowanceStop, InvocationAllowance, PhaseCancellationReason,
                PhaseCompletion,
            },
            cancellation::RuntimeCancellation,
        },
    };

    struct AlwaysCancelled;

    impl RuntimeCancellation for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    fn request() -> ProfileHttpRequest {
        ProfileHttpRequest {
            method: HttpMethod::Get,
            url: "https://example.test/oversized".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: 5_000,
            authored_charset: None,
        }
    }

    fn oversized_client(gate: &str) -> ScriptedProfileHttpClient {
        ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/oversized".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Gate(gate.to_string())],
            content_length: Some(PhaseLimits::BACKEND.max_response_bytes + 1),
        }])
    }

    #[tokio::test]
    async fn cancellation_precedes_oversized_length_without_polling_body() {
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        let cancellation = AlwaysCancelled;
        let context =
            RuntimeExecutionContext::with_cancellation(&cancellation).for_invocation(&allowance);
        let client = oversized_client("cancelled-body");

        let error = client
            .fetch(request(), context)
            .await
            .err()
            .expect("Cancellation must win over byte exhaustion");

        assert_eq!(error.kind, ProfileHttpFailureKind::Cancelled);
        assert_eq!(error.admitted_bytes, 0);
        assert!(!client.gate_is_waiting("cancelled-body"));
        assert!(allowance.stop().is_none());
        let report = allowance.report(PhaseCompletion::Cancelled {
            reason: PhaseCancellationReason::UserCancelled,
        });
        assert_eq!(report.usage.response_bytes, 0);
        assert!(matches!(
            report.completion,
            PhaseCompletion::Cancelled { .. }
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn expired_deadline_precedes_oversized_length_without_polling_body_and_is_marked() {
        let limits = PhaseLimits {
            max_duration_ms: 10,
            ..PhaseLimits::BACKEND
        };
        let allowance = InvocationAllowance::new(limits, true, None);
        tokio::time::advance(Duration::from_millis(10)).await;
        let context = RuntimeExecutionContext::uncancellable().for_invocation(&allowance);
        let client = oversized_client("deadline-body");

        let error = client
            .fetch(request(), context)
            .await
            .err()
            .expect("Expired deadline must win over byte exhaustion");

        assert_eq!(error.kind, ProfileHttpFailureKind::Timeout);
        assert_eq!(error.admitted_bytes, 0);
        assert!(!client.gate_is_waiting("deadline-body"));
        let Some(AllowanceStop::Exhausted(exhaustion)) = allowance.stop() else {
            panic!("expired deadline must be marked as exhaustion");
        };
        assert_eq!(exhaustion.dimension, AllowanceDimension::Duration);
        let report = allowance.report(PhaseCompletion::BudgetExhausted { exhaustion });
        assert_eq!(report.usage.response_bytes, 0);
        assert!(matches!(
            report.completion,
            PhaseCompletion::BudgetExhausted { .. }
        ));
    }
}
