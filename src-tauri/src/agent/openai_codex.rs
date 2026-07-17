mod streaming;

pub use self::streaming::OpenAiCodexProvider;
pub use crate::agent::auth::AuthStatus;
use crate::agent::auth::{AuthStorage, AuthStorageError, OAuthCredential};
use crate::agent::{AgentError, AgentErrorCategory};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PROVIDER_ID: &str = "openai-codex";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const DEVICE_VERIFICATION_URI: &str = "https://auth.openai.com/codex/device";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const SCOPE: &str = "openid profile email offline_access";
const JWT_AUTH_CLAIM: &str = "https://api.openai.com/auth";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub type AuthFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, AgentError>> + Send + 'a>>;

impl From<AuthStorageError> for AgentError {
    fn from(error: AuthStorageError) -> Self {
        use crate::agent::auth::AuthStorageErrorCategory;
        match error.category {
            AuthStorageErrorCategory::InvalidConfiguration => {
                AgentError::invalid_authentication_configuration()
            }
            AuthStorageErrorCategory::MigrationConflict => {
                AgentError::authentication_storage_conflict()
            }
            AuthStorageErrorCategory::Unavailable => AgentError::fixed(
                AgentErrorCategory::Authentication,
                "authentication storage is unavailable",
            ),
            AuthStorageErrorCategory::RefreshFailed => AgentError::authentication(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginMethod {
    Browser,
    DeviceCode,
}

pub struct SecretAuthorizationInput(String);

impl SecretAuthorizationInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub struct BrowserAuthorization {
    url: String,
    instructions: &'static str,
}

impl BrowserAuthorization {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn instructions(&self) -> &str {
        self.instructions
    }
}

pub struct DeviceAuthorization {
    verification_uri: &'static str,
    user_code: String,
    expires_in: Duration,
}

impl DeviceAuthorization {
    pub fn verification_uri(&self) -> &str {
        self.verification_uri
    }

    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    pub fn expires_in(&self) -> Duration {
        self.expires_in
    }
}

pub trait AuthInteraction: Send {
    fn select_login_method(&mut self) -> AuthFuture<'_, LoginMethod>;
    fn authorize_browser(
        &mut self,
        authorization: BrowserAuthorization,
    ) -> AuthFuture<'_, SecretAuthorizationInput>;
    fn display_device_code(&mut self, device_code: DeviceAuthorization) -> AuthFuture<'_, ()>;
}

struct OAuthHttpRequest {
    url: String,
    content_type: &'static str,
    body: String,
}

struct OAuthHttpResponse {
    status: u16,
    body: String,
}

impl OAuthHttpResponse {
    fn new(status: u16, body: String) -> Self {
        Self { status, body }
    }

    fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

trait OAuthHttpClient: Send + Sync {
    fn send(&self, request: OAuthHttpRequest) -> AuthFuture<'_, OAuthHttpResponse>;
}

trait OAuthRuntime: Send + Sync {
    fn now_ms(&self) -> Result<u64, AgentError>;
    fn monotonic_elapsed(&self) -> Duration;
    fn random_bytes(&self, length: usize) -> Result<Vec<u8>, AgentError>;
    fn sleep(&self, duration: Duration) -> AuthFuture<'_, ()>;
}

struct SystemOAuthRuntime {
    started: std::time::Instant,
}

impl SystemOAuthRuntime {
    fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl OAuthRuntime for SystemOAuthRuntime {
    fn now_ms(&self) -> Result<u64, AgentError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AgentError::invalid_authentication_configuration())?
            .as_millis()
            .try_into()
            .map_err(|_| AgentError::invalid_authentication_configuration())
    }

    fn monotonic_elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    fn random_bytes(&self, length: usize) -> Result<Vec<u8>, AgentError> {
        let mut bytes = Vec::with_capacity(length);
        while bytes.len() < length {
            bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
        }
        bytes.truncate(length);
        Ok(bytes)
    }

    fn sleep(&self, duration: Duration) -> AuthFuture<'_, ()> {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
            Ok(())
        })
    }
}

struct ReqwestOAuthHttpClient {
    client: reqwest::Client,
}

impl ReqwestOAuthHttpClient {
    fn new() -> Result<Self, AgentError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| AgentError::transport())?;
        Ok(Self { client })
    }
}

impl OAuthHttpClient for ReqwestOAuthHttpClient {
    fn send(&self, request: OAuthHttpRequest) -> AuthFuture<'_, OAuthHttpResponse> {
        Box::pin(async move {
            let response = self
                .client
                .post(&request.url)
                .header(reqwest::header::CONTENT_TYPE, request.content_type)
                .body(request.body)
                .send()
                .await
                .map_err(|_| AgentError::transport())?;
            let status = response.status().as_u16();
            let bytes = response
                .bytes()
                .await
                .map_err(|_| AgentError::transport())?;
            if bytes.len() > MAX_RESPONSE_BYTES {
                return Err(AgentError::authentication());
            }
            let body =
                String::from_utf8(bytes.to_vec()).map_err(|_| AgentError::authentication())?;
            Ok(OAuthHttpResponse::new(status, body))
        })
    }
}

pub(crate) struct ProviderCredential {
    pub(crate) access: String,
    pub(crate) account_id: String,
}

pub struct AgentAuthentication {
    storage: AuthStorage,
    http: Arc<dyn OAuthHttpClient>,
    runtime: Arc<dyn OAuthRuntime>,
}

impl AgentAuthentication {
    pub fn for_current_user() -> Result<Self, AgentError> {
        Self::with_storage(AuthStorage::for_current_user()?)
    }

    pub fn from_agents_data_root(agents_data_root: impl AsRef<Path>) -> Result<Self, AgentError> {
        Self::with_storage(AuthStorage::in_agents_data_root(agents_data_root.as_ref())?)
    }

    fn with_storage(storage: AuthStorage) -> Result<Self, AgentError> {
        Ok(Self {
            storage,
            http: Arc::new(ReqwestOAuthHttpClient::new()?),
            runtime: Arc::new(SystemOAuthRuntime::new()),
        })
    }

    #[cfg(test)]
    fn with_adapters(
        storage: AuthStorage,
        http: Arc<dyn OAuthHttpClient>,
        runtime: Arc<dyn OAuthRuntime>,
    ) -> Self {
        Self {
            storage,
            http,
            runtime,
        }
    }

    pub fn status(&self) -> Result<AuthStatus, AgentError> {
        self.storage.status(PROVIDER_ID).map_err(Into::into)
    }

    pub async fn login(&self, interaction: &mut impl AuthInteraction) -> Result<(), AgentError> {
        let credential = match interaction.select_login_method().await? {
            LoginMethod::Browser => self.login_browser(interaction).await,
            LoginMethod::DeviceCode => self.login_device(interaction).await,
        }?;
        self.storage.save(PROVIDER_ID, &credential)?;
        Ok(())
    }

    pub fn logout(&self) -> Result<(), AgentError> {
        self.storage.remove(PROVIDER_ID).map_err(Into::into)
    }

    pub(crate) async fn resolve_for_request(&self) -> Result<ProviderCredential, AgentError> {
        let http = Arc::clone(&self.http);
        let runtime = Arc::clone(&self.runtime);
        let clock_runtime = Arc::clone(&self.runtime);
        let refresh_error = Arc::new(std::sync::Mutex::new(None));
        let captured_error = Arc::clone(&refresh_error);
        let resolution = self
            .storage
            .resolve_with_refresh_using_clock(
                PROVIDER_ID,
                move || {
                    clock_runtime
                        .now_ms()
                        .map_err(|_| AuthStorageError::invalid_configuration())
                },
                move |expired| async move {
                    match refresh_credential(http.as_ref(), runtime.as_ref(), expired).await {
                        Ok(credential) => Ok(credential),
                        Err(error) => {
                            *captured_error.lock().expect("refresh error lock poisoned") =
                                Some(error);
                            Err(crate::agent::auth::AuthStorageError::refresh_failed())
                        }
                    }
                },
            )
            .await;
        let credential = match resolution {
            Ok(credential) => credential,
            Err(storage_error) => {
                if let Some(error) = refresh_error
                    .lock()
                    .expect("refresh error lock poisoned")
                    .take()
                {
                    return Err(error);
                }
                return Err(storage_error.into());
            }
        }
        .ok_or_else(AgentError::authentication)?;
        Ok(ProviderCredential {
            access: credential.access,
            account_id: credential.account_id,
        })
    }

    async fn login_browser(
        &self,
        interaction: &mut impl AuthInteraction,
    ) -> Result<OAuthCredential, AgentError> {
        let state = hex(&self.runtime.random_bytes(16)?);
        let verifier =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.runtime.random_bytes(32)?);
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(verifier.as_bytes()));
        let mut url = url::Url::parse(AUTHORIZE_URL)
            .map_err(|_| AgentError::invalid_authentication_configuration())?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", CLIENT_ID)
            .append_pair("redirect_uri", REDIRECT_URI)
            .append_pair("scope", SCOPE)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("id_token_add_organizations", "true")
            .append_pair("codex_cli_simplified_flow", "true")
            .append_pair("originator", "pi");
        let input = interaction
            .authorize_browser(BrowserAuthorization {
                url: url.into(),
                instructions:
                    "Complete authentication in the browser or paste the redirect result.",
            })
            .await?;
        let (code, returned_state) = parse_authorization_input(input)?;
        if returned_state
            .as_deref()
            .is_some_and(|returned| returned != state)
        {
            return Err(AgentError::authentication());
        }
        exchange_code(
            self.http.as_ref(),
            self.runtime.as_ref(),
            &code,
            &verifier,
            REDIRECT_URI,
        )
        .await
    }

    async fn login_device(
        &self,
        interaction: &mut impl AuthInteraction,
    ) -> Result<OAuthCredential, AgentError> {
        let response = self
            .http
            .send(json_request(
                DEVICE_USER_CODE_URL,
                serde_json::json!({ "client_id": CLIENT_ID }).to_string(),
            ))
            .await?;
        if !response.is_success() {
            return Err(AgentError::authentication());
        }
        let device: DeviceCodeResponse =
            serde_json::from_str(&response.body).map_err(|_| AgentError::authentication())?;
        let interval = parse_interval(device.interval)?.max(Duration::from_secs(1));
        if device.device_auth_id.is_empty() || device.user_code.is_empty() {
            return Err(AgentError::authentication());
        }
        let started = self.runtime.monotonic_elapsed();
        tokio::time::timeout(
            DEVICE_TIMEOUT,
            interaction.display_device_code(DeviceAuthorization {
                verification_uri: DEVICE_VERIFICATION_URI,
                user_code: device.user_code.clone(),
                expires_in: DEVICE_TIMEOUT,
            }),
        )
        .await
        .map_err(|_| AgentError::authentication())??;
        if device_deadline_elapsed(self.runtime.as_ref(), started) {
            return Err(AgentError::authentication());
        }

        let mut wait = interval;
        loop {
            if device_deadline_elapsed(self.runtime.as_ref(), started) {
                return Err(AgentError::authentication());
            }

            let remaining = device_deadline_remaining(self.runtime.as_ref(), started)
                .ok_or_else(AgentError::authentication)?;
            let response = tokio::time::timeout(
                remaining,
                self.http.send(json_request(
                    DEVICE_TOKEN_URL,
                    serde_json::json!({
                        "device_auth_id": device.device_auth_id,
                        "user_code": device.user_code,
                    })
                    .to_string(),
                )),
            )
            .await
            .map_err(|_| AgentError::authentication())??;
            if device_deadline_elapsed(self.runtime.as_ref(), started) {
                return Err(AgentError::authentication());
            }
            if response.is_success() {
                let code: DeviceTokenResponse = serde_json::from_str(&response.body)
                    .map_err(|_| AgentError::authentication())?;
                if code.authorization_code.is_empty() || code.code_verifier.is_empty() {
                    return Err(AgentError::authentication());
                }
                let remaining = device_deadline_remaining(self.runtime.as_ref(), started)
                    .ok_or_else(AgentError::authentication)?;
                let credential = tokio::time::timeout(
                    remaining,
                    exchange_code(
                        self.http.as_ref(),
                        self.runtime.as_ref(),
                        &code.authorization_code,
                        &code.code_verifier,
                        DEVICE_REDIRECT_URI,
                    ),
                )
                .await
                .map_err(|_| AgentError::authentication())??;
                return if device_deadline_elapsed(self.runtime.as_ref(), started) {
                    Err(AgentError::authentication())
                } else {
                    Ok(credential)
                };
            }
            if slow_down_error(&response.body) {
                wait = wait.saturating_add(Duration::from_secs(5));
            } else if response.status != 403
                && response.status != 404
                && !pending_error(&response.body)
            {
                return Err(AgentError::authentication());
            }

            let elapsed = self.runtime.monotonic_elapsed().saturating_sub(started);
            let remaining = DEVICE_TIMEOUT.saturating_sub(elapsed);
            self.runtime.sleep(wait.min(remaining)).await?;
        }
    }
}

async fn refresh_credential(
    http: &dyn OAuthHttpClient,
    runtime: &dyn OAuthRuntime,
    expired: OAuthCredential,
) -> Result<OAuthCredential, AgentError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", &expired.refresh)
        .append_pair("client_id", CLIENT_ID)
        .finish();
    read_token_response(http.send(form_request(TOKEN_URL, body)).await?, runtime)
}

async fn exchange_code(
    http: &dyn OAuthHttpClient,
    runtime: &dyn OAuthRuntime,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<OAuthCredential, AgentError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("code", code)
        .append_pair("code_verifier", verifier)
        .append_pair("redirect_uri", redirect_uri)
        .finish();
    read_token_response(http.send(form_request(TOKEN_URL, body)).await?, runtime)
}

fn form_request(url: &str, body: String) -> OAuthHttpRequest {
    OAuthHttpRequest {
        url: url.to_owned(),
        content_type: "application/x-www-form-urlencoded",
        body,
    }
}

fn json_request(url: &str, body: String) -> OAuthHttpRequest {
    OAuthHttpRequest {
        url: url.to_owned(),
        content_type: "application/json",
        body,
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

fn read_token_response(
    response: OAuthHttpResponse,
    runtime: &dyn OAuthRuntime,
) -> Result<OAuthCredential, AgentError> {
    if !response.is_success() {
        return Err(AgentError::authentication());
    }
    let token: TokenResponse =
        serde_json::from_str(&response.body).map_err(|_| AgentError::authentication())?;
    if token.access_token.is_empty() || token.refresh_token.is_empty() {
        return Err(AgentError::authentication());
    }
    let account_id = account_id_from_jwt(&token.access_token)?;
    let expires_at_ms = runtime
        .now_ms()?
        .checked_add(
            token
                .expires_in
                .checked_mul(1_000)
                .ok_or_else(AgentError::authentication)?,
        )
        .ok_or_else(AgentError::authentication)?;
    Ok(OAuthCredential::new(
        token.access_token,
        token.refresh_token,
        expires_at_ms,
        account_id,
    ))
}

fn account_id_from_jwt(access: &str) -> Result<String, AgentError> {
    let mut parts = access.split('.');
    let _header = parts.next();
    let payload = parts.next().ok_or_else(AgentError::authentication)?;
    if parts.next().is_none() || parts.next().is_some() {
        return Err(AgentError::authentication());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .map_err(|_| AgentError::authentication())?;
    let payload: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| AgentError::authentication())?;
    payload
        .get(JWT_AUTH_CLAIM)
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|account_id| !account_id.is_empty())
        .map(str::to_owned)
        .ok_or_else(AgentError::authentication)
}

fn parse_authorization_input(
    input: SecretAuthorizationInput,
) -> Result<(String, Option<String>), AgentError> {
    let value = input.0.trim();
    if value.is_empty() {
        return Err(AgentError::authentication());
    }
    if let Ok(url) = url::Url::parse(value) {
        let parameters: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let code = parameters
            .get("code")
            .filter(|code| !code.is_empty())
            .map(|code| code.to_string())
            .ok_or_else(AgentError::authentication)?;
        return Ok((code, parameters.get("state").map(|state| state.to_string())));
    }
    if let Some((code, state)) = value.split_once('#') {
        if code.is_empty() {
            return Err(AgentError::authentication());
        }
        return Ok((code.to_owned(), Some(state.to_owned())));
    }
    if value.contains("code=") {
        let parameters: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(value.as_bytes()).collect();
        let code = parameters
            .get("code")
            .filter(|code| !code.is_empty())
            .map(|code| code.to_string())
            .ok_or_else(AgentError::authentication)?;
        return Ok((code, parameters.get("state").map(|state| state.to_string())));
    }
    Ok((value.to_owned(), None))
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_auth_id: String,
    user_code: String,
    interval: serde_json::Value,
}

#[derive(Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}

fn parse_interval(value: serde_json::Value) -> Result<Duration, AgentError> {
    let seconds = match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(AgentError::authentication)?;
    Ok(Duration::from_secs(seconds))
}

fn provider_error_code(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    match value.get("error")? {
        serde_json::Value::String(code) => Some(code.clone()),
        serde_json::Value::Object(error) => error.get("code")?.as_str().map(str::to_owned),
        _ => None,
    }
}

fn pending_error(body: &str) -> bool {
    provider_error_code(body).as_deref() == Some("deviceauth_authorization_pending")
}

fn slow_down_error(body: &str) -> bool {
    provider_error_code(body).as_deref() == Some("slow_down")
}

fn device_deadline_elapsed(runtime: &dyn OAuthRuntime, started: Duration) -> bool {
    device_deadline_remaining(runtime, started).is_none()
}

fn device_deadline_remaining(runtime: &dyn OAuthRuntime, started: Duration) -> Option<Duration> {
    let elapsed = runtime.monotonic_elapsed().saturating_sub(started);
    (elapsed < DEVICE_TIMEOUT).then(|| DEVICE_TIMEOUT - elapsed)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    struct SyntheticRuntime {
        now_ms: AtomicU64,
        monotonic_ms: AtomicU64,
        sleeps: Mutex<Vec<Duration>>,
    }

    impl SyntheticRuntime {
        fn new(now_ms: u64) -> Self {
            Self {
                now_ms: AtomicU64::new(now_ms),
                monotonic_ms: AtomicU64::new(0),
                sleeps: Mutex::new(Vec::new()),
            }
        }
    }

    impl OAuthRuntime for SyntheticRuntime {
        fn now_ms(&self) -> Result<u64, AgentError> {
            Ok(self.now_ms.load(Ordering::SeqCst))
        }

        fn monotonic_elapsed(&self) -> Duration {
            Duration::from_millis(self.monotonic_ms.load(Ordering::SeqCst))
        }

        fn random_bytes(&self, length: usize) -> Result<Vec<u8>, AgentError> {
            Ok((0..length)
                .map(|index| (index as u8).wrapping_add(1))
                .collect())
        }

        fn sleep(&self, duration: Duration) -> AuthFuture<'_, ()> {
            self.sleeps.lock().unwrap().push(duration);
            self.monotonic_ms.fetch_add(
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
                Ordering::SeqCst,
            );
            Box::pin(async { Ok(()) })
        }
    }

    struct SyntheticHttp {
        responses: Mutex<VecDeque<OAuthHttpResponse>>,
        requests: Mutex<Vec<OAuthHttpRequest>>,
    }

    impl SyntheticHttp {
        fn new(responses: Vec<OAuthHttpResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl OAuthHttpClient for SyntheticHttp {
        fn send(&self, request: OAuthHttpRequest) -> AuthFuture<'_, OAuthHttpResponse> {
            self.requests.lock().unwrap().push(request);
            let response = self.responses.lock().unwrap().pop_front();
            Box::pin(async move { response.ok_or_else(AgentError::transport) })
        }
    }

    struct SyntheticInteraction {
        method: LoginMethod,
        authorization_url: Option<String>,
        device_code_seen: bool,
        display_delay: Option<(Arc<SyntheticRuntime>, Duration)>,
    }

    impl SyntheticInteraction {
        fn browser() -> Self {
            Self {
                method: LoginMethod::Browser,
                authorization_url: None,
                device_code_seen: false,
                display_delay: None,
            }
        }

        fn device() -> Self {
            Self {
                method: LoginMethod::DeviceCode,
                authorization_url: None,
                device_code_seen: false,
                display_delay: None,
            }
        }
    }

    impl AuthInteraction for SyntheticInteraction {
        fn select_login_method(&mut self) -> AuthFuture<'_, LoginMethod> {
            let method = self.method;
            Box::pin(async move { Ok(method) })
        }

        fn authorize_browser(
            &mut self,
            authorization: BrowserAuthorization,
        ) -> AuthFuture<'_, SecretAuthorizationInput> {
            self.authorization_url = Some(authorization.url().to_owned());
            Box::pin(async {
                Ok(SecretAuthorizationInput::new(
                    "synthetic-authorization-code",
                ))
            })
        }

        fn display_device_code(&mut self, _device_code: DeviceAuthorization) -> AuthFuture<'_, ()> {
            self.device_code_seen = true;
            if let Some((runtime, delay)) = &self.display_delay {
                runtime
                    .monotonic_ms
                    .fetch_add(delay.as_millis() as u64, Ordering::SeqCst);
            }
            Box::pin(async { Ok(()) })
        }
    }

    fn synthetic_jwt(account_suffix: &str) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(
            r#"{{"https://api.openai.com/auth":{{"chatgpt_account_id":"synthetic-account-{account_suffix}"}}}}"#
        ));
        format!("{header}.{payload}.synthetic-signature")
    }

    fn token_response(access: &str, refresh: &str, expires_in: u64) -> OAuthHttpResponse {
        OAuthHttpResponse::new(
            200,
            serde_json::json!({
                "access_token": access,
                "refresh_token": refresh,
                "expires_in": expires_in,
            })
            .to_string(),
        )
    }

    fn authentication(
        responses: Vec<OAuthHttpResponse>,
        now_ms: u64,
    ) -> (
        AgentAuthentication,
        Arc<SyntheticHttp>,
        Arc<SyntheticRuntime>,
        tempfile::TempDir,
    ) {
        let app_data = tempfile::tempdir().unwrap();
        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let http = Arc::new(SyntheticHttp::new(responses));
        let runtime = Arc::new(SyntheticRuntime::new(now_ms));
        let auth = AgentAuthentication::with_adapters(storage, http.clone(), runtime.clone());
        (auth, http, runtime, app_data)
    }

    #[test]
    fn browser_pkce_login_persists_credential_and_logout_removes_it() {
        let access = synthetic_jwt("browser");
        let (auth, http, _runtime, _app_data) = authentication(
            vec![token_response(&access, "synthetic-refresh-browser", 60)],
            1_000,
        );
        let mut interaction = SyntheticInteraction::browser();

        assert_eq!(auth.status().unwrap(), AuthStatus::NotConfigured);
        tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap();
        assert_eq!(auth.status().unwrap(), AuthStatus::Configured);

        let authorization_url =
            url::Url::parse(interaction.authorization_url.as_ref().unwrap()).unwrap();
        let parameters: std::collections::BTreeMap<_, _> =
            authorization_url.query_pairs().collect();
        assert_eq!(
            authorization_url.as_str().split('?').next().unwrap(),
            AUTHORIZE_URL
        );
        assert_eq!(
            parameters.get("response_type").map(|value| value.as_ref()),
            Some("code")
        );
        assert_eq!(
            parameters
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
        assert_eq!(
            parameters.get("scope").map(|value| value.as_ref()),
            Some(SCOPE)
        );
        assert_eq!(
            parameters.get("originator").map(|value| value.as_ref()),
            Some("pi")
        );

        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, TOKEN_URL);
        let form: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(requests[0].body.as_bytes()).collect();
        assert_eq!(
            form.get("grant_type").map(|value| value.as_ref()),
            Some("authorization_code")
        );
        assert_eq!(
            form.get("redirect_uri").map(|value| value.as_ref()),
            Some(REDIRECT_URI)
        );
        assert!(form.get("code_verifier").is_some());
        drop(requests);

        auth.logout().unwrap();
        assert_eq!(auth.status().unwrap(), AuthStatus::NotConfigured);
    }

    #[test]
    fn exact_expiry_refresh_rotates_and_persists_before_returning() {
        let initial_access = synthetic_jwt("initial");
        let rotated_access = synthetic_jwt("rotated");
        let (auth, http, runtime, _app_data) = authentication(
            vec![
                token_response(&initial_access, "synthetic-refresh-initial", 1),
                token_response(&rotated_access, "synthetic-refresh-rotated", 60),
            ],
            5_000,
        );
        let mut interaction = SyntheticInteraction::browser();
        tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap();
        runtime.now_ms.store(6_000, Ordering::SeqCst);

        let resolved = tauri::async_runtime::block_on(auth.resolve_for_request()).unwrap();
        assert!(resolved.access == rotated_access);
        assert!(resolved.account_id == "synthetic-account-rotated");
        let stored = auth.storage.load(PROVIDER_ID).unwrap().unwrap();
        assert!(stored.refresh == "synthetic-refresh-rotated");
        assert!(stored.access == rotated_access);

        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let refresh_form: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(requests[1].body.as_bytes()).collect();
        assert_eq!(
            refresh_form.get("grant_type").map(|value| value.as_ref()),
            Some("refresh_token")
        );
        assert_eq!(
            refresh_form
                .get("refresh_token")
                .map(|value| value.as_ref()),
            Some("synthetic-refresh-initial")
        );
    }

    #[test]
    fn device_login_polls_pending_response_then_exchanges_code() {
        let access = synthetic_jwt("device");
        let (auth, http, runtime, _app_data) = authentication(
            vec![
                OAuthHttpResponse::new(200, r#"{"device_auth_id":"synthetic-device-auth","user_code":"synthetic-user-code","interval":2}"#.to_owned()),
                OAuthHttpResponse::new(403, String::new()),
                OAuthHttpResponse::new(200, r#"{"authorization_code":"synthetic-device-authorization","code_verifier":"synthetic-device-verifier"}"#.to_owned()),
                token_response(&access, "synthetic-refresh-device", 60),
            ],
            10_000,
        );
        let mut interaction = SyntheticInteraction::device();

        tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap();

        assert!(interaction.device_code_seen);
        assert_eq!(
            runtime.sleeps.lock().unwrap().as_slice(),
            &[Duration::from_secs(2)]
        );
        let requests = http.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|request| request.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                DEVICE_USER_CODE_URL,
                DEVICE_TOKEN_URL,
                DEVICE_TOKEN_URL,
                TOKEN_URL,
            ]
        );
        let exchange: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(requests[3].body.as_bytes()).collect();
        assert_eq!(
            exchange.get("redirect_uri").map(|value| value.as_ref()),
            Some(DEVICE_REDIRECT_URI)
        );
    }

    #[test]
    fn device_login_clamps_zero_poll_interval_to_one_second() {
        let access = synthetic_jwt("zero-interval");
        let (auth, _http, runtime, _app_data) = authentication(
            vec![
                OAuthHttpResponse::new(200, r#"{"device_auth_id":"synthetic-device-auth","user_code":"synthetic-user-code","interval":0}"#.to_owned()),
                OAuthHttpResponse::new(403, String::new()),
                OAuthHttpResponse::new(200, r#"{"authorization_code":"synthetic-device-authorization","code_verifier":"synthetic-device-verifier"}"#.to_owned()),
                token_response(&access, "synthetic-refresh-zero-interval", 60),
            ],
            10_000,
        );
        let mut interaction = SyntheticInteraction::device();

        tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap();

        assert_eq!(
            runtime.sleeps.lock().unwrap().as_slice(),
            &[Duration::from_secs(1)]
        );
    }

    #[test]
    fn repeated_slow_down_responses_still_stop_at_the_monotonic_deadline() {
        let (auth, _http, runtime, _app_data) = authentication(
            vec![
                OAuthHttpResponse::new(200, r#"{"device_auth_id":"synthetic-device-auth","user_code":"synthetic-user-code","interval":400}"#.to_owned()),
                OAuthHttpResponse::new(400, r#"{"error":"slow_down"}"#.to_owned()),
                OAuthHttpResponse::new(400, r#"{"error":"slow_down"}"#.to_owned()),
                OAuthHttpResponse::new(403, String::new()),
            ],
            10_000,
        );
        let mut interaction = SyntheticInteraction::device();

        let error = tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap_err();

        assert_eq!(error.category, AgentErrorCategory::Authentication);
        assert_eq!(
            runtime.sleeps.lock().unwrap().as_slice(),
            &[
                Duration::from_secs(405),
                Duration::from_secs(410),
                Duration::from_secs(85),
            ]
        );
    }

    #[test]
    fn device_login_stops_at_the_monotonic_fifteen_minute_deadline() {
        let (auth, _http, runtime, _app_data) = authentication(
            vec![
                OAuthHttpResponse::new(200, r#"{"device_auth_id":"synthetic-device-auth","user_code":"synthetic-user-code","interval":899}"#.to_owned()),
                OAuthHttpResponse::new(403, String::new()),
                OAuthHttpResponse::new(403, String::new()),
            ],
            10_000,
        );
        let mut interaction = SyntheticInteraction::device();

        let error = tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap_err();

        assert_eq!(error.category, AgentErrorCategory::Authentication);
        assert_eq!(
            runtime.sleeps.lock().unwrap().iter().sum::<Duration>(),
            DEVICE_TIMEOUT
        );
    }

    #[test]
    fn device_login_counts_authorization_interaction_against_deadline() {
        let (auth, http, runtime, _app_data) = authentication(
            vec![OAuthHttpResponse::new(
                200,
                r#"{"device_auth_id":"synthetic-device-auth","user_code":"synthetic-user-code","interval":2}"#.to_owned(),
            )],
            10_000,
        );
        let mut interaction = SyntheticInteraction::device();
        interaction.display_delay = Some((runtime, DEVICE_TIMEOUT));

        let error = tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap_err();

        assert_eq!(error.category, AgentErrorCategory::Authentication);
        assert_eq!(http.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn login_preserves_redacted_transport_failures() {
        let (auth, _http, _runtime, _app_data) = authentication(Vec::new(), 1_000);
        let mut interaction = SyntheticInteraction::browser();

        let error = tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap_err();

        assert_eq!(error.category, AgentErrorCategory::Transport);
        assert_eq!(error.message, "authentication transport is unavailable");
    }

    #[test]
    fn refresh_preserves_redacted_transport_failures() {
        let initial_access = synthetic_jwt("refresh-transport");
        let (auth, _http, runtime, _app_data) = authentication(
            vec![token_response(
                &initial_access,
                "synthetic-refresh-transport",
                1,
            )],
            5_000,
        );
        let mut interaction = SyntheticInteraction::browser();
        tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap();
        runtime.now_ms.store(6_000, Ordering::SeqCst);

        let error = match tauri::async_runtime::block_on(auth.resolve_for_request()) {
            Ok(_) => panic!("expired credential unexpectedly resolved"),
            Err(error) => error,
        };

        assert_eq!(error.category, AgentErrorCategory::Transport);
        assert_eq!(error.message, "authentication transport is unavailable");
    }

    #[test]
    fn malformed_token_and_provider_failures_return_fixed_redacted_errors() {
        let (auth, _http, _runtime, _app_data) = authentication(
            vec![OAuthHttpResponse::new(
                400,
                "synthetic sensitive provider body".to_owned(),
            )],
            1_000,
        );
        let mut interaction = SyntheticInteraction::browser();
        let error = tauri::async_runtime::block_on(auth.login(&mut interaction)).unwrap_err();

        assert_eq!(error.category, AgentErrorCategory::Authentication);
        assert_eq!(error.message, "authentication failed");
        assert!(!format!("{error:?}").contains("synthetic"));
    }
}
