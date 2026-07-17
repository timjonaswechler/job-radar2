use job_radar_lib::agent::configuration::{
    AgentConfiguration, AgentDataFolderOpener, AuthenticationKind, ConfigurationState,
    ExternalUrlOpener, OpenError, SecretApiKeyInput, SubscriptionLoginProgress,
    SubscriptionLoginProgressReporter, SubscriptionLoginStage,
};
use std::fs;
use std::future::Future;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::task::Poll;

fn agents_root(temp: &tempfile::TempDir) -> PathBuf {
    let root = temp.path().join("application-data").join("agents");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(root.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn write_private(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn status_projects_provider_and_model_configuration_without_secret_values() {
    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();

    let status = api.status();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();

    assert_eq!(codex.display_name, "OpenAI Codex");
    assert_eq!(
        codex.authentication_methods,
        vec![AuthenticationKind::Subscription]
    );
    assert_eq!(codex.active_authentication, None);
    assert!(!codex.available);
    assert_eq!(codex.models.len(), 7);
    assert_eq!(
        status.authentication_configuration,
        ConfigurationState::Ready
    );
    assert_eq!(status.model_configuration, ConfigurationState::Ready);

    let serialized = serde_json::to_string(&status).unwrap();
    assert!(!serialized.contains("access"));
    assert!(!serialized.contains("refresh"));
    assert!(!serialized.contains("apiKey"));
    assert!(!serialized.contains("account"));
}

#[test]
fn oauth_status_never_projects_stored_credential_or_account_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    write_private(
        &root.join("auth.json"),
        r#"{"openai-codex":{"type":"oauth","access":"synthetic-oauth-access","refresh":"synthetic-oauth-refresh","expires":9999999999999,"accountId":"synthetic-account-id"}}"#,
    );
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();

    let status = api.status();
    let codex = status
        .providers
        .iter()
        .find(|provider| provider.id == "openai-codex")
        .unwrap();
    assert_eq!(
        codex.active_authentication,
        Some(AuthenticationKind::Subscription)
    );
    assert!(codex.available);
    let serialized = serde_json::to_string(&status).unwrap();
    for secret in [
        "synthetic-oauth-access",
        "synthetic-oauth-refresh",
        "synthetic-account-id",
    ] {
        assert!(!serialized.contains(secret));
    }
}

#[test]
fn api_key_submission_and_removal_only_return_value_free_status() {
    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    write_private(
        &root.join("models.json"),
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://synthetic.invalid/v1","models":[{"id":"synthetic-model","name":"Synthetic"}]}}}"#,
    );
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();

    let status = api
        .submit_api_key(
            "synthetic-provider",
            SecretApiKeyInput::new("synthetic-secret-that-must-not-return"),
        )
        .unwrap();
    let provider = status
        .providers
        .iter()
        .find(|provider| provider.id == "synthetic-provider")
        .unwrap();
    assert_eq!(
        provider.active_authentication,
        Some(AuthenticationKind::ApiKey)
    );
    assert!(provider.available);
    assert!(!serde_json::to_string(&status)
        .unwrap()
        .contains("synthetic-secret-that-must-not-return"));

    let removed = api.remove_authentication("synthetic-provider").unwrap();
    let provider = removed
        .providers
        .iter()
        .find(|provider| provider.id == "synthetic-provider")
        .unwrap();
    assert_eq!(provider.active_authentication, None);
    assert!(!provider.available);
}

#[test]
fn reload_reports_redacted_file_states_and_preserves_last_known_good_models() {
    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    write_private(
        &root.join("models.json"),
        r#"{"providers":{"synthetic-provider":{"api":"openai-responses","baseUrl":"https://synthetic.invalid/v1","models":[{"id":"synthetic-model"}]}}}"#,
    );
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();
    assert!(api
        .status()
        .providers
        .iter()
        .any(|provider| provider.id == "synthetic-provider"));

    write_private(&root.join("models.json"), "not-json");
    write_private(&root.join("auth.json"), "not-json");
    let status = api.reload();

    assert_eq!(status.model_configuration, ConfigurationState::Invalid);
    assert_eq!(
        status.authentication_configuration,
        ConfigurationState::Invalid
    );
    assert!(status
        .providers
        .iter()
        .any(|provider| provider.id == "synthetic-provider"));
    let serialized = serde_json::to_string(&status).unwrap();
    assert!(!serialized.contains("not-json"));
    assert!(serialized.contains("agent model configuration is invalid"));
    assert!(serialized.contains("authentication storage is unavailable"));
}

#[derive(Default)]
struct RecordingFolderOpener(Mutex<Vec<PathBuf>>);

impl AgentDataFolderOpener for RecordingFolderOpener {
    fn open(&self, path: &Path) -> Result<(), OpenError> {
        self.0.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[test]
fn subscription_login_progress_is_value_free_and_can_be_cancelled() {
    struct CancellingReporter {
        api: Weak<AgentConfiguration>,
        progress: Mutex<Vec<SubscriptionLoginProgress>>,
    }

    impl SubscriptionLoginProgressReporter for CancellingReporter {
        fn report(&self, progress: SubscriptionLoginProgress) {
            let should_cancel = progress.stage == SubscriptionLoginStage::Starting;
            self.progress.lock().unwrap().push(progress);
            if should_cancel {
                assert!(self
                    .api
                    .upgrade()
                    .unwrap()
                    .cancel_subscription_login("openai-codex"));
            }
        }
    }

    struct RejectingOpener;
    impl ExternalUrlOpener for RejectingOpener {
        fn open(&self, _: &str) -> Result<(), OpenError> {
            panic!("a login cancelled at start must not open the browser")
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    let api = Arc::new(AgentConfiguration::from_agents_data_root(&root).unwrap());
    let reporter = CancellingReporter {
        api: Arc::downgrade(&api),
        progress: Mutex::new(Vec::new()),
    };

    let error = tauri::async_runtime::block_on(api.login_subscription(
        "openai-codex",
        &RejectingOpener,
        &reporter,
    ))
    .unwrap_err();

    assert_eq!(error.code, "subscription_login_cancelled");
    let progress = reporter.progress.lock().unwrap();
    assert_eq!(
        progress.first().unwrap().stage,
        SubscriptionLoginStage::Starting
    );
    assert_eq!(
        progress.last().unwrap().stage,
        SubscriptionLoginStage::Cancelled
    );
    let serialized = serde_json::to_string(&*progress).unwrap();
    assert!(!serialized.contains("url"));
    assert!(!serialized.contains("authorization"));
    assert!(!serialized.contains("state"));
}

#[test]
fn dropping_a_subscription_login_future_releases_the_provider_registration() {
    #[derive(Default)]
    struct RecordingReporter(Mutex<Vec<SubscriptionLoginProgress>>);
    impl SubscriptionLoginProgressReporter for RecordingReporter {
        fn report(&self, progress: SubscriptionLoginProgress) {
            self.0.lock().unwrap().push(progress);
        }
    }

    struct AcceptingOpener;
    impl ExternalUrlOpener for AcceptingOpener {
        fn open(&self, _: &str) -> Result<(), OpenError> {
            Ok(())
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();
    let reporter = RecordingReporter::default();

    tauri::async_runtime::block_on(async {
        let mut login =
            Box::pin(api.login_subscription("openai-codex", &AcceptingOpener, &reporter));
        std::future::poll_fn(|context| {
            assert!(matches!(login.as_mut().poll(context), Poll::Pending));
            Poll::Ready(())
        })
        .await;
        drop(login);
    });

    assert!(!api.cancel_subscription_login("openai-codex"));
}

#[test]
fn opening_agent_data_uses_the_injected_canonical_folder_adapter() {
    let temp = tempfile::tempdir().unwrap();
    let root = agents_root(&temp);
    let api = AgentConfiguration::from_agents_data_root(&root).unwrap();
    let opener = RecordingFolderOpener::default();

    api.open_data_folder(&opener).unwrap();

    assert_eq!(*opener.0.lock().unwrap(), vec![root]);
}
