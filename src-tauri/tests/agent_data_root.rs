#![cfg(unix)]

use job_radar_lib::agent::openai_codex::{AgentAuthentication, AuthStatus};
use job_radar_lib::agent::AgentErrorCategory;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

#[test]
fn injectable_agents_data_root_migrates_legacy_auth_with_private_permissions() {
    let app_data = tempfile::tempdir().unwrap();
    let legacy_dir = app_data.path().join("agent");
    fs::create_dir(&legacy_dir).unwrap();
    fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let legacy_auth = legacy_dir.join("auth.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&legacy_auth)
        .unwrap();
    file.write_all(
        br#"{
  "openai-codex": {
    "type": "oauth",
    "access": "synthetic-access",
    "refresh": "synthetic-refresh",
    "expires": 4102444800000,
    "accountId": "synthetic-account"
  }
}
"#,
    )
    .unwrap();
    file.sync_all().unwrap();

    let agents_data_root = app_data.path().join("agents");
    let authentication = AgentAuthentication::from_agents_data_root(&agents_data_root).unwrap();

    assert_eq!(authentication.status().unwrap(), AuthStatus::Configured);
    assert!(!legacy_auth.exists());
    assert_eq!(
        agents_data_root.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file_name in ["auth.json", "auth.lock"] {
        assert_eq!(
            agents_data_root
                .join(file_name)
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn canonical_and_legacy_auth_report_a_redacted_conflict_without_merging() {
    let app_data = tempfile::tempdir().unwrap();
    let agents_data_root = app_data.path().join("agents");
    AgentAuthentication::from_agents_data_root(&agents_data_root).unwrap();
    let legacy_dir = app_data.path().join("agent");
    fs::create_dir(&legacy_dir).unwrap();
    fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let legacy_auth = legacy_dir.join("auth.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&legacy_auth)
        .unwrap();
    file.write_all(b"{}\n").unwrap();
    file.sync_all().unwrap();

    let error = match AgentAuthentication::from_agents_data_root(&agents_data_root) {
        Ok(_) => panic!("conflicting authentication documents unexpectedly merged"),
        Err(error) => error,
    };

    assert_eq!(error.category, AgentErrorCategory::InvalidConfiguration);
    assert_eq!(
        error.message,
        "conflicting authentication storage locations require review"
    );
    assert!(!error
        .message
        .contains(app_data.path().to_string_lossy().as_ref()));
    assert!(legacy_auth.exists());
    assert!(agents_data_root.join("auth.json").exists());
}

#[test]
fn injectable_root_rejects_a_symlinked_trusted_ancestor() {
    let sandbox = tempfile::tempdir().unwrap();
    let real_ancestor = sandbox.path().join("real-ancestor");
    fs::create_dir(&real_ancestor).unwrap();
    let linked_ancestor = sandbox.path().join("linked-ancestor");
    std::os::unix::fs::symlink(&real_ancestor, &linked_ancestor).unwrap();
    let agents_data_root = linked_ancestor.join("app-data/agents");

    assert!(AgentAuthentication::from_agents_data_root(&agents_data_root).is_err());
    assert!(!real_ancestor.join("app-data/agents").exists());
}

#[test]
fn injectable_root_must_end_in_agents() {
    let app_data = tempfile::tempdir().unwrap();
    let invalid_root = app_data.path().join("credentials");

    assert!(AgentAuthentication::from_agents_data_root(invalid_root).is_err());
    assert!(!app_data.path().join("credentials").exists());
}
