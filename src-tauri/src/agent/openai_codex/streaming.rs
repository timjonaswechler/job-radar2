use super::{AgentAuthentication, ProviderCredential};
use crate::agent::models::{Model, ProviderId, ReasoningLevel};
use crate::agent::registry::{ModelRegistry, ResolvedModelRequest};
use crate::agent::{
    AgentError, AssistantContent, ContentKind, ConversationProvider, ConversationRequest,
    FinishReason, Message, ProviderEvent, ProviderEventStream, ProviderTurnCompletion, TokenUsage,
};
use futures_util::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
const FALLBACK_INSTRUCTIONS: &str = "You are a helpful assistant.";

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, AgentError>> + Send + 'static>>;

trait CredentialResolver: Send + Sync {
    fn resolve(
        &self,
        runtime_override: Option<String>,
    ) -> BoxFuture<Result<ProviderCredential, AgentError>>;
}

struct AuthenticationResolver(Arc<AgentAuthentication>);

impl CredentialResolver for AuthenticationResolver {
    fn resolve(
        &self,
        runtime_override: Option<String>,
    ) -> BoxFuture<Result<ProviderCredential, AgentError>> {
        let authentication = Arc::clone(&self.0);
        Box::pin(async move {
            authentication
                .resolve_for_request(runtime_override.as_deref())
                .await
        })
    }
}

struct CodexHttpRequest {
    url: String,
    body: Vec<u8>,
    headers: reqwest::header::HeaderMap,
    access: String,
    account_id: String,
    originator: &'static str,
    user_agent: &'static str,
    beta: &'static str,
    accept: &'static str,
    content_type: &'static str,
    session_id: String,
    request_id: String,
}

struct CodexHttpResponse {
    status: u16,
    retry_after: Option<Duration>,
    body: ByteStream,
}

trait CodexHttpClient: Send + Sync {
    fn send(&self, request: CodexHttpRequest) -> BoxFuture<Result<CodexHttpResponse, AgentError>>;
}

struct ReqwestCodexHttpClient {
    client: reqwest::Client,
}

impl ReqwestCodexHttpClient {
    fn new() -> Result<Self, AgentError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| AgentError::provider_transport())?;
        Ok(Self { client })
    }
}

impl CodexHttpClient for ReqwestCodexHttpClient {
    fn send(&self, request: CodexHttpRequest) -> BoxFuture<Result<CodexHttpResponse, AgentError>> {
        let client = self.client.clone();
        Box::pin(async move {
            let response = client
                .post(request.url)
                .headers(request.headers)
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {}", request.access),
                )
                .header("chatgpt-account-id", request.account_id)
                .header("originator", request.originator)
                .header(reqwest::header::USER_AGENT, request.user_agent)
                .header("OpenAI-Beta", request.beta)
                .header(reqwest::header::ACCEPT, request.accept)
                .header(reqwest::header::CONTENT_TYPE, request.content_type)
                .header("session-id", &request.session_id)
                .header("x-client-request-id", &request.request_id)
                .body(request.body)
                .send()
                .await
                .map_err(|_| AgentError::provider_transport())?;
            let status = response.status().as_u16();
            let retry_after = parse_retry_after_headers(response.headers());
            let body = response.bytes_stream().map(|chunk| {
                chunk
                    .map(|bytes| bytes.to_vec())
                    .map_err(|_| AgentError::provider_transport())
            });
            Ok(CodexHttpResponse {
                status,
                retry_after,
                body: Box::pin(body),
            })
        })
    }
}

pub struct OpenAiCodexProvider {
    authentication: Arc<dyn CredentialResolver>,
    registry: Arc<ModelRegistry>,
    models: Vec<Model>,
    http: Arc<dyn CodexHttpClient>,
}

impl OpenAiCodexProvider {
    pub fn new(
        authentication: Arc<AgentAuthentication>,
        registry: Arc<ModelRegistry>,
    ) -> Result<Self, AgentError> {
        let provider_id = ProviderId::new(super::PROVIDER_ID)?;
        let models = registry
            .snapshot()
            .provider(&provider_id)
            .ok_or_else(AgentError::model_unavailable)?
            .models()
            .to_vec();
        Ok(Self {
            authentication: Arc::new(AuthenticationResolver(authentication)),
            registry,
            models,
            http: Arc::new(ReqwestCodexHttpClient::new()?),
        })
    }

    pub fn for_current_user() -> Result<Self, AgentError> {
        let authentication = Arc::new(AgentAuthentication::for_current_user()?);
        let registry = Arc::new(ModelRegistry::for_current_user()?);
        Self::new(authentication, registry)
    }

    #[cfg(test)]
    fn with_adapters(
        authentication: Arc<dyn CredentialResolver>,
        http: Arc<dyn CodexHttpClient>,
    ) -> Self {
        let app_data = tempfile::tempdir().unwrap();
        let registry =
            Arc::new(ModelRegistry::from_agents_data_root(app_data.path().join("agents")).unwrap());
        Self::with_foundations(authentication, registry, http)
    }

    #[cfg(test)]
    fn with_foundations(
        authentication: Arc<dyn CredentialResolver>,
        registry: Arc<ModelRegistry>,
        http: Arc<dyn CodexHttpClient>,
    ) -> Self {
        let provider_id = ProviderId::new(super::PROVIDER_ID).unwrap();
        let models = registry
            .snapshot()
            .provider(&provider_id)
            .unwrap()
            .models()
            .to_vec();
        Self {
            authentication,
            registry,
            models,
            http,
        }
    }
}

impl ConversationProvider for OpenAiCodexProvider {
    fn models(&self) -> &[Model] {
        &self.models
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream {
        let authentication = Arc::clone(&self.authentication);
        let http = Arc::clone(&self.http);
        let resolved = self
            .registry
            .resolve_request(request.model().provider(), request.model().id());
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        tauri::async_runtime::spawn(async move {
            if sender.send(ProviderEvent::Started).await.is_err() {
                return;
            }
            let result = run_stream(
                authentication.as_ref(),
                http.as_ref(),
                resolved,
                request,
                &sender,
            )
            .await;
            if let Err(error) = result {
                let _ = sender.send(ProviderEvent::Failed(error)).await;
            }
        });
        Box::pin(futures_util::stream::unfold(
            receiver,
            |mut receiver| async { receiver.recv().await.map(|event| (event, receiver)) },
        ))
    }
}

async fn run_stream(
    authentication: &dyn CredentialResolver,
    http: &dyn CodexHttpClient,
    resolved: Result<ResolvedModelRequest, AgentError>,
    request: ConversationRequest,
    sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
) -> Result<(), AgentError> {
    let resolved = resolved?;
    if resolved.model().provider().as_str() != super::PROVIDER_ID {
        return Err(AgentError::model_unavailable());
    }
    let url = codex_responses_url(resolved.model().base_url())?;
    let body = build_request_body(&request, resolved.model())?;
    let headers = configured_headers(resolved.headers())?;
    let credential = authentication
        .resolve(resolved.api_key_override().map(str::to_owned))
        .await?;
    let session_id = clamp_identifier(request.conversation_id());
    let response = http
        .send(CodexHttpRequest {
            url,
            body,
            headers,
            access: credential.access,
            account_id: credential.account_id,
            originator: "pi",
            user_agent: "pi (job-radar; rust)",
            beta: "responses=experimental",
            accept: "text/event-stream",
            content_type: "application/json",
            request_id: session_id.clone(),
            session_id,
        })
        .await?;
    if !(200..300).contains(&response.status) {
        return Err(http_error(response).await);
    }
    translate_sse(response.body, sender).await
}

fn build_request_body(request: &ConversationRequest, model: &Model) -> Result<Vec<u8>, AgentError> {
    let mut input = Vec::new();
    for message in request.messages() {
        match message {
            Message::User(user) => input.push(json!({
                "role": "user",
                "content": [{"type": "input_text", "text": user.text()}]
            })),
            Message::Assistant(assistant) => {
                if !assistant.replay_metadata().is_empty() {
                    let replay: Vec<Value> = serde_json::from_slice(assistant.replay_metadata())
                        .map_err(|_| AgentError::invalid_provider_configuration())?;
                    input.extend(replay);
                } else {
                    let text = assistant
                        .content()
                        .iter()
                        .filter_map(|content| match content {
                            AssistantContent::Text(text) => Some(text.as_str()),
                            AssistantContent::Reasoning(_) => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    input.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": text, "annotations": []}],
                        "status": "completed"
                    }));
                }
            }
        }
    }
    let effort = reasoning_effort(model, request.reasoning_level());
    let mut body = json!({
        "model": model.id().as_str(),
        "store": false,
        "stream": true,
        "instructions": if request.system_prompt().is_empty() { FALLBACK_INSTRUCTIONS } else { request.system_prompt() },
        "input": input,
        "text": {"verbosity": "low"},
        "include": ["reasoning.encrypted_content"],
        "prompt_cache_key": clamp_identifier(request.conversation_id()),
        "tool_choice": "auto",
        "parallel_tool_calls": true
    });
    if let Some(effort) = effort {
        body["reasoning"] = json!({"effort": effort, "summary": "auto"});
    }
    serde_json::to_vec(&body).map_err(|_| AgentError::invalid_provider_configuration())
}

fn reasoning_effort(model: &Model, requested: ReasoningLevel) -> Option<String> {
    let normalized = model.normalize_reasoning(requested);
    if let Some(mapped) = model.thinking_level_map().get(&normalized) {
        return mapped.clone();
    }
    match normalized {
        ReasoningLevel::Off => None,
        ReasoningLevel::Minimal | ReasoningLevel::Low => Some("low".to_owned()),
        ReasoningLevel::Medium => Some("medium".to_owned()),
        ReasoningLevel::High => Some("high".to_owned()),
        ReasoningLevel::XHigh => Some("xhigh".to_owned()),
        ReasoningLevel::Max => Some("max".to_owned()),
    }
}

fn codex_responses_url(base_url: &str) -> Result<String, AgentError> {
    let mut url =
        url::Url::parse(base_url).map_err(|_| AgentError::invalid_provider_configuration())?;
    if url.scheme() != "https"
        || url.host_str() != Some("chatgpt.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AgentError::invalid_provider_configuration());
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with("/codex/responses") {
        path.to_owned()
    } else {
        format!("{path}/codex/responses")
    };
    url.set_path(&path);
    Ok(url.into())
}

fn configured_headers(
    headers: &BTreeMap<String, String>,
) -> Result<reqwest::header::HeaderMap, AgentError> {
    const RESERVED: [&str; 10] = [
        "authorization",
        "chatgpt-account-id",
        "originator",
        "user-agent",
        "openai-beta",
        "accept",
        "content-type",
        "session-id",
        "x-client-request-id",
        "host",
    ];
    let mut result = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        if RESERVED
            .iter()
            .any(|reserved| name.eq_ignore_ascii_case(reserved))
        {
            return Err(AgentError::invalid_provider_configuration());
        }
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| AgentError::invalid_provider_configuration())?;
        let value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|_| AgentError::invalid_provider_configuration())?;
        result.insert(name, value);
    }
    Ok(result)
}

fn clamp_identifier(value: &str) -> String {
    value.chars().take(64).collect()
}

async fn http_error(mut response: CodexHttpResponse) -> AgentError {
    let body = collect_bounded(&mut response.body, MAX_ERROR_BODY_BYTES).await;
    classify_error(response.status, response.retry_after, body.as_deref())
}

async fn collect_bounded(stream: &mut ByteStream, limit: usize) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if body.len().saturating_add(chunk.len()) > limit {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    Some(body)
}

fn classify_error(status: u16, retry_after: Option<Duration>, body: Option<&[u8]>) -> AgentError {
    let code = body
        .and_then(|body| serde_json::from_slice::<Value>(body).ok())
        .as_ref()
        .and_then(error_code)
        .map(str::to_owned);
    if status == 401
        || status == 403
        || matches!(code.as_deref(), Some("invalid_api_key" | "unauthorized"))
    {
        return AgentError::authentication();
    }
    if status == 429
        || code
            .as_deref()
            .is_some_and(|code| code.contains("rate_limit"))
    {
        return AgentError::rate_limited(retry_after);
    }
    if status == 404
        || matches!(
            code.as_deref(),
            Some("model_not_found" | "unsupported_model")
        )
    {
        return AgentError::model_unavailable();
    }
    AgentError::provider()
}

fn error_code(value: &Value) -> Option<&str> {
    if let Some(code) = value.get("code").and_then(Value::as_str) {
        return Some(code);
    }
    value.get("error").and_then(|error| match error {
        Value::String(code) => Some(code.as_str()),
        Value::Object(error) => error.get("code").and_then(Value::as_str),
        _ => None,
    })
}

fn parse_retry_after_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(milliseconds) = headers
        .get("retry-after-ms")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Some(Duration::from_millis(milliseconds).min(MAX_RETRY_AFTER));
    }
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds).min(MAX_RETRY_AFTER))
}

struct SseDecoder {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
    event_data_bytes: usize,
}

impl SseDecoder {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            data_lines: Vec::new(),
            event_data_bytes: 0,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, AgentError> {
        let mut events = Vec::new();
        for byte in chunk {
            if *byte != b'\n' {
                self.buffer.push(*byte);
                if self.buffer.len() > MAX_SSE_EVENT_BYTES {
                    return Err(AgentError::provider());
                }
                continue;
            }

            if self.buffer.last() == Some(&b'\r') {
                self.buffer.pop();
            }
            let line = std::str::from_utf8(&self.buffer).map_err(|_| AgentError::provider())?;
            if line.is_empty() {
                if !self.data_lines.is_empty() {
                    events.push(self.data_lines.join("\n"));
                    self.data_lines.clear();
                    self.event_data_bytes = 0;
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.strip_prefix(' ').unwrap_or(data);
                let separator_bytes = usize::from(!self.data_lines.is_empty());
                self.event_data_bytes = self
                    .event_data_bytes
                    .saturating_add(separator_bytes)
                    .saturating_add(data.len());
                if self.event_data_bytes > MAX_SSE_EVENT_BYTES {
                    return Err(AgentError::provider());
                }
                self.data_lines.push(data.to_owned());
            }
            self.buffer.clear();
        }
        Ok(events)
    }

    fn finish(self) -> Result<(), AgentError> {
        if self.buffer.is_empty() && self.data_lines.is_empty() {
            Ok(())
        } else {
            Err(AgentError::provider())
        }
    }
}

struct TranslationState {
    next_content_index: usize,
    slots: BTreeMap<u64, Slot>,
    replay_items: BTreeMap<u64, Value>,
    terminal_seen: bool,
}

struct Slot {
    content_index: usize,
    kind: ContentKind,
    text: String,
    finished: bool,
    pending_reasoning_separator: bool,
}

impl TranslationState {
    fn new() -> Self {
        Self {
            next_content_index: 0,
            slots: BTreeMap::new(),
            replay_items: BTreeMap::new(),
            terminal_seen: false,
        }
    }

    async fn process(
        &mut self,
        value: Value,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<Option<ProviderEvent>, AgentError> {
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "response.output_item.added" => {
                let output_index = required_index(&value)?;
                let item = value
                    .get("item")
                    .cloned()
                    .ok_or_else(AgentError::provider)?;
                self.replay_items.insert(output_index, item.clone());
                if let Some(kind) = item_content_kind(&item)? {
                    self.ensure_slot(output_index, kind, sender).await?;
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                self.delta(&value, ContentKind::Reasoning, sender).await?;
            }
            "response.reasoning_summary_part.done" => {
                let output_index = required_index(&value)?;
                self.ensure_slot(output_index, ContentKind::Reasoning, sender)
                    .await?;
                self.slots
                    .get_mut(&output_index)
                    .expect("slot ensured")
                    .pending_reasoning_separator = true;
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                self.delta(&value, ContentKind::Text, sender).await?;
            }
            "response.output_item.done" => {
                let output_index = required_index(&value)?;
                let item = value
                    .get("item")
                    .cloned()
                    .ok_or_else(AgentError::provider)?;
                self.replay_items.insert(output_index, item.clone());
                if let Some(kind) = item_content_kind(&item)? {
                    self.ensure_slot(output_index, kind, sender).await?;
                    let final_text = item_text(&item, kind);
                    if let Some(final_text) = final_text {
                        let current = self
                            .slots
                            .get(&output_index)
                            .expect("slot ensured")
                            .text
                            .clone();
                        if final_text.starts_with(&current) {
                            let remainder = &final_text[current.len()..];
                            if !remainder.is_empty() {
                                self.delta_text(output_index, kind, remainder, sender)
                                    .await?;
                            }
                        } else if final_text != current {
                            return Err(AgentError::provider());
                        }
                    }
                    self.finish_slot(output_index, sender).await?;
                }
            }
            "response.completed" | "response.done" | "response.incomplete" => {
                if self.terminal_seen {
                    return Err(AgentError::provider());
                }
                self.finish_all(sender).await?;
                let response = value.get("response").ok_or_else(AgentError::provider)?;
                let status = response.get("status").and_then(Value::as_str).unwrap_or(
                    if event_type == "response.incomplete" {
                        "incomplete"
                    } else {
                        "completed"
                    },
                );
                let finish_reason = match status {
                    "completed" => FinishReason::Completed,
                    "incomplete" => FinishReason::LengthLimit,
                    "failed" | "cancelled" => return Err(AgentError::provider()),
                    _ => return Err(AgentError::provider()),
                };
                let output = response
                    .get("output")
                    .and_then(Value::as_array)
                    .filter(|output| !output.is_empty())
                    .cloned()
                    .unwrap_or_else(|| self.replay_items.values().cloned().collect());
                let replay = serde_json::to_vec(&output).map_err(|_| AgentError::provider())?;
                let usage = parse_usage(response.get("usage"));
                self.terminal_seen = true;
                return Ok(Some(ProviderEvent::Completed(
                    ProviderTurnCompletion::with_replay(usage, finish_reason, replay),
                )));
            }
            "error" | "response.failed" => {
                self.terminal_seen = true;
                let response = value.get("response").unwrap_or(&value);
                return Ok(Some(ProviderEvent::Failed(classify_error(
                    0,
                    None,
                    serde_json::to_vec(response).ok().as_deref(),
                ))));
            }
            "response.created" | "response.in_progress" => {}
            _ => {}
        }
        Ok(None)
    }

    async fn ensure_slot(
        &mut self,
        output_index: u64,
        kind: ContentKind,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AgentError> {
        if let Some(slot) = self.slots.get(&output_index) {
            return if slot.kind == kind && !slot.finished {
                Ok(())
            } else {
                Err(AgentError::provider())
            };
        }
        if self.slots.values().any(|slot| !slot.finished) {
            return Err(AgentError::provider());
        }
        let content_index = self.next_content_index;
        self.next_content_index += 1;
        self.slots.insert(
            output_index,
            Slot {
                content_index,
                kind,
                text: String::new(),
                finished: false,
                pending_reasoning_separator: false,
            },
        );
        send(
            sender,
            ProviderEvent::ContentStarted {
                index: content_index,
                kind,
            },
        )
        .await
    }

    async fn delta(
        &mut self,
        value: &Value,
        kind: ContentKind,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AgentError> {
        let output_index = required_index(value)?;
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .ok_or_else(AgentError::provider)?;
        self.delta_text(output_index, kind, delta, sender).await
    }

    async fn delta_text(
        &mut self,
        output_index: u64,
        kind: ContentKind,
        delta: &str,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AgentError> {
        self.ensure_slot(output_index, kind, sender).await?;
        let (content_index, needs_separator) = {
            let slot = self.slots.get_mut(&output_index).expect("slot ensured");
            let needs_separator = kind == ContentKind::Reasoning
                && slot.pending_reasoning_separator
                && !slot.text.is_empty()
                && !delta.is_empty();
            slot.pending_reasoning_separator = false;
            if needs_separator {
                slot.text.push_str("\n\n");
            }
            slot.text.push_str(delta);
            (slot.content_index, needs_separator)
        };
        if needs_separator {
            send(
                sender,
                ProviderEvent::ContentDelta {
                    index: content_index,
                    delta: "\n\n".to_owned(),
                },
            )
            .await?;
        }
        send(
            sender,
            ProviderEvent::ContentDelta {
                index: content_index,
                delta: delta.to_owned(),
            },
        )
        .await
    }

    async fn finish_slot(
        &mut self,
        output_index: u64,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AgentError> {
        let slot = self
            .slots
            .get_mut(&output_index)
            .ok_or_else(AgentError::provider)?;
        if slot.finished {
            return Ok(());
        }
        slot.finished = true;
        send(
            sender,
            ProviderEvent::ContentFinished {
                index: slot.content_index,
            },
        )
        .await
    }

    async fn finish_all(
        &mut self,
        sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AgentError> {
        let open: Vec<u64> = self
            .slots
            .iter()
            .filter_map(|(index, slot)| (!slot.finished).then_some(*index))
            .collect();
        for index in open {
            self.finish_slot(index, sender).await?;
        }
        Ok(())
    }
}

async fn translate_sse(
    mut body: ByteStream,
    sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
) -> Result<(), AgentError> {
    let mut decoder = SseDecoder::new();
    let mut state = TranslationState::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        for data in decoder.push(&chunk)? {
            if data == "[DONE]" {
                continue;
            }
            if state.terminal_seen {
                return Err(AgentError::provider());
            }
            let value: Value = serde_json::from_str(&data).map_err(|_| AgentError::provider())?;
            if let Some(terminal) = state.process(value, sender).await? {
                send(sender, terminal).await?;
            }
        }
    }
    decoder.finish()?;
    if state.terminal_seen {
        Ok(())
    } else {
        Err(AgentError::provider())
    }
}

async fn send(
    sender: &tokio::sync::mpsc::Sender<ProviderEvent>,
    event: ProviderEvent,
) -> Result<(), AgentError> {
    sender.send(event).await.map_err(|_| AgentError::provider())
}

fn required_index(value: &Value) -> Result<u64, AgentError> {
    value
        .get("output_index")
        .and_then(Value::as_u64)
        .ok_or_else(AgentError::provider)
}

fn item_content_kind(item: &Value) -> Result<Option<ContentKind>, AgentError> {
    match item.get("type").and_then(Value::as_str) {
        Some("reasoning") => Ok(Some(ContentKind::Reasoning)),
        Some("message") => Ok(Some(ContentKind::Text)),
        Some(_) => Err(AgentError::provider()),
        None => Err(AgentError::provider()),
    }
}

fn item_text(item: &Value, kind: ContentKind) -> Option<String> {
    match kind {
        ContentKind::Reasoning => {
            let summary = text_array(item.get("summary"));
            let content = text_array(item.get("content"));
            (!summary.is_empty())
                .then_some(summary)
                .or_else(|| (!content.is_empty()).then_some(content))
        }
        ContentKind::Text => {
            let content = item.get("content")?.as_array()?;
            Some(
                content
                    .iter()
                    .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => part.get("text").and_then(Value::as_str),
                        Some("refusal") => part.get("refusal").and_then(Value::as_str),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            )
        }
    }
}

fn text_array(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn parse_usage(value: Option<&Value>) -> TokenUsage {
    let value = value.unwrap_or(&Value::Null);
    let input_total = value
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = value
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = value
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_write = value
        .pointer("/input_tokens_details/cache_write_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning = value
        .pointer("/output_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64);
    TokenUsage {
        input: input_total
            .saturating_sub(cache_read)
            .saturating_sub(cache_write),
        output,
        cache_read,
        cache_write,
        reasoning,
        total: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(input_total.saturating_add(output)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::models::{ModelId, ReasoningLevel};
    use crate::agent::{AgentConversation, AgentErrorCategory, ConversationEvent};
    use futures_util::StreamExt;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct FixedCredential;

    impl CredentialResolver for FixedCredential {
        fn resolve(
            &self,
            _runtime_override: Option<String>,
        ) -> BoxFuture<Result<ProviderCredential, AgentError>> {
            Box::pin(async {
                Ok(ProviderCredential {
                    access: "synthetic-access-value".to_owned(),
                    account_id: "synthetic-account-value".to_owned(),
                })
            })
        }
    }

    struct CountingCredential(Arc<AtomicUsize>);

    impl CredentialResolver for CountingCredential {
        fn resolve(
            &self,
            runtime_override: Option<String>,
        ) -> BoxFuture<Result<ProviderCredential, AgentError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            FixedCredential.resolve(runtime_override)
        }
    }

    #[derive(Clone)]
    struct SafeRequestInspection {
        url: String,
        credential_headers_match: bool,
        session_id_len: usize,
        session_ids_match: bool,
        compatibility_headers_match: bool,
        configured_headers_match: bool,
        cache_key_matches_session: bool,
        body: Value,
    }

    struct SyntheticHttp {
        responses: Mutex<VecDeque<CodexHttpResponse>>,
        inspections: Mutex<Vec<SafeRequestInspection>>,
    }

    impl SyntheticHttp {
        fn new(responses: Vec<CodexHttpResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                inspections: Mutex::new(Vec::new()),
            }
        }
    }

    impl CodexHttpClient for SyntheticHttp {
        fn send(
            &self,
            request: CodexHttpRequest,
        ) -> BoxFuture<Result<CodexHttpResponse, AgentError>> {
            self.inspections
                .lock()
                .unwrap()
                .push(SafeRequestInspection {
                    url: request.url.to_owned(),
                    credential_headers_match: request.access == "synthetic-access-value"
                        && request.account_id == "synthetic-account-value",
                    session_id_len: request.session_id.len(),
                    session_ids_match: request.session_id == request.request_id,
                    compatibility_headers_match: request.originator == "pi"
                        && request.user_agent.starts_with("pi (")
                        && request.beta == "responses=experimental"
                        && request.accept == "text/event-stream"
                        && request.content_type == "application/json",
                    configured_headers_match: request
                        .headers
                        .get("x-synthetic-generation")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        == request.url.split('/').nth(3).unwrap_or_default(),
                    cache_key_matches_session: serde_json::from_slice::<Value>(&request.body)
                        .ok()
                        .and_then(|body| body["prompt_cache_key"].as_str().map(str::to_owned))
                        .as_deref()
                        == Some(request.session_id.as_str()),
                    body: serde_json::from_slice(&request.body).unwrap(),
                });
            let response = self.responses.lock().unwrap().pop_front();
            Box::pin(async move { response.ok_or_else(AgentError::provider_transport) })
        }
    }

    fn response(status: u16, chunks: Vec<String>) -> CodexHttpResponse {
        CodexHttpResponse {
            status,
            retry_after: None,
            body: Box::pin(futures_util::stream::iter(
                chunks
                    .into_iter()
                    .map(|chunk| Ok(chunk.into_bytes()))
                    .collect::<Vec<_>>(),
            )),
        }
    }

    fn event(value: Value) -> String {
        format!("data: {}\n\n", value)
    }

    fn completed_text_response(text: &str, item_id: &str) -> CodexHttpResponse {
        response(
            200,
            vec![
                event(json!({
                    "type":"response.output_item.added","output_index":0,
                    "item":{"type":"message","id":item_id,"role":"assistant","content":[],"status":"in_progress"}
                })),
                event(json!({
                    "type":"response.output_text.delta","output_index":0,"delta":text
                })),
                event(json!({
                    "type":"response.output_item.done","output_index":0,
                    "item":{"type":"message","id":item_id,"role":"assistant","status":"completed",
                        "content":[{"type":"output_text","text":text,"annotations":[]}]}
                })),
                event(json!({
                    "type":"response.completed","response":{"status":"completed","output":[{
                        "type":"message","id":item_id,"role":"assistant","status":"completed",
                        "content":[{"type":"output_text","text":text,"annotations":[]}]}
                    ]}
                })),
            ],
        )
    }

    fn collect_turn(conversation: &mut AgentConversation, text: &str) -> Vec<ConversationEvent> {
        tauri::async_runtime::block_on(async {
            conversation
                .send(text.to_owned())
                .unwrap()
                .collect::<Vec<_>>()
                .await
        })
    }

    #[cfg(unix)]
    fn write_models(root: &std::path::Path, generation: &str, effort: &str) {
        use std::os::unix::fs::PermissionsExt;
        let document = json!({
            "providers": {
                "openai-codex": {
                    "baseUrl": format!("https://chatgpt.com/{generation}"),
                    "headers": {"x-synthetic-generation": generation},
                    "modelOverrides": {
                        "gpt-5.4": {
                            "reasoning": true,
                            "thinkingLevelMap": {"medium": effort}
                        }
                    }
                }
            }
        });
        let path = root.join("models.json");
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn reserved_configured_headers_are_rejected_before_credentials_or_http_adapters() {
        use std::os::unix::fs::PermissionsExt;

        let app_data = tempfile::tempdir().unwrap();
        let root = app_data.path().join("agents");
        let registry = Arc::new(ModelRegistry::from_agents_data_root(&root).unwrap());
        let models_path = root.join("models.json");
        std::fs::write(
            &models_path,
            br#"{"providers":{"openai-codex":{"headers":{"AuThOrIzAtIoN":"synthetic-forbidden-value"}}}}"#,
        )
        .unwrap();
        std::fs::set_permissions(&models_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        registry.reload().unwrap();
        let resolution_count = Arc::new(AtomicUsize::new(0));
        let http = Arc::new(SyntheticHttp::new(Vec::new()));
        let provider = OpenAiCodexProvider::with_foundations(
            Arc::new(CountingCredential(Arc::clone(&resolution_count))),
            registry,
            http.clone(),
        );
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Synthetic prompt");

        assert!(matches!(
            events.last(),
            Some(ConversationEvent::Failed { error })
                if error.category == crate::agent::AgentErrorCategory::InvalidConfiguration
        ));
        assert_eq!(resolution_count.load(Ordering::SeqCst), 0);
        assert!(http.inspections.lock().unwrap().is_empty());
        assert!(!format!("{events:?}").contains("synthetic-forbidden-value"));
    }

    #[test]
    #[cfg(unix)]
    fn foreign_codex_origin_is_rejected_before_credentials_or_http_adapters() {
        use std::os::unix::fs::PermissionsExt;

        let app_data = tempfile::tempdir().unwrap();
        let root = app_data.path().join("agents");
        let registry = Arc::new(ModelRegistry::from_agents_data_root(&root).unwrap());
        let models_path = root.join("models.json");
        std::fs::write(
            &models_path,
            br#"{"providers":{"openai-codex":{"baseUrl":"https://synthetic.invalid/backend-api"}}}"#,
        )
        .unwrap();
        std::fs::set_permissions(&models_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        registry.reload().unwrap();
        let resolution_count = Arc::new(AtomicUsize::new(0));
        let http = Arc::new(SyntheticHttp::new(Vec::new()));
        let provider = OpenAiCodexProvider::with_foundations(
            Arc::new(CountingCredential(Arc::clone(&resolution_count))),
            registry,
            http.clone(),
        );
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Synthetic prompt");

        assert!(matches!(
            events.last(),
            Some(ConversationEvent::Failed { error })
                if error.category == crate::agent::AgentErrorCategory::InvalidConfiguration
        ));
        assert_eq!(resolution_count.load(Ordering::SeqCst), 0);
        assert!(http.inspections.lock().unwrap().is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn request_generation_is_pinned_in_flight_and_reloaded_for_the_next_turn() {
        let app_data = tempfile::tempdir().unwrap();
        let root = app_data.path().join("agents");
        let registry = Arc::new(ModelRegistry::from_agents_data_root(&root).unwrap());
        write_models(&root, "first", "first-effort");
        registry.reload().unwrap();
        let http = Arc::new(SyntheticHttp::new(vec![
            completed_text_response("First reply", "synthetic-first-item"),
            completed_text_response("Second reply", "synthetic-second-item"),
        ]));
        let provider = OpenAiCodexProvider::with_foundations(
            Arc::new(FixedCredential),
            Arc::clone(&registry),
            http.clone(),
        );
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let first_stream = conversation.send("First".to_owned()).unwrap();
        write_models(&root, "second", "second-effort");
        registry.reload().unwrap();
        tauri::async_runtime::block_on(first_stream.collect::<Vec<_>>());
        collect_turn(&mut conversation, "Second");

        let inspections = http.inspections.lock().unwrap();
        assert_eq!(
            inspections[0].url,
            "https://chatgpt.com/first/codex/responses"
        );
        assert_eq!(inspections[0].body["reasoning"]["effort"], "first-effort");
        assert!(inspections[0].configured_headers_match);
        assert_eq!(
            inspections[1].url,
            "https://chatgpt.com/second/codex/responses"
        );
        assert_eq!(inspections[1].body["reasoning"]["effort"], "second-effort");
        assert!(inspections[1].configured_headers_match);
    }

    #[test]
    fn public_conversation_streams_codex_text_and_builds_pinned_safe_request() {
        let terminal = event(json!({
            "type": "response.completed",
            "response": {
                "status": "completed",
                "output": [{
                    "type": "message", "id": "synthetic-message", "role": "assistant",
                    "status": "completed", "content": [{"type": "output_text", "text": "Hello", "annotations": []}]
                }],
                "usage": {"input_tokens": 5, "output_tokens": 2, "total_tokens": 7,
                    "input_tokens_details": {"cached_tokens": 1},
                    "output_tokens_details": {"reasoning_tokens": 0}}
            }
        }));
        let http = Arc::new(SyntheticHttp::new(vec![response(
            200,
            vec![
                event(
                    json!({"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":"synthetic-message","role":"assistant","content":[],"status":"in_progress"}}),
                ),
                event(
                    json!({"type":"response.output_text.delta","output_index":0,"delta":"Hello"}),
                ),
                event(
                    json!({"type":"response.output_item.done","output_index":0,"item":{"type":"message","id":"synthetic-message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Hello","annotations":[]}]}}),
                ),
                terminal,
            ],
        )]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http.clone());
        let mut conversation = AgentConversation::new(
            "Be concise.".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Hi");

        assert!(matches!(events.first(), Some(ConversationEvent::Started)));
        assert!(matches!(
            events.last(),
            Some(ConversationEvent::Completed { .. })
        ));
        let inspection = http.inspections.lock().unwrap()[0].clone();
        assert_eq!(
            inspection.url,
            "https://chatgpt.com/backend-api/codex/responses"
        );
        assert!(inspection.credential_headers_match);
        assert!(inspection.session_ids_match);
        assert!(inspection.compatibility_headers_match);
        assert!(inspection.cache_key_matches_session);
        assert!(inspection.session_id_len <= 64);
        assert_eq!(inspection.body["model"], "gpt-5.4");
        assert_eq!(inspection.body["store"], false);
        assert_eq!(inspection.body["stream"], true);
        assert_eq!(inspection.body["instructions"], "Be concise.");
        assert_eq!(inspection.body["reasoning"]["effort"], "medium");
        assert_eq!(inspection.body["input"][0]["content"][0]["text"], "Hi");
        assert_eq!(conversation.messages().len(), 2);
    }

    #[test]
    fn multi_turn_requests_replay_opaque_output_and_resolve_auth_each_turn() {
        let http = Arc::new(SyntheticHttp::new(vec![
            completed_text_response("First reply", "synthetic-first-item"),
            completed_text_response("Second reply", "synthetic-second-item"),
        ]));
        let resolution_count = Arc::new(AtomicUsize::new(0));
        let provider = OpenAiCodexProvider::with_adapters(
            Arc::new(CountingCredential(Arc::clone(&resolution_count))),
            http.clone(),
        );
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        collect_turn(&mut conversation, "First");
        conversation
            .select_model(ModelId::new("gpt-5.6-luna").unwrap())
            .unwrap();
        conversation.set_reasoning_level(ReasoningLevel::Max);
        collect_turn(&mut conversation, "Second");

        assert_eq!(resolution_count.load(Ordering::SeqCst), 2);
        let inspections = http.inspections.lock().unwrap();
        assert_eq!(inspections.len(), 2);
        let second = &inspections[1].body;
        assert_eq!(second["model"], "gpt-5.6-luna");
        assert_eq!(second["reasoning"]["effort"], "max");
        assert_eq!(second["input"].as_array().unwrap().len(), 3);
        assert_eq!(second["input"][0]["content"][0]["text"], "First");
        assert_eq!(second["input"][1]["id"], "synthetic-first-item");
        assert_eq!(second["input"][2]["content"][0]["text"], "Second");
    }

    #[test]
    fn refusal_content_preserves_wire_order_and_replays_opaque_output_next_turn() {
        let refusal_item = json!({
            "type": "message",
            "id": "synthetic-refusal-item",
            "role": "assistant",
            "status": "completed",
            "content": [
                {"type": "output_text", "text": "Prefix ", "annotations": []},
                {"type": "refusal", "refusal": "cannot comply"},
                {"type": "output_text", "text": " suffix", "annotations": []}
            ]
        });
        let first = response(
            200,
            vec![
                event(json!({
                    "type":"response.output_item.added", "output_index":0,
                    "item":{"type":"message","id":"synthetic-refusal-item","role":"assistant","content":[],"status":"in_progress"}
                })),
                event(
                    json!({"type":"response.output_text.delta","output_index":0,"delta":"Prefix "}),
                ),
                event(
                    json!({"type":"response.refusal.delta","output_index":0,"delta":"cannot comply"}),
                ),
                event(
                    json!({"type":"response.output_text.delta","output_index":0,"delta":" suffix"}),
                ),
                event(json!({
                    "type":"response.output_item.done", "output_index":0, "item":refusal_item.clone()
                })),
                event(json!({
                    "type":"response.completed", "response":{"status":"completed","output":[refusal_item]}
                })),
            ],
        );
        let http = Arc::new(SyntheticHttp::new(vec![
            first,
            completed_text_response("Second reply", "synthetic-second-item"),
        ]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http.clone());
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let first_events = collect_turn(&mut conversation, "First");
        assert!(matches!(
            first_events.last(),
            Some(ConversationEvent::Completed { .. })
        ));
        let Message::Assistant(first_assistant) = &conversation.messages()[1] else {
            panic!("first response was not committed")
        };
        assert_eq!(
            first_assistant.content(),
            &[AssistantContent::Text(
                "Prefix cannot comply suffix".to_owned()
            )]
        );

        collect_turn(&mut conversation, "Second");

        let inspections = http.inspections.lock().unwrap();
        let replayed = &inspections[1].body["input"][1];
        assert_eq!(replayed["id"], "synthetic-refusal-item");
        assert_eq!(replayed["content"][0]["type"], "output_text");
        assert_eq!(replayed["content"][1]["type"], "refusal");
        assert_eq!(replayed["content"][1]["refusal"], "cannot comply");
        assert_eq!(replayed["content"][2]["type"], "output_text");
    }

    fn failed_error(events: &[ConversationEvent]) -> &AgentError {
        let Some(ConversationEvent::Failed { error }) = events.last() else {
            panic!("expected failed terminal event")
        };
        error
    }

    #[test]
    fn http_rate_limit_exposes_only_safe_category_and_bounded_retry_delay() {
        let mut rate_response = response(
            429,
            vec![json!({
                "error": {"code":"rate_limit_exceeded","message":"synthetic-sensitive-provider-body"}
            })
            .to_string()],
        );
        rate_response.retry_after = Some(Duration::from_secs(17));
        let http = Arc::new(SyntheticHttp::new(vec![rate_response]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http);
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Off,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Hello");
        let error = failed_error(&events);

        assert_eq!(error.category, AgentErrorCategory::RateLimited);
        assert_eq!(error.message, "provider rate limit reached");
        assert_eq!(error.retry_after, Some(Duration::from_secs(17)));
        assert!(!format!("{error:?}").contains("synthetic-sensitive"));
        assert!(conversation.messages().is_empty());
    }

    #[test]
    fn http_statuses_map_to_stable_redacted_categories() {
        let cases = [
            (
                401,
                AgentErrorCategory::Authentication,
                "authentication failed",
            ),
            (
                404,
                AgentErrorCategory::ModelUnavailable,
                "agent model is unavailable",
            ),
            (500, AgentErrorCategory::Provider, "provider request failed"),
        ];

        for (status, category, message) in cases {
            let provider = OpenAiCodexProvider::with_adapters(
                Arc::new(FixedCredential),
                Arc::new(SyntheticHttp::new(vec![response(
                    status,
                    vec![r#"{"error":{"message":"synthetic-sensitive-provider-body"}}"#.to_owned()],
                )])),
            );
            let mut conversation = AgentConversation::new(
                "System".to_owned(),
                provider,
                ModelId::new("gpt-5.4").unwrap(),
                ReasoningLevel::Off,
            )
            .unwrap();

            let events = collect_turn(&mut conversation, "Hello");
            let error = failed_error(&events);

            assert_eq!(error.category, category);
            assert_eq!(error.message, message);
            assert!(!format!("{error:?}").contains("synthetic-sensitive"));
        }
    }

    #[test]
    fn provider_failure_and_malformed_or_missing_terminal_are_redacted() {
        let cases = vec![
            response(
                200,
                vec![event(json!({
                    "type":"response.failed",
                    "response":{"error":{"code":"provider_failure","message":"synthetic-sensitive-provider-body"}}
                }))],
            ),
            response(200, vec!["data: {malformed}\n\n".to_owned()]),
            response(200, vec!["data: [DONE]\n\n".to_owned()]),
        ];

        for response in cases {
            let provider = OpenAiCodexProvider::with_adapters(
                Arc::new(FixedCredential),
                Arc::new(SyntheticHttp::new(vec![response])),
            );
            let mut conversation = AgentConversation::new(
                "System".to_owned(),
                provider,
                ModelId::new("gpt-5.4").unwrap(),
                ReasoningLevel::Off,
            )
            .unwrap();

            let events = collect_turn(&mut conversation, "Hello");
            let error = failed_error(&events);

            assert_eq!(error.category, AgentErrorCategory::Provider);
            assert_eq!(error.message, "provider request failed");
            assert!(!format!("{error:?}").contains("synthetic-sensitive"));
            assert!(conversation.messages().is_empty());
        }
    }

    #[test]
    fn transport_failure_after_output_fails_once_without_retry_or_commit() {
        let http = Arc::new(SyntheticHttp::new(vec![CodexHttpResponse {
            status: 200,
            retry_after: None,
            body: Box::pin(futures_util::stream::iter(vec![
                Ok(event(json!({
                    "type":"response.output_text.delta","output_index":0,"delta":"partial"
                }))
                .into_bytes()),
                Err(AgentError::provider_transport()),
            ])),
        }]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http.clone());
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Off,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Hello");
        let error = failed_error(&events);

        assert_eq!(error.category, AgentErrorCategory::Transport);
        assert_eq!(error.message, "provider transport is unavailable");
        assert_eq!(http.inspections.lock().unwrap().len(), 1);
        assert!(conversation.messages().is_empty());
    }

    #[test]
    fn multiple_reasoning_summary_parts_are_joined_without_trailing_separator() {
        let http = Arc::new(SyntheticHttp::new(vec![response(
            200,
            vec![
                event(json!({"type":"response.output_item.added","output_index":0,
                    "item":{"type":"reasoning","id":"synthetic-reasoning","summary":[]}})),
                event(
                    json!({"type":"response.reasoning_summary_text.delta","output_index":0,"delta":"First"}),
                ),
                event(json!({"type":"response.reasoning_summary_part.done","output_index":0})),
                event(
                    json!({"type":"response.reasoning_summary_text.delta","output_index":0,"delta":"Second"}),
                ),
                event(json!({"type":"response.reasoning_summary_part.done","output_index":0})),
                event(json!({"type":"response.output_item.done","output_index":0,
                "item":{"type":"reasoning","id":"synthetic-reasoning","summary":[
                    {"type":"summary_text","text":"First"},{"type":"summary_text","text":"Second"}
                ]}})),
                event(
                    json!({"type":"response.completed","response":{"status":"completed","output":[]}}),
                ),
            ],
        )]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http);
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::Medium,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Think");

        assert!(matches!(
            events.last(),
            Some(ConversationEvent::Completed { .. })
        ));
        let Message::Assistant(message) = &conversation.messages()[1] else {
            panic!("assistant response missing")
        };
        assert_eq!(
            message.content(),
            &[AssistantContent::Reasoning("First\n\nSecond".to_owned())]
        );
    }

    #[test]
    fn split_crlf_and_multiline_sse_stream_reasoning_and_incomplete_usage() {
        let added = event(json!({
            "type":"response.output_item.added", "output_index":0,
            "item":{"type":"reasoning","id":"synthetic-reasoning","summary":[],"encrypted_content":"synthetic-opaque"}
        }));
        let delta_json = json!({
            "type":"response.reasoning_summary_text.delta", "output_index":0,
            "delta":"Reasoned"
        })
        .to_string();
        let midpoint = delta_json.find(",\"output_index").unwrap() + 1;
        let multiline = format!(
            "data: {}\r\ndata: {}\r\n\r\n",
            &delta_json[..midpoint],
            &delta_json[midpoint..]
        );
        let done = event(json!({
            "type":"response.output_item.done", "output_index":0,
            "item":{"type":"reasoning","id":"synthetic-reasoning",
                "summary":[{"type":"summary_text","text":"Reasoned"}],
                "encrypted_content":"synthetic-opaque"}
        }));
        let terminal = event(json!({
            "type":"response.incomplete",
            "response":{"status":"incomplete","output":[],"usage":{
                "input_tokens":12,"output_tokens":4,"total_tokens":16,
                "input_tokens_details":{"cached_tokens":3,"cache_write_tokens":2},
                "output_tokens_details":{"reasoning_tokens":4}
            }}
        }));
        let wire = format!("{added}{multiline}{done}{terminal}data: [DONE]\r\n\r\n");
        let split = [1, 7, 23, wire.len() - 2];
        let mut start = 0;
        let mut chunks = Vec::new();
        for end in split {
            chunks.push(wire.as_bytes()[start..end].to_vec());
            start = end;
        }
        chunks.push(wire.as_bytes()[start..].to_vec());
        let http = Arc::new(SyntheticHttp::new(vec![CodexHttpResponse {
            status: 200,
            retry_after: None,
            body: Box::pin(futures_util::stream::iter(
                chunks.into_iter().map(Ok).collect::<Vec<_>>(),
            )),
        }]));
        let provider = OpenAiCodexProvider::with_adapters(Arc::new(FixedCredential), http);
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            ModelId::new("gpt-5.4").unwrap(),
            ReasoningLevel::High,
        )
        .unwrap();

        let events = collect_turn(&mut conversation, "Think");

        assert!(events.iter().any(|event| matches!(
            event,
            ConversationEvent::ContentDelta { delta, .. } if delta == "Reasoned"
        )));
        let Message::Assistant(message) = &conversation.messages()[1] else {
            panic!("assistant response missing")
        };
        assert_eq!(
            message.content(),
            &[AssistantContent::Reasoning("Reasoned".to_owned())]
        );
        assert_eq!(message.finish_reason(), FinishReason::LengthLimit);
        assert_eq!(
            message.usage(),
            &TokenUsage {
                input: 7,
                output: 4,
                cache_read: 3,
                cache_write: 2,
                reasoning: Some(4),
                total: 16,
            }
        );
    }

    #[test]
    fn decoder_accepts_large_transport_chunk_of_individually_bounded_events() {
        let event_wire = b"data: {}\n\n";
        let event_count = MAX_SSE_EVENT_BYTES / event_wire.len() + 2;
        let mut wire = Vec::with_capacity(event_count * event_wire.len());
        for _ in 0..event_count {
            wire.extend_from_slice(event_wire);
        }
        assert!(wire.len() > MAX_SSE_EVENT_BYTES);

        let mut decoder = SseDecoder::new();
        let events = decoder.push(&wire).unwrap();

        assert_eq!(events.len(), event_count);
        assert!(events.iter().all(|event| event == "{}"));
        decoder.finish().unwrap();
    }

    #[test]
    fn decoder_rejects_one_oversized_sse_event() {
        let mut wire = b"data: ".to_vec();
        wire.extend(std::iter::repeat_n(b'x', MAX_SSE_EVENT_BYTES + 1));
        wire.extend_from_slice(b"\n\n");

        let mut decoder = SseDecoder::new();

        assert_eq!(
            decoder.push(&wire).unwrap_err().category,
            AgentErrorCategory::Provider
        );
    }

    #[test]
    fn session_identifiers_and_retry_headers_are_safely_bounded() {
        assert_eq!(clamp_identifier(&"x".repeat(80)).len(), 64);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after-ms", "2500".parse().unwrap());
        assert_eq!(
            parse_retry_after_headers(&headers),
            Some(Duration::from_millis(2500))
        );
        headers.insert("retry-after-ms", "999999999".parse().unwrap());
        assert_eq!(parse_retry_after_headers(&headers), Some(MAX_RETRY_AFTER));
        headers.remove("retry-after-ms");
        headers.insert(reqwest::header::RETRY_AFTER, "19".parse().unwrap());
        assert_eq!(
            parse_retry_after_headers(&headers),
            Some(Duration::from_secs(19))
        );
    }
}
