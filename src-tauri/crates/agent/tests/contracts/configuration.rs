#[cfg(unix)]
use agent::{
    AuthenticationKind, Capability, InteractionError, InteractionFuture, LoginAttemptId,
    LoginInteraction, LoginMethod, LoginProgress, LoginStage,
};
use agent::{Configuration, ProviderId};
use agent::{ConfigurationErrorKind as ErrorKind, ConfigurationState, SecretInput};
use std::fs;
#[cfg(unix)]
use std::future::Future;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::sync::{Arc, Mutex, Weak};
#[cfg(unix)]
use std::task::Poll;

fn configuration_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("agents");
    fs::create_dir(&root).unwrap();
    (temporary, root)
}

fn write_private(root: &std::path::Path, name: &str, document: &str) {
    let path = root.join(name);
    fs::write(&path, document).unwrap();
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn write_models(root: &std::path::Path, document: &str) {
    write_private(root, "models.json", document);
}

fn write_oauth(root: &std::path::Path) {
    write_private(
        root,
        "auth.json",
        r#"{"openai-codex":{"type":"oauth","access":"synthetic-access","refresh":"synthetic-refresh","expires":4102444800000,"accountId":"synthetic-account"}}"#,
    );
}

#[test]
fn invalid_agents_root_returns_a_typed_error_without_panicking() {
    let temporary = tempfile::tempdir().unwrap();
    let error = match Configuration::new(temporary.path().join("not-agents")) {
        Ok(_) => panic!("invalid agents root unexpectedly constructed Configuration"),
        Err(error) => error,
    };

    assert_eq!(error.kind(), ErrorKind::InvalidConfiguration);
    assert_eq!(error.code(), "agent_configuration_invalid");
}

// Executable authentication requires private persisted credentials. Native Windows storage is #294.
#[cfg(unix)]
#[test]
fn configured_providers_remain_visible_but_only_compiled_auth_combinations_are_executable() {
    let (_temporary, root) = configuration_root();
    write_models(
        &root,
        r#"{
          "providers": {
            "synthetic-provider": {
              "name": "Synthetic Provider",
              "api": "openai-responses",
              "baseUrl": "https://synthetic.invalid/v1",
              "apiKey": "synthetic-configuration-value",
              "models": [{"id": "synthetic-model", "reasoning": true}]
            },
            "openai-codex": {
              "apiKey": "synthetic-codex-key",
              "modelOverrides": {"gpt-5.4": {"name": "Configured Codex"}}
            }
          }
        }"#,
    );

    let configuration = Configuration::new(root).unwrap();
    let status = configuration.status();
    let synthetic = status
        .providers
        .iter()
        .find(|provider| provider.id == "synthetic-provider")
        .unwrap();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();

    assert!(synthetic.configured_by_models_file);
    assert_eq!(synthetic.capability, Capability::ConfiguredOnly);
    assert_eq!(codex.active_authentication, None);
    assert!(codex
        .authentication_methods
        .contains(&AuthenticationKind::ApiKey));
    assert_eq!(codex.capability, Capability::ConfiguredOnly);
    assert_eq!(
        status.authentication_configuration,
        ConfigurationState::Ready
    );
    assert_eq!(status.model_configuration, ConfigurationState::Ready);

    let models = configuration.provider().model_snapshot();
    assert!(models.is_empty());
    assert!(!models
        .iter()
        .any(|model| { model.provider() == &ProviderId::new("synthetic-provider").unwrap() }));
}

#[cfg(unix)]
#[test]
fn oauth_is_executable_while_api_key_configuration_is_not() {
    let (_temporary, root) = configuration_root();
    let configuration = Configuration::new(root.clone()).unwrap();
    write_oauth(&root);

    let status = configuration.reload().unwrap();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();

    assert_eq!(
        codex.active_authentication,
        Some(AuthenticationKind::Subscription)
    );
    assert_eq!(codex.capability, Capability::Executable);
    assert!(codex.models.iter().all(|model| model.executable));
    assert!(!configuration.provider().model_snapshot().is_empty());
}

#[cfg(unix)]
#[test]
fn codex_api_key_override_prevents_oauth_configuration_from_being_executable() {
    let (_temporary, root) = configuration_root();
    write_models(
        &root,
        r#"{"providers":{"openai-codex":{"apiKey":"synthetic-override","modelOverrides":{"gpt-5.4":{"name":"Configured Codex"}}}}}"#,
    );
    let configuration = Configuration::new(root.clone()).unwrap();
    write_oauth(&root);

    let status = configuration.reload().unwrap();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();

    assert_eq!(
        codex.active_authentication,
        Some(AuthenticationKind::Subscription)
    );
    assert!(codex.configured_by_models_file);
    assert_eq!(codex.capability, Capability::ConfiguredOnly);
    assert!(configuration.provider().model_snapshot().is_empty());
}

#[cfg(unix)]
#[test]
fn codex_request_overrides_are_visible_but_never_claimed_executable() {
    let (_temporary, root) = configuration_root();
    write_models(
        &root,
        r#"{"providers":{"openai-codex":{"headers":{"Authorization":"synthetic-header"},"modelOverrides":{"gpt-5.4":{"name":"Configured Codex"}}}}}"#,
    );
    let configuration = Configuration::new(root.clone()).unwrap();
    write_oauth(&root);

    let status = configuration.reload().unwrap();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();

    assert_eq!(codex.capability, Capability::ConfiguredOnly);
    assert!(configuration.provider().model_snapshot().is_empty());
    assert!(!serde_json::to_string(&status)
        .unwrap()
        .contains("synthetic-header"));
}

#[test]
fn reload_retains_last_good_models_but_fails_authentication_closed() {
    let (_temporary, root) = configuration_root();
    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://synthetic.invalid/v1","models":[{"id":"synthetic-model"}]}}}"#,
    );
    let configuration = Configuration::new(root.clone()).unwrap();
    write_oauth(&root);
    configuration.reload().unwrap();

    write_models(&root, r#"{"providers":{"broken":null}}"#);
    write_private(&root, "auth.json", "not-json");
    let degraded = configuration.reload().unwrap();

    assert_eq!(degraded.model_configuration, ConfigurationState::Invalid);
    assert_eq!(
        degraded.authentication_configuration,
        ConfigurationState::Invalid
    );
    assert!(degraded.providers.iter().any(|provider| {
        provider.id == "synthetic-provider"
            && provider
                .models
                .iter()
                .any(|model| model.id == "synthetic-model")
    }));
    assert!(degraded
        .providers
        .iter()
        .all(|provider| !provider.executable));
    assert!(configuration.provider().model_snapshot().is_empty());
}

#[test]
fn typed_authentication_errors_do_not_classify_by_message_or_expose_input() {
    let (_temporary, root) = configuration_root();
    write_models(
        &root,
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://synthetic.invalid/v1","models":[{"id":"synthetic-model"}]}}}"#,
    );
    let configuration = Configuration::new(root.clone()).unwrap();
    write_private(&root, "auth.json", "synthetic-malformed-auth-canary");
    configuration.reload().unwrap();

    let error = configuration
        .set_api_key(
            ProviderId::new("synthetic-provider").unwrap(),
            SecretInput::new("synthetic-secret-input-canary"),
        )
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AuthenticationConfiguration);
    assert_eq!(error.code(), "authentication_configuration_invalid");
    let serialized = serde_json::to_string(&error).unwrap();
    assert!(!serialized.contains("synthetic-malformed-auth-canary"));
    assert!(!serialized.contains("synthetic-secret-input-canary"));
}

#[cfg(unix)]
#[tokio::test]
async fn login_interaction_failures_keep_their_typed_outer_error() {
    struct FailingInteraction;

    impl LoginInteraction for FailingInteraction {
        fn select_method(
            &mut self,
            _attempt: &LoginAttemptId,
            _provider: &ProviderId,
        ) -> InteractionFuture<'_, LoginMethod> {
            Box::pin(async { Err(InteractionError) })
        }

        fn open_url(
            &mut self,
            _attempt: &LoginAttemptId,
            _url: &str,
        ) -> Result<(), InteractionError> {
            unreachable!()
        }

        fn display_device_code(
            &mut self,
            _attempt: &LoginAttemptId,
            _verification_uri: &str,
            _user_code: &str,
        ) -> Result<(), InteractionError> {
            unreachable!()
        }

        fn report(&mut self, _progress: LoginProgress) {}
    }

    let (_temporary, root) = configuration_root();
    let configuration = Configuration::new(root).unwrap();
    let error = configuration
        .login(
            LoginAttemptId::new("failing-attempt").unwrap(),
            ProviderId::new("openai-codex").unwrap(),
            &mut FailingInteraction,
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InteractionUnavailable);
    assert_eq!(error.code(), "login_interaction_unavailable");
}

#[cfg(unix)]
#[tokio::test]
async fn login_progress_and_cancellation_are_attempt_aware_and_reject_stale_attempts() {
    struct CancellingInteraction {
        configuration: Weak<Configuration>,
        method: LoginMethod,
        progress: Mutex<Vec<LoginProgress>>,
    }

    impl LoginInteraction for CancellingInteraction {
        fn select_method(
            &mut self,
            _attempt: &LoginAttemptId,
            _provider: &ProviderId,
        ) -> InteractionFuture<'_, LoginMethod> {
            let method = self.method;
            Box::pin(async move { Ok(method) })
        }

        fn open_url(
            &mut self,
            _attempt: &LoginAttemptId,
            _url: &str,
        ) -> Result<(), InteractionError> {
            panic!("a login cancelled at admission must not open a URL")
        }

        fn display_device_code(
            &mut self,
            _attempt: &LoginAttemptId,
            _verification_uri: &str,
            _user_code: &str,
        ) -> Result<(), InteractionError> {
            panic!("browser login must not display a device code")
        }

        fn report(&mut self, progress: LoginProgress) {
            if progress.stage == LoginStage::Starting {
                self.configuration
                    .upgrade()
                    .unwrap()
                    .cancel_login(&progress.attempt_id)
                    .unwrap();
            }
            self.progress.lock().unwrap().push(progress);
        }
    }

    let (_temporary, root) = configuration_root();
    let configuration = Arc::new(Configuration::new(root).unwrap());
    let attempt = LoginAttemptId::new("attempt-one").unwrap();
    let mut interaction = CancellingInteraction {
        configuration: Arc::downgrade(&configuration),
        method: LoginMethod::Browser,
        progress: Mutex::new(Vec::new()),
    };
    let error = configuration
        .login(
            attempt.clone(),
            ProviderId::new("openai-codex").unwrap(),
            &mut interaction,
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), "subscription_login_cancelled");
    assert_eq!(
        interaction
            .progress
            .lock()
            .unwrap()
            .iter()
            .map(|progress| (&progress.attempt_id, progress.stage))
            .collect::<Vec<_>>(),
        vec![
            (&attempt, LoginStage::Starting),
            (&attempt, LoginStage::Cancelled)
        ]
    );
    assert_eq!(
        configuration.cancel_login(&attempt).unwrap_err().code(),
        "stale_login_attempt"
    );

    let device_attempt = LoginAttemptId::new("attempt-two").unwrap();
    let mut device_interaction = CancellingInteraction {
        configuration: Arc::downgrade(&configuration),
        method: LoginMethod::DeviceCode,
        progress: Mutex::new(Vec::new()),
    };
    let device_error = configuration
        .login(
            device_attempt,
            ProviderId::new("openai-codex").unwrap(),
            &mut device_interaction,
        )
        .await
        .unwrap_err();
    assert_eq!(device_error.kind(), ErrorKind::LoginCancelled);
    assert!(device_interaction
        .progress
        .lock()
        .unwrap()
        .iter()
        .all(|progress| progress.stage != LoginStage::DisplayingDeviceCode));
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_login_admission_allows_exactly_one_attempt() {
    struct WaitingInteraction;

    impl LoginInteraction for WaitingInteraction {
        fn select_method(
            &mut self,
            _attempt: &LoginAttemptId,
            _provider: &ProviderId,
        ) -> InteractionFuture<'_, LoginMethod> {
            Box::pin(async { Ok(LoginMethod::Browser) })
        }

        fn open_url(
            &mut self,
            _attempt: &LoginAttemptId,
            _url: &str,
        ) -> Result<(), InteractionError> {
            Ok(())
        }

        fn display_device_code(
            &mut self,
            _attempt: &LoginAttemptId,
            _verification_uri: &str,
            _user_code: &str,
        ) -> Result<(), InteractionError> {
            unreachable!()
        }

        fn report(&mut self, _progress: LoginProgress) {}
    }

    let (_temporary, root) = configuration_root();
    let configuration = Configuration::new(root).unwrap();
    let first_attempt = LoginAttemptId::new("first-attempt").unwrap();
    let mut first_interaction = WaitingInteraction;
    let mut first = Box::pin(configuration.login(
        first_attempt.clone(),
        ProviderId::new("openai-codex").unwrap(),
        &mut first_interaction,
    ));
    std::future::poll_fn(|context| {
        assert!(matches!(first.as_mut().poll(context), Poll::Pending));
        Poll::Ready(())
    })
    .await;

    let mut second_interaction = WaitingInteraction;
    let error = configuration
        .login(
            LoginAttemptId::new("second-attempt").unwrap(),
            ProviderId::new("openai-codex").unwrap(),
            &mut second_interaction,
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::LoginInProgress);

    drop(first);
    assert_eq!(
        configuration
            .cancel_login(&first_attempt)
            .unwrap_err()
            .kind(),
        ErrorKind::StaleLoginAttempt
    );
}

#[test]
fn provider_neutral_reasoning_projection_spells_x_high() {
    let (_temporary, root) = configuration_root();
    let configuration = Configuration::new(root).unwrap();
    let status = configuration.status();
    let model = status
        .providers
        .iter()
        .flat_map(|provider| &provider.models)
        .find(|model| model.reasoning_levels.contains(&"x_high"))
        .expect("a bundled model supports x-high reasoning");

    assert!(!model.reasoning_levels.contains(&"xhigh"));
}
