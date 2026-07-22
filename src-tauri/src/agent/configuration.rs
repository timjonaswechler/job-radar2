use crate::agent::auth::StoredAuthenticationKind;
use crate::agent::models::ReasoningLevel;
use crate::agent::openai_codex::{
    AgentAuthentication, AuthFuture, AuthInteraction, BrowserAuthorization, DeviceAuthorization,
    LoginMethod, SecretAuthorizationInput,
};
use crate::agent::providers::AuthenticationMethod;
use crate::agent::{
    AgentError, ConversationProvider, ConversationRequest, ModelRegistry, ProviderEvent,
    ProviderEventStream,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const OPENAI_CODEX_PROVIDER: &str = "openai-codex";
const OAUTH_CALLBACK_ADDRESS: &str = "127.0.0.1:1455";
const OAUTH_CALLBACK_PATH: &str = "/auth/callback";
const OAUTH_CALLBACK_MAX_REQUEST_BYTES: usize = 8 * 1024;
const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const OAUTH_CALLBACK_READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationKind {
    ApiKey,
    Subscription,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationState {
    Ready,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationDiagnostic {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelStatus {
    pub id: String,
    pub display_name: String,
    pub reasoning_levels: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigurationStatus {
    pub id: String,
    pub display_name: String,
    pub authentication_methods: Vec<AuthenticationKind>,
    pub active_authentication: Option<AuthenticationKind>,
    pub configured_by_models_file: bool,
    pub available: bool,
    pub models: Vec<AgentModelStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfigurationStatus {
    pub providers: Vec<ProviderConfigurationStatus>,
    pub authentication_configuration: ConfigurationState,
    pub model_configuration: ConfigurationState,
    pub diagnostics: Vec<ConfigurationDiagnostic>,
}

/// A write-only command input. It deliberately implements neither `Serialize` nor `Debug`.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct SecretApiKeyInput(String);

impl SecretApiKeyInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfigurationError {
    pub code: &'static str,
    pub message: &'static str,
}

impl std::fmt::Display for AgentConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for AgentConfigurationError {}

impl AgentConfigurationError {
    pub(crate) fn unavailable() -> Self {
        invalid_configuration()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenError;

pub trait AgentDataFolderOpener: Send + Sync {
    fn open(&self, path: &Path) -> Result<(), OpenError>;
}

pub trait ExternalUrlOpener: Send + Sync {
    fn open(&self, url: &str) -> Result<(), OpenError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionLoginStage {
    Starting,
    OpeningBrowser,
    WaitingForBrowser,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionLoginProgress {
    pub provider_id: String,
    pub stage: SubscriptionLoginStage,
}

pub trait SubscriptionLoginProgressReporter: Send + Sync {
    fn report(&self, progress: SubscriptionLoginProgress);
}

struct AuthenticationState {
    authentication: Option<Arc<AgentAuthentication>>,
    diagnostic: Option<ConfigurationDiagnostic>,
}

#[derive(Default)]
struct LoginCancellation {
    // 0 = cancellable, 1 = cancelled, 2 = finalizing and no longer cancellable.
    state: std::sync::atomic::AtomicU8,
}

impl LoginCancellation {
    fn cancel(&self) -> bool {
        self.state
            .compare_exchange(
                0,
                1,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }

    fn is_cancelled(&self) -> bool {
        self.state.load(std::sync::atomic::Ordering::Acquire) == 1
    }

    fn begin_finalizing(&self) -> bool {
        self.state
            .compare_exchange(
                0,
                2,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }
}

pub(crate) struct ConfiguredAgentChatProvider {
    configuration: Arc<AgentConfiguration>,
    models: Vec<crate::agent::models::Model>,
}

impl ConversationProvider for ConfiguredAgentChatProvider {
    fn models(&self) -> &[crate::agent::models::Model] {
        &self.models
    }

    fn model_snapshot(&self) -> Vec<crate::agent::models::Model> {
        let provider_id = crate::agent::models::ProviderId::new(OPENAI_CODEX_PROVIDER)
            .expect("built-in provider identifier must be valid");
        self.configuration
            .registry
            .snapshot()
            .provider(&provider_id)
            .map(|provider| provider.models().to_vec())
            .unwrap_or_default()
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream {
        match self.configuration.conversation_provider() {
            Ok(provider) => provider.stream(request),
            Err(_) => Box::pin(futures_util::stream::iter(vec![
                ProviderEvent::Started,
                ProviderEvent::Failed(AgentError::fixed(
                    crate::agent::AgentErrorCategory::Authentication,
                    "authentication is unavailable",
                )),
            ])),
        }
    }
}

pub struct AgentConfiguration {
    agents_data_root: PathBuf,
    authentication: RwLock<AuthenticationState>,
    registry: Arc<ModelRegistry>,
    configuration_change: RwLock<()>,
    logins: Mutex<HashMap<String, Arc<LoginCancellation>>>,
}

struct LoginRegistration<'a> {
    logins: &'a Mutex<HashMap<String, Arc<LoginCancellation>>>,
    provider_id: String,
}

impl Drop for LoginRegistration<'_> {
    fn drop(&mut self) {
        self.logins
            .lock()
            .expect("login state lock poisoned")
            .remove(&self.provider_id);
    }
}

impl AgentConfiguration {
    pub fn for_current_user() -> Result<Self, AgentConfigurationError> {
        #[cfg(target_os = "macos")]
        {
            let location = crate::app::paths::current_user_app_data_location()
                .map_err(|_| invalid_configuration())?;
            Self::from_agents_data_root(location.root.join("agents"))
        }

        #[cfg(not(target_os = "macos"))]
        Err(invalid_configuration())
    }

    pub fn from_agents_data_root(
        agents_data_root: impl AsRef<Path>,
    ) -> Result<Self, AgentConfigurationError> {
        let agents_data_root = agents_data_root.as_ref().to_path_buf();
        let registry = Arc::new(
            ModelRegistry::from_agents_data_root(&agents_data_root)
                .map_err(|_| invalid_configuration())?,
        );
        let (authentication, diagnostic) =
            match AgentAuthentication::from_agents_data_root(&agents_data_root) {
                Ok(authentication) => (Some(Arc::new(authentication)), None),
                Err(error) => (None, Some(authentication_diagnostic(&error))),
            };
        Ok(Self {
            agents_data_root,
            authentication: RwLock::new(AuthenticationState {
                authentication,
                diagnostic,
            }),
            registry,
            configuration_change: RwLock::new(()),
            logins: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn configured_chat_provider(self: &Arc<Self>) -> ConfiguredAgentChatProvider {
        let provider_id = crate::agent::models::ProviderId::new(OPENAI_CODEX_PROVIDER)
            .expect("built-in provider identifier must be valid");
        let models = self
            .registry
            .snapshot()
            .provider(&provider_id)
            .map(|provider| provider.models().to_vec())
            .unwrap_or_default();
        ConfiguredAgentChatProvider {
            configuration: Arc::clone(self),
            models,
        }
    }

    fn conversation_provider(
        &self,
    ) -> Result<crate::agent::openai_codex::OpenAiCodexProvider, AgentConfigurationError> {
        crate::agent::openai_codex::OpenAiCodexProvider::new(
            self.authentication()?,
            Arc::clone(&self.registry),
        )
        .map_err(|error| map_agent_error(&error))
    }

    pub fn status(&self) -> AgentConfigurationStatus {
        let _change = self
            .configuration_change
            .read()
            .expect("configuration change lock poisoned");
        let snapshot = self.registry.snapshot();
        let authentication = self
            .authentication
            .read()
            .expect("authentication state lock poisoned");
        let mut diagnostics = Vec::new();
        if let Some(diagnostic) = authentication.diagnostic.clone() {
            diagnostics.push(diagnostic);
        }
        if self.registry.last_reload_failed() {
            diagnostics.push(model_diagnostic());
        }
        let authentication_read_failed = std::cell::Cell::new(false);

        let providers = snapshot
            .providers()
            .iter()
            .map(|provider| {
                let authentication_methods = provider
                    .authentication_methods()
                    .iter()
                    .map(|method| match method {
                        AuthenticationMethod::ApiKey => AuthenticationKind::ApiKey,
                        AuthenticationMethod::OAuth => AuthenticationKind::Subscription,
                    })
                    .collect::<Vec<_>>();
                let stored = authentication.authentication.as_ref().and_then(|auth| {
                    match auth.authentication_kind(provider.id().as_str()) {
                        Ok(kind) => kind,
                        Err(_) => {
                            authentication_read_failed.set(true);
                            None
                        }
                    }
                });
                let active_authentication = stored.map(|kind| match kind {
                    StoredAuthenticationKind::ApiKey => AuthenticationKind::ApiKey,
                    StoredAuthenticationKind::OAuth => AuthenticationKind::Subscription,
                });
                let configured_by_models_file = provider.has_configured_api_key();
                let models = provider
                    .models()
                    .iter()
                    .map(|model| AgentModelStatus {
                        id: model.id().as_str().to_owned(),
                        display_name: model.display_name().to_owned(),
                        reasoning_levels: model
                            .supported_reasoning_levels()
                            .iter()
                            .copied()
                            .map(reasoning_level_name)
                            .collect(),
                    })
                    .collect();
                ProviderConfigurationStatus {
                    id: provider.id().as_str().to_owned(),
                    display_name: provider.display_name().to_owned(),
                    available: authentication_methods.is_empty()
                        || active_authentication.is_some()
                        || configured_by_models_file,
                    authentication_methods,
                    active_authentication,
                    configured_by_models_file,
                    models,
                }
            })
            .collect();
        if authentication_read_failed.get() && authentication.diagnostic.is_none() {
            diagnostics.push(ConfigurationDiagnostic {
                code: "authentication_configuration_invalid",
                message: "authentication storage is unavailable",
            });
        }

        AgentConfigurationStatus {
            providers,
            authentication_configuration: if authentication.diagnostic.is_some()
                || authentication_read_failed.get()
            {
                ConfigurationState::Invalid
            } else {
                ConfigurationState::Ready
            },
            model_configuration: if self.registry.last_reload_failed() {
                ConfigurationState::Invalid
            } else {
                ConfigurationState::Ready
            },
            diagnostics,
        }
    }

    pub fn submit_api_key(
        &self,
        provider_id: &str,
        api_key: SecretApiKeyInput,
    ) -> Result<AgentConfigurationStatus, AgentConfigurationError> {
        let change = self
            .configuration_change
            .write()
            .expect("configuration change lock poisoned");
        let snapshot = self.registry.snapshot();
        let provider = snapshot
            .providers()
            .iter()
            .find(|provider| provider.id().as_str() == provider_id)
            .ok_or_else(provider_unavailable)?;
        if !provider
            .authentication_methods()
            .contains(&AuthenticationMethod::ApiKey)
        {
            return Err(authentication_method_unavailable());
        }
        let authentication = self.authentication()?;
        authentication
            .set_api_key(provider_id, api_key.into_inner())
            .map_err(|error| map_agent_error(&error))?;
        self.clear_authentication_diagnostic();
        drop(change);
        Ok(self.status())
    }

    pub fn remove_authentication(
        &self,
        provider_id: &str,
    ) -> Result<AgentConfigurationStatus, AgentConfigurationError> {
        let change = self
            .configuration_change
            .write()
            .expect("configuration change lock poisoned");
        if self
            .registry
            .snapshot()
            .providers()
            .iter()
            .all(|provider| provider.id().as_str() != provider_id)
        {
            return Err(provider_unavailable());
        }
        self.authentication()?
            .remove(provider_id)
            .map_err(|error| map_agent_error(&error))?;
        self.clear_authentication_diagnostic();
        drop(change);
        Ok(self.status())
    }

    pub fn reload(&self) -> AgentConfigurationStatus {
        let change = self
            .configuration_change
            .write()
            .expect("configuration change lock poisoned");
        let _ = self.registry.reload();
        let existing = self
            .authentication
            .read()
            .expect("authentication state lock poisoned")
            .authentication
            .clone();
        let result = match existing {
            Some(authentication) => authentication.reload().map(|_| authentication),
            None => {
                AgentAuthentication::from_agents_data_root(&self.agents_data_root).map(Arc::new)
            }
        };
        let mut state = self
            .authentication
            .write()
            .expect("authentication state lock poisoned");
        match result {
            Ok(authentication) => {
                state.authentication = Some(authentication);
                state.diagnostic = None;
            }
            Err(error) => {
                state.diagnostic = Some(
                    if error.message
                        == "conflicting authentication storage locations require review"
                    {
                        authentication_diagnostic(&error)
                    } else {
                        ConfigurationDiagnostic {
                            code: "authentication_configuration_invalid",
                            message: "authentication storage is unavailable",
                        }
                    },
                );
            }
        }
        drop(state);
        drop(change);
        self.status()
    }

    pub fn open_data_folder(
        &self,
        opener: &dyn AgentDataFolderOpener,
    ) -> Result<(), AgentConfigurationError> {
        opener
            .open(&self.agents_data_root)
            .map_err(|_| AgentConfigurationError {
                code: "agent_data_folder_unavailable",
                message: "agent data folder could not be opened",
            })
    }

    pub async fn login_subscription(
        &self,
        provider_id: &str,
        opener: &dyn ExternalUrlOpener,
        progress: &dyn SubscriptionLoginProgressReporter,
    ) -> Result<AgentConfigurationStatus, AgentConfigurationError> {
        if provider_id != OPENAI_CODEX_PROVIDER {
            return Err(authentication_method_unavailable());
        }
        let authentication = self.authentication()?;
        let cancellation = Arc::new(LoginCancellation::default());
        {
            let mut logins = self.logins.lock().expect("login state lock poisoned");
            if logins.contains_key(provider_id) {
                return Err(AgentConfigurationError {
                    code: "subscription_login_in_progress",
                    message: "subscription login is already in progress",
                });
            }
            logins.insert(provider_id.to_owned(), Arc::clone(&cancellation));
        }
        let _registration = LoginRegistration {
            logins: &self.logins,
            provider_id: provider_id.to_owned(),
        };
        report(progress, provider_id, SubscriptionLoginStage::Starting);
        let mut interaction = BrowserLoginInteraction {
            provider_id,
            opener,
            progress,
            cancellation: Arc::clone(&cancellation),
        };
        let result = authentication.login(&mut interaction).await;
        match result {
            Ok(()) => {
                self.clear_authentication_diagnostic();
                report(progress, provider_id, SubscriptionLoginStage::Completed);
                Ok(self.status())
            }
            Err(_) if cancellation.is_cancelled() => {
                report(progress, provider_id, SubscriptionLoginStage::Cancelled);
                Err(login_cancelled())
            }
            Err(error) => {
                report(progress, provider_id, SubscriptionLoginStage::Failed);
                Err(map_agent_error(&error))
            }
        }
    }

    pub fn cancel_subscription_login(&self, provider_id: &str) -> bool {
        let cancellation = self
            .logins
            .lock()
            .expect("login state lock poisoned")
            .get(provider_id)
            .cloned();
        cancellation.is_some_and(|cancellation| cancellation.cancel())
    }

    fn authentication(&self) -> Result<Arc<AgentAuthentication>, AgentConfigurationError> {
        self.authentication
            .read()
            .expect("authentication state lock poisoned")
            .authentication
            .clone()
            .ok_or(AgentConfigurationError {
                code: "authentication_configuration_invalid",
                message: "authentication storage is unavailable",
            })
    }

    fn clear_authentication_diagnostic(&self) {
        self.authentication
            .write()
            .expect("authentication state lock poisoned")
            .diagnostic = None;
    }
}

struct BrowserLoginInteraction<'a> {
    provider_id: &'a str,
    opener: &'a dyn ExternalUrlOpener,
    progress: &'a dyn SubscriptionLoginProgressReporter,
    cancellation: Arc<LoginCancellation>,
}

impl AuthInteraction for BrowserLoginInteraction<'_> {
    fn select_login_method(&mut self) -> AuthFuture<'_, LoginMethod> {
        Box::pin(async { Ok(LoginMethod::Browser) })
    }

    fn authorize_browser(
        &mut self,
        authorization: BrowserAuthorization,
    ) -> AuthFuture<'_, SecretAuthorizationInput> {
        Box::pin(async move {
            if self.cancellation.is_cancelled() {
                return Err(cancelled_agent_error());
            }
            let address: SocketAddr = OAUTH_CALLBACK_ADDRESS
                .parse()
                .map_err(|_| invalid_login_agent_error())?;
            let listener = TcpListener::bind(address).map_err(|_| invalid_login_agent_error())?;
            listener
                .set_nonblocking(true)
                .map_err(|_| invalid_login_agent_error())?;
            report(
                self.progress,
                self.provider_id,
                SubscriptionLoginStage::OpeningBrowser,
            );
            self.opener
                .open(authorization.url())
                .map_err(|_| invalid_login_agent_error())?;
            report(
                self.progress,
                self.provider_id,
                SubscriptionLoginStage::WaitingForBrowser,
            );
            let input = capture_loopback_callback(&listener, &self.cancellation).await?;
            report(
                self.progress,
                self.provider_id,
                SubscriptionLoginStage::Finalizing,
            );
            Ok(input)
        })
    }

    fn display_device_code(&mut self, _: DeviceAuthorization) -> AuthFuture<'_, ()> {
        Box::pin(async { Err(invalid_login_agent_error()) })
    }
}

async fn capture_loopback_callback(
    listener: &TcpListener,
    cancellation: &LoginCancellation,
) -> Result<SecretAuthorizationInput, AgentError> {
    let deadline = Instant::now() + OAUTH_CALLBACK_TIMEOUT;
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled_agent_error());
        }
        if Instant::now() >= deadline {
            return Err(invalid_login_agent_error());
        }
        match listener.accept() {
            Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                let Some(request) = read_bounded_callback_request(
                    &mut stream,
                    deadline,
                    OAUTH_CALLBACK_READ_TIMEOUT,
                    cancellation,
                ) else {
                    let _ = stream.write_all(neutral_browser_response(false));
                    continue;
                };
                let Some(callback) = parse_callback_request(&request) else {
                    let _ = stream.write_all(neutral_browser_response(false));
                    continue;
                };
                if !cancellation.begin_finalizing() {
                    return Err(cancelled_agent_error());
                }
                stream
                    .write_all(neutral_browser_response(true))
                    .map_err(|_| invalid_login_agent_error())?;
                return Ok(SecretAuthorizationInput::new(callback));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(_) => return Err(invalid_login_agent_error()),
        }
    }
}

fn read_bounded_callback_request(
    stream: &mut TcpStream,
    deadline: Instant,
    read_timeout: Duration,
    cancellation: &LoginCancellation,
) -> Option<Vec<u8>> {
    stream.set_nonblocking(true).ok()?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 512];
    let mut last_progress = Instant::now();
    loop {
        let now = Instant::now();
        if cancellation.is_cancelled()
            || now >= deadline
            || now.saturating_duration_since(last_progress) >= read_timeout
        {
            return None;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(read) => {
                last_progress = Instant::now();
                request.extend_from_slice(&buffer[..read]);
                if request.len() > OAUTH_CALLBACK_MAX_REQUEST_BYTES {
                    return None;
                }
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    return Some(request);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

fn parse_callback_request(request: &[u8]) -> Option<String> {
    let request = std::str::from_utf8(request).ok()?;
    let request_line = request.split("\r\n").next()?;
    let mut parts = request_line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some()
        || method != "GET"
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || !target.starts_with(OAUTH_CALLBACK_PATH)
    {
        return None;
    }
    let (path, query) = target.split_once('?')?;
    if path != OAUTH_CALLBACK_PATH || query.is_empty() || target.contains('#') {
        return None;
    }
    let parameters: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
    if parameters.get("code").is_none_or(|value| value.is_empty())
        || parameters.get("state").is_none_or(|value| value.is_empty())
    {
        return None;
    }
    Some(format!("http://localhost:1455{target}"))
}

fn neutral_browser_response(success: bool) -> &'static [u8] {
    if success {
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 48\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nAuthorization received. Return to Job Radar now."
    } else {
        b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 47\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nCallback was not accepted. Return to Job Radar."
    }
}

fn report(
    reporter: &dyn SubscriptionLoginProgressReporter,
    provider_id: &str,
    stage: SubscriptionLoginStage,
) {
    reporter.report(SubscriptionLoginProgress {
        provider_id: provider_id.to_owned(),
        stage,
    });
}

fn reasoning_level_name(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Off => "off",
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::XHigh => "xhigh",
        ReasoningLevel::Max => "max",
    }
}

fn authentication_diagnostic(error: &AgentError) -> ConfigurationDiagnostic {
    let message = if error.message == "conflicting authentication storage locations require review"
    {
        "conflicting authentication storage locations require review"
    } else if error.message == "authentication is not securely configured" {
        "authentication is not securely configured"
    } else {
        "authentication storage is unavailable"
    };
    ConfigurationDiagnostic {
        code: "authentication_configuration_invalid",
        message,
    }
}

fn model_diagnostic() -> ConfigurationDiagnostic {
    ConfigurationDiagnostic {
        code: "model_configuration_invalid",
        message: "agent model configuration is invalid",
    }
}

fn invalid_configuration() -> AgentConfigurationError {
    AgentConfigurationError {
        code: "agent_configuration_invalid",
        message: "agent configuration is unavailable",
    }
}

fn provider_unavailable() -> AgentConfigurationError {
    AgentConfigurationError {
        code: "provider_unavailable",
        message: "AI provider is unavailable",
    }
}

fn authentication_method_unavailable() -> AgentConfigurationError {
    AgentConfigurationError {
        code: "authentication_method_unavailable",
        message: "authentication method is unavailable",
    }
}

fn login_cancelled() -> AgentConfigurationError {
    AgentConfigurationError {
        code: "subscription_login_cancelled",
        message: "subscription login was cancelled",
    }
}

fn map_agent_error(error: &AgentError) -> AgentConfigurationError {
    match error.message.as_str() {
        "authentication is not securely configured" => AgentConfigurationError {
            code: "authentication_configuration_invalid",
            message: "authentication is not securely configured",
        },
        "conflicting authentication storage locations require review" => AgentConfigurationError {
            code: "authentication_configuration_conflict",
            message: "conflicting authentication storage locations require review",
        },
        "authentication transport is unavailable" => AgentConfigurationError {
            code: "authentication_transport_unavailable",
            message: "authentication transport is unavailable",
        },
        _ => AgentConfigurationError {
            code: "authentication_failed",
            message: "authentication failed",
        },
    }
}

fn invalid_login_agent_error() -> AgentError {
    AgentError {
        category: crate::agent::AgentErrorCategory::InvalidConfiguration,
        message: "subscription login is unavailable".to_owned(),
        retry_after: None,
    }
}

fn cancelled_agent_error() -> AgentError {
    AgentError {
        category: crate::agent::AgentErrorCategory::Authentication,
        message: "subscription login was cancelled".to_owned(),
        retry_after: None,
    }
}

#[cfg(test)]
mod tests {
    use super::LoginCancellation;

    #[test]
    fn subscription_login_cancellation_and_finalization_are_mutually_exclusive() {
        let cancelled = LoginCancellation::default();
        assert!(cancelled.cancel());
        assert!(cancelled.is_cancelled());
        assert!(!cancelled.begin_finalizing());

        let finalizing = LoginCancellation::default();
        assert!(finalizing.begin_finalizing());
        assert!(!finalizing.cancel());
        assert!(!finalizing.is_cancelled());
    }
}
