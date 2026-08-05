use crate::api::ApiKind;
use crate::auth::{
    AuthStorage, AuthStorageError, AuthStorageErrorCategory, StoredAuthenticationKind,
};
use crate::models::{Model, ProviderId, ReasoningLevel};
use crate::openai_codex::{
    AgentAuthentication, AuthFuture, AuthInteraction, BrowserAuthorization, DeviceAuthorization,
    LoginMethod as ProviderLoginMethod, OpenAiCodexProvider, SecretAuthorizationInput, PROVIDER_ID,
};
use crate::providers::AuthenticationMethod;
use crate::registry::ModelRegistry;
use crate::{
    AgentError, AgentErrorCategory, ConversationProvider, ConversationRequest, ProviderEvent,
    ProviderEventStream,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};

const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    CatalogOnly,
    ConfiguredOnly,
    Executable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: String,
    pub display_name: String,
    pub reasoning_levels: Vec<&'static str>,
    pub executable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub display_name: String,
    pub authentication_methods: Vec<AuthenticationKind>,
    pub active_authentication: Option<AuthenticationKind>,
    pub configured_by_models_file: bool,
    pub capability: Capability,
    pub executable: bool,
    pub models: Vec<ModelStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub providers: Vec<ProviderStatus>,
    pub authentication_configuration: ConfigurationState,
    pub model_configuration: ConfigurationState,
    pub diagnostics: Vec<Diagnostic>,
}

/// A write-only secret input. It deliberately implements neither `Serialize` nor `Debug`.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct SecretInput(String);

impl SecretInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct LoginAttemptId(String);

impl<'de> Deserialize<'de> for LoginAttemptId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl LoginAttemptId {
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        valid
            .then_some(Self(value))
            .ok_or_else(|| Error::new(ErrorKind::InvalidLoginAttempt))
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionError;

pub type InteractionFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, InteractionError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginMethod {
    Browser,
    DeviceCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginStage {
    Starting,
    OpeningBrowser,
    WaitingForBrowser,
    DisplayingDeviceCode,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginProgress {
    pub attempt_id: LoginAttemptId,
    pub provider_id: String,
    pub stage: LoginStage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenError;

pub trait DataFolderOpener: Send + Sync {
    fn open(&self, path: &Path) -> Result<(), OpenError>;
}

pub trait LoginInteraction: Send {
    fn select_method(
        &mut self,
        attempt: &LoginAttemptId,
        provider: &ProviderId,
    ) -> InteractionFuture<'_, LoginMethod>;

    fn open_url(&mut self, attempt: &LoginAttemptId, url: &str) -> Result<(), InteractionError>;

    fn display_device_code(
        &mut self,
        attempt: &LoginAttemptId,
        verification_uri: &str,
        user_code: &str,
    ) -> Result<(), InteractionError>;

    fn report(&mut self, progress: LoginProgress);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidConfiguration,
    AuthenticationConfiguration,
    AuthenticationConflict,
    AuthenticationUnavailable,
    AuthenticationTransportUnavailable,
    ProviderUnavailable,
    AuthenticationMethodUnavailable,
    ProviderExecutionUnavailable,
    InvalidLoginAttempt,
    LoginInProgress,
    LoginCancelled,
    StaleLoginAttempt,
    InteractionUnavailable,
    DataFolderUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    pub fn invalid_input() -> Self {
        Self::new(ErrorKind::InvalidConfiguration)
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            ErrorKind::InvalidConfiguration => "agent_configuration_invalid",
            ErrorKind::AuthenticationConfiguration => "authentication_configuration_invalid",
            ErrorKind::AuthenticationConflict => "authentication_configuration_conflict",
            ErrorKind::AuthenticationUnavailable => "authentication_failed",
            ErrorKind::AuthenticationTransportUnavailable => "authentication_transport_unavailable",
            ErrorKind::ProviderUnavailable => "provider_unavailable",
            ErrorKind::AuthenticationMethodUnavailable => "authentication_method_unavailable",
            ErrorKind::ProviderExecutionUnavailable => "provider_execution_unavailable",
            ErrorKind::InvalidLoginAttempt => "invalid_login_attempt",
            ErrorKind::LoginInProgress => "subscription_login_in_progress",
            ErrorKind::LoginCancelled => "subscription_login_cancelled",
            ErrorKind::StaleLoginAttempt => "stale_login_attempt",
            ErrorKind::InteractionUnavailable => "login_interaction_unavailable",
            ErrorKind::DataFolderUnavailable => "agent_data_folder_unavailable",
        }
    }

    pub fn message(&self) -> &'static str {
        match self.kind {
            ErrorKind::InvalidConfiguration => "agent configuration is unavailable",
            ErrorKind::AuthenticationConfiguration => "authentication storage is unavailable",
            ErrorKind::AuthenticationConflict => {
                "conflicting authentication storage locations require review"
            }
            ErrorKind::AuthenticationUnavailable => "authentication failed",
            ErrorKind::AuthenticationTransportUnavailable => {
                "authentication transport is unavailable"
            }
            ErrorKind::ProviderUnavailable => "AI provider is unavailable",
            ErrorKind::AuthenticationMethodUnavailable => "authentication method is unavailable",
            ErrorKind::ProviderExecutionUnavailable => "AI provider is not executable",
            ErrorKind::InvalidLoginAttempt => "login attempt identity is invalid",
            ErrorKind::LoginInProgress => "subscription login is already in progress",
            ErrorKind::LoginCancelled => "subscription login was cancelled",
            ErrorKind::StaleLoginAttempt => "login attempt is no longer active",
            ErrorKind::InteractionUnavailable => "login interaction is unavailable",
            ErrorKind::DataFolderUnavailable => "agent data folder could not be opened",
        }
    }

    fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for Error {}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Projection {
            code: &'static str,
            message: &'static str,
        }
        Projection {
            code: self.code(),
            message: self.message(),
        }
        .serialize(serializer)
    }
}

struct Generation {
    status: Status,
    registry: Arc<ModelRegistry>,
    executable_models: Vec<Model>,
    provider: Option<Arc<OpenAiCodexProvider>>,
}

struct Gateway {
    published: Arc<RwLock<Arc<Generation>>>,
    initial_models: Vec<Model>,
}

impl ConversationProvider for Gateway {
    fn models(&self) -> &[Model] {
        &self.initial_models
    }

    fn model_snapshot(&self) -> Vec<Model> {
        self.published
            .read()
            .expect("configuration generation lock poisoned")
            .executable_models
            .clone()
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream {
        let generation = Arc::clone(
            &self
                .published
                .read()
                .expect("configuration generation lock poisoned"),
        );
        match &generation.provider {
            Some(provider) => provider.stream(request),
            None => Box::pin(futures_util::stream::iter(vec![
                ProviderEvent::Started,
                ProviderEvent::Failed(AgentError::authentication_unavailable()),
            ])),
        }
    }
}

#[derive(Default)]
struct LoginCancellation {
    // 0 = cancellable, 1 = cancelled, 2 = finalizing.
    state: AtomicU8,
}

impl LoginCancellation {
    fn cancel(&self) -> bool {
        self.state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }
}

impl crate::openai_codex::loopback::Cancellation for LoginCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled()
    }

    fn begin_finalizing(&self) -> bool {
        self.state
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

struct LoginRegistration<'a> {
    logins: &'a Mutex<HashMap<LoginAttemptId, Arc<LoginCancellation>>>,
    attempt: LoginAttemptId,
}

impl Drop for LoginRegistration<'_> {
    fn drop(&mut self) {
        self.logins
            .lock()
            .expect("login state lock poisoned")
            .remove(&self.attempt);
    }
}

/// Owns one coherent, immutable provider/model/authentication publication.
///
/// Reload and credential mutations are serialized. Existing provider calls retain the generation
/// captured when `stream` is admitted; later calls observe the next fully published generation.
pub struct Configuration {
    agents_root: PathBuf,
    published: Arc<RwLock<Arc<Generation>>>,
    change: Mutex<()>,
    logins: Mutex<HashMap<LoginAttemptId, Arc<LoginCancellation>>>,
}

impl Configuration {
    pub fn new(agents_root: PathBuf) -> Result<Self, Error> {
        let generation = Arc::new(load_generation(&agents_root, None)?);
        if generation.status.model_configuration == ConfigurationState::Invalid
            && generation.registry.snapshot().providers().is_empty()
        {
            return Err(Error::new(ErrorKind::InvalidConfiguration));
        }
        Ok(Self {
            agents_root,
            published: Arc::new(RwLock::new(generation)),
            change: Mutex::new(()),
            logins: Mutex::new(HashMap::new()),
        })
    }

    pub fn status(&self) -> Status {
        self.published
            .read()
            .expect("configuration generation lock poisoned")
            .status
            .clone()
    }

    pub fn reload(&self) -> Result<Status, Error> {
        let _change = self
            .change
            .lock()
            .expect("configuration change lock poisoned");
        let previous = Arc::clone(
            &self
                .published
                .read()
                .expect("configuration generation lock poisoned"),
        );
        let generation = Arc::new(load_generation(&self.agents_root, Some(&previous))?);
        let status = generation.status.clone();
        *self
            .published
            .write()
            .expect("configuration generation lock poisoned") = generation;
        Ok(status)
    }

    pub fn set_api_key(&self, provider: ProviderId, secret: SecretInput) -> Result<Status, Error> {
        let _change = self
            .change
            .lock()
            .expect("configuration change lock poisoned");
        let current = Arc::clone(
            &self
                .published
                .read()
                .expect("configuration generation lock poisoned"),
        );
        let descriptor = current
            .registry
            .snapshot()
            .provider(&provider)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::ProviderUnavailable))?;
        if !descriptor
            .authentication_methods()
            .contains(&AuthenticationMethod::ApiKey)
        {
            return Err(Error::new(ErrorKind::AuthenticationMethodUnavailable));
        }
        let authentication = load_authentication(&self.agents_root).map_err(|error| error)?;
        authentication
            .set_api_key(provider.as_str(), secret.into_inner())
            .map_err(map_mutation_error)?;
        let generation = Arc::new(load_generation(&self.agents_root, Some(&current))?);
        let status = generation.status.clone();
        *self
            .published
            .write()
            .expect("configuration generation lock poisoned") = generation;
        Ok(status)
    }

    pub fn remove_authentication(&self, provider: ProviderId) -> Result<Status, Error> {
        let _change = self
            .change
            .lock()
            .expect("configuration change lock poisoned");
        let current = Arc::clone(
            &self
                .published
                .read()
                .expect("configuration generation lock poisoned"),
        );
        let authentication = load_authentication(&self.agents_root).map_err(|error| error)?;
        authentication
            .remove(provider.as_str())
            .map_err(map_mutation_error)?;
        let generation = Arc::new(load_generation(&self.agents_root, Some(&current))?);
        let status = generation.status.clone();
        *self
            .published
            .write()
            .expect("configuration generation lock poisoned") = generation;
        Ok(status)
    }

    pub async fn login(
        &self,
        attempt: LoginAttemptId,
        provider: ProviderId,
        interaction: &mut dyn LoginInteraction,
    ) -> Result<Status, Error> {
        if provider.as_str() != PROVIDER_ID {
            return Err(Error::new(ErrorKind::AuthenticationMethodUnavailable));
        }
        let authentication = Arc::new(load_authentication(&self.agents_root)?);
        let cancellation = Arc::new(LoginCancellation::default());
        {
            let mut logins = self.logins.lock().expect("login state lock poisoned");
            if !logins.is_empty() {
                return Err(Error::new(ErrorKind::LoginInProgress));
            }
            logins.insert(attempt.clone(), Arc::clone(&cancellation));
        }
        let _registration = LoginRegistration {
            logins: &self.logins,
            attempt: attempt.clone(),
        };
        report(interaction, &attempt, &provider, LoginStage::Starting);
        let interaction_failed = Arc::new(AtomicBool::new(false));
        let mut adapter = HostLoginAdapter {
            attempt: &attempt,
            provider: &provider,
            interaction,
            cancellation: Arc::clone(&cancellation),
            interaction_failed: Arc::clone(&interaction_failed),
        };
        let result = authentication.login(&mut adapter).await;
        if let Err(error) = result {
            if cancellation.cancelled() {
                report(interaction, &attempt, &provider, LoginStage::Cancelled);
                return Err(Error::new(ErrorKind::LoginCancelled));
            }
            if interaction_failed.load(Ordering::Acquire) {
                report(interaction, &attempt, &provider, LoginStage::Failed);
                return Err(Error::new(ErrorKind::InteractionUnavailable));
            }
            report(interaction, &attempt, &provider, LoginStage::Failed);
            return Err(map_agent_error(error));
        }
        let _change = self
            .change
            .lock()
            .expect("configuration change lock poisoned");
        let latest = Arc::clone(
            &self
                .published
                .read()
                .expect("configuration generation lock poisoned"),
        );
        let generation = Arc::new(load_generation(&self.agents_root, Some(&latest))?);
        let status = generation.status.clone();
        *self
            .published
            .write()
            .expect("configuration generation lock poisoned") = generation;
        report(interaction, &attempt, &provider, LoginStage::Completed);
        Ok(status)
    }

    pub fn cancel_login(&self, attempt: &LoginAttemptId) -> Result<(), Error> {
        let cancellation = self
            .logins
            .lock()
            .expect("login state lock poisoned")
            .get(attempt)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::StaleLoginAttempt))?;
        cancellation
            .cancel()
            .then_some(())
            .ok_or_else(|| Error::new(ErrorKind::StaleLoginAttempt))
    }

    pub fn open_data_folder(&self, opener: &dyn DataFolderOpener) -> Result<(), Error> {
        opener
            .open(&self.agents_root)
            .map_err(|_| Error::new(ErrorKind::DataFolderUnavailable))
    }

    pub fn provider(&self) -> Arc<dyn ConversationProvider> {
        let models = self
            .published
            .read()
            .expect("configuration generation lock poisoned")
            .executable_models
            .clone();
        Arc::new(Gateway {
            published: Arc::clone(&self.published),
            initial_models: models,
        })
    }
}

struct HostLoginAdapter<'a> {
    attempt: &'a LoginAttemptId,
    provider: &'a ProviderId,
    interaction: &'a mut dyn LoginInteraction,
    cancellation: Arc<LoginCancellation>,
    interaction_failed: Arc<AtomicBool>,
}

impl AuthInteraction for HostLoginAdapter<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancellation.cancelled()
    }

    fn begin_finalizing(&self) -> bool {
        crate::openai_codex::loopback::Cancellation::begin_finalizing(self.cancellation.as_ref())
    }

    fn select_login_method(&mut self) -> AuthFuture<'_, ProviderLoginMethod> {
        Box::pin(async move {
            self.interaction
                .select_method(self.attempt, self.provider)
                .await
                .map(|method| match method {
                    LoginMethod::Browser => ProviderLoginMethod::Browser,
                    LoginMethod::DeviceCode => ProviderLoginMethod::DeviceCode,
                })
                .map_err(|_| {
                    self.interaction_failed.store(true, Ordering::Release);
                    interaction_agent_error()
                })
        })
    }

    fn authorize_browser(
        &mut self,
        authorization: BrowserAuthorization,
    ) -> AuthFuture<'_, SecretAuthorizationInput> {
        Box::pin(async move {
            if self.cancellation.cancelled() {
                return Err(cancelled_agent_error());
            }
            let expected_state = url::Url::parse(authorization.url())
                .ok()
                .and_then(|url| {
                    url.query_pairs()
                        .find(|(name, _)| name == "state")
                        .map(|(_, value)| value.into_owned())
                })
                .filter(|state| !state.is_empty())
                .ok_or_else(interaction_agent_error)?;
            let listener = crate::openai_codex::loopback::bind()?;
            report(
                self.interaction,
                self.attempt,
                self.provider,
                LoginStage::OpeningBrowser,
            );
            self.interaction
                .open_url(self.attempt, authorization.url())
                .map_err(|_| {
                    self.interaction_failed.store(true, Ordering::Release);
                    interaction_agent_error()
                })?;
            report(
                self.interaction,
                self.attempt,
                self.provider,
                LoginStage::WaitingForBrowser,
            );
            let input = crate::openai_codex::loopback::capture(
                &listener,
                &expected_state,
                self.cancellation.as_ref(),
            )
            .await?;
            report(
                self.interaction,
                self.attempt,
                self.provider,
                LoginStage::Finalizing,
            );
            Ok(input)
        })
    }

    fn display_device_code(&mut self, device: DeviceAuthorization) -> AuthFuture<'_, ()> {
        Box::pin(async move {
            if self.cancellation.cancelled() {
                return Err(cancelled_agent_error());
            }
            report(
                self.interaction,
                self.attempt,
                self.provider,
                LoginStage::DisplayingDeviceCode,
            );
            self.interaction
                .display_device_code(self.attempt, device.verification_uri(), device.user_code())
                .map_err(|_| {
                    self.interaction_failed.store(true, Ordering::Release);
                    interaction_agent_error()
                })
        })
    }
}

fn report(
    interaction: &mut dyn LoginInteraction,
    attempt: &LoginAttemptId,
    provider: &ProviderId,
    stage: LoginStage,
) {
    interaction.report(LoginProgress {
        attempt_id: attempt.clone(),
        provider_id: provider.as_str().to_owned(),
        stage,
    });
}

fn interaction_agent_error() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::InvalidConfiguration,
        "login interaction is unavailable",
    )
}

fn cancelled_agent_error() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::Authentication,
        "subscription login was cancelled",
    )
}

fn load_generation(agents_root: &Path, previous: Option<&Generation>) -> Result<Generation, Error> {
    let (registry, model_invalid) = match ModelRegistry::from_agents_data_root(agents_root) {
        Ok(candidate) if !candidate.last_reload_failed() => (Arc::new(candidate), false),
        Ok(candidate) => (
            previous
                .map(|generation| Arc::clone(&generation.registry))
                .unwrap_or_else(|| Arc::new(candidate)),
            true,
        ),
        Err(_) => match previous {
            Some(generation) => (Arc::clone(&generation.registry), true),
            None => return Err(Error::new(ErrorKind::InvalidConfiguration)),
        },
    };

    let (authentication, authentication_error) = match load_authentication(agents_root) {
        Ok(authentication) => (Some(Arc::new(authentication)), None),
        Err(error) => (None, Some(error.kind)),
    };

    let snapshot = registry.snapshot();
    let mut executable_models = Vec::new();
    let providers = snapshot
        .providers()
        .iter()
        .map(|provider| {
            let active_authentication = authentication.as_ref().and_then(|auth| {
                auth.authentication_kind(provider.id().as_str())
                    .ok()
                    .flatten()
                    .map(authentication_kind)
            });
            let executable = provider.id().as_str() == PROVIDER_ID
                && active_authentication == Some(AuthenticationKind::Subscription)
                && !registry.has_request_overrides(provider.id())
                && provider.api() == ApiKind::OpenAiResponses
                && provider.default_base_url() == CODEX_BASE_URL
                && provider
                    .models()
                    .iter()
                    .all(|model| model.base_url() == CODEX_BASE_URL);
            if executable {
                executable_models.extend(provider.models().iter().cloned());
            }
            let configured = active_authentication.is_some() || provider.has_api_key_reference();
            let capability = if executable {
                Capability::Executable
            } else if configured {
                Capability::ConfiguredOnly
            } else {
                Capability::CatalogOnly
            };
            ProviderStatus {
                id: provider.id().as_str().to_owned(),
                display_name: provider.display_name().to_owned(),
                authentication_methods: provider
                    .authentication_methods()
                    .iter()
                    .copied()
                    .map(|method| match method {
                        AuthenticationMethod::ApiKey => AuthenticationKind::ApiKey,
                        AuthenticationMethod::OAuth => AuthenticationKind::Subscription,
                    })
                    .collect(),
                active_authentication,
                configured_by_models_file: provider.has_api_key_reference(),
                capability,
                executable,
                models: provider
                    .models()
                    .iter()
                    .map(|model| ModelStatus {
                        id: model.id().as_str().to_owned(),
                        display_name: model.display_name().to_owned(),
                        reasoning_levels: model
                            .supported_reasoning_levels()
                            .iter()
                            .copied()
                            .map(reasoning_level_name)
                            .collect(),
                        executable,
                    })
                    .collect(),
            }
        })
        .collect();

    let mut diagnostics = Vec::new();
    if model_invalid {
        diagnostics.push(Diagnostic {
            code: "model_configuration_invalid",
            message: "agent model configuration is invalid",
        });
    }
    if let Some(kind) = authentication_error {
        diagnostics.push(Diagnostic {
            code: Error::new(kind).code(),
            message: Error::new(kind).message(),
        });
    }
    let status = Status {
        providers,
        authentication_configuration: if authentication_error.is_some() {
            ConfigurationState::Invalid
        } else {
            ConfigurationState::Ready
        },
        model_configuration: if model_invalid {
            ConfigurationState::Invalid
        } else {
            ConfigurationState::Ready
        },
        diagnostics,
    };
    let provider = if executable_models.is_empty() {
        None
    } else {
        authentication.as_ref().and_then(|authentication| {
            OpenAiCodexProvider::new(Arc::clone(authentication), Arc::clone(&registry))
                .ok()
                .map(Arc::new)
        })
    };
    Ok(Generation {
        status,
        registry,
        executable_models,
        provider,
    })
}

fn load_authentication(agents_root: &Path) -> Result<AgentAuthentication, Error> {
    let storage =
        AuthStorage::in_agents_data_root(agents_root).map_err(|error| map_storage_error(&error))?;
    AgentAuthentication::with_storage(storage)
        .map_err(|_| Error::new(ErrorKind::AuthenticationUnavailable))
}

fn authentication_kind(kind: StoredAuthenticationKind) -> AuthenticationKind {
    match kind {
        StoredAuthenticationKind::ApiKey => AuthenticationKind::ApiKey,
        StoredAuthenticationKind::OAuth => AuthenticationKind::Subscription,
    }
}

fn reasoning_level_name(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Off => "off",
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::XHigh => "x_high",
        ReasoningLevel::Max => "max",
    }
}

fn map_storage_error(error: &AuthStorageError) -> Error {
    let kind = match error.category {
        AuthStorageErrorCategory::InvalidConfiguration => ErrorKind::AuthenticationConfiguration,
        AuthStorageErrorCategory::MigrationConflict => ErrorKind::AuthenticationConflict,
        AuthStorageErrorCategory::Unavailable | AuthStorageErrorCategory::RefreshFailed => {
            ErrorKind::AuthenticationUnavailable
        }
    };
    Error::new(kind)
}

fn map_mutation_error(error: AgentError) -> Error {
    let kind = match error.category {
        AgentErrorCategory::InvalidConfiguration => ErrorKind::AuthenticationConfiguration,
        AgentErrorCategory::Authentication => ErrorKind::AuthenticationUnavailable,
        _ => ErrorKind::InvalidConfiguration,
    };
    Error::new(kind)
}

fn map_agent_error(error: AgentError) -> Error {
    let kind = match error.category {
        AgentErrorCategory::Authentication => ErrorKind::AuthenticationUnavailable,
        AgentErrorCategory::Transport => ErrorKind::AuthenticationTransportUnavailable,
        _ => ErrorKind::ProviderExecutionUnavailable,
    };
    Error::new(kind)
}
