use std::{fs, path::Path};

use job_radar_lib::{
    load_source_profile_registry_snapshot, DiagnosticCategory, DiagnosticSeverity,
};

#[test]
fn registry_loads_new_dsl_builtin_profiles_and_ignores_custom_builtin_key_collision() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_profile_dir = temp_dir.path().join("source-profiles");
    fs::create_dir_all(&custom_profile_dir).unwrap();
    fs::copy(
        fixture_path("resources/profiles/greenhouse.json"),
        custom_profile_dir.join("greenhouse.json"),
    )
    .unwrap();

    let snapshot = load_source_profile_registry_snapshot(temp_dir.path());

    let greenhouse_profiles = snapshot
        .profiles
        .iter()
        .filter(|profile| profile.document.key == "greenhouse")
        .collect::<Vec<_>>();
    assert_eq!(greenhouse_profiles.len(), 1);
    assert_eq!(greenhouse_profiles[0].origin, "built_in");

    let serialized = serde_json::to_string(&greenhouse_profiles[0].document).unwrap();
    assert!(!serialized.contains("adapterKey"));
    assert!(!serialized.contains("inventory"));

    let diagnostic = snapshot
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "duplicate_source_profile_key")
        .expect("custom Source Profile key collision should emit a Structured Diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Registry);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.path, "/key");
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["sourceProfileKey"],
        "greenhouse"
    );
}

#[test]
fn registry_loads_custom_sources_with_derived_validation_state_and_compiler_diagnostics() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_profile_dir = temp_dir.path().join("source-profiles");
    let custom_source_dir = temp_dir.path().join("sources");
    fs::create_dir_all(&custom_profile_dir).unwrap();
    fs::create_dir_all(&custom_source_dir).unwrap();
    fs::copy(
        fixture_path("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json"),
        custom_profile_dir.join("example_jobs.json"),
    )
    .unwrap();
    fs::write(
        custom_source_dir.join("invalid_source.json"),
        r#"{
          "schemaVersion": 2,
          "key": "invalid_source",
          "name": "Invalid Source",
          "status": "active",
          "sourceConfig": {
            "feedUrl": 42,
            "keyword": "rust"
          },
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
          }
        }
        "#,
    )
    .unwrap();

    let snapshot = load_source_profile_registry_snapshot(temp_dir.path());

    let source = snapshot.source("invalid_source").unwrap();
    assert_eq!(
        source.validation_state.state,
        job_radar_lib::ValidationStateKind::Invalid
    );
    assert!(!source.validation_state.can_compile);
    assert!(!source.validation_state.can_execute);
    assert!(source
        .validation_state
        .diagnostics
        .iter()
        .any(
            |diagnostic| diagnostic.code == "invalid_source_config_property_type"
                && diagnostic.path == "/sourceConfig/feedUrl"
        ));
    assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "forbidden_search_criteria_in_source_config"
        && diagnostic.path == "/sourceConfig/keyword"));
}

#[test]
fn registry_exposes_schema_and_profile_compiler_failures_as_structured_diagnostics() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_profile_dir = temp_dir.path().join("source-profiles");
    let custom_source_dir = temp_dir.path().join("sources");
    fs::create_dir_all(&custom_profile_dir).unwrap();
    fs::create_dir_all(&custom_source_dir).unwrap();
    fs::copy(
        fixture_path("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json"),
        custom_profile_dir.join("example_jobs.json"),
    )
    .unwrap();
    fs::write(
        custom_source_dir.join("persisted_invalid_status.json"),
        r#"{
          "schemaVersion": 2,
          "key": "persisted_invalid_status",
          "name": "Persisted Invalid Status",
          "status": "invalid",
          "sourceConfig": {},
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
          }
        }
        "#,
    )
    .unwrap();
    fs::write(
        custom_source_dir.join("missing_profile.json"),
        r#"{
          "schemaVersion": 2,
          "key": "missing_profile",
          "name": "Missing Profile",
          "status": "active",
          "sourceConfig": {},
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "missing_profile_key",
            "pathKey": "json_feed"
          }
        }
        "#,
    )
    .unwrap();
    fs::write(
        custom_source_dir.join("missing_access_path.json"),
        r#"{
          "schemaVersion": 2,
          "key": "missing_access_path",
          "name": "Missing Access Path",
          "status": "active",
          "sourceConfig": { "feedUrl": "https://example.test/jobs.json" },
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "missing_path"
          }
        }
        "#,
    )
    .unwrap();
    fs::write(
        custom_source_dir.join("invalid_overrides.json"),
        r#"{
          "schemaVersion": 2,
          "key": "invalid_overrides",
          "name": "Invalid Overrides",
          "status": "active",
          "sourceConfig": {
            "feedUrl": "https://example.test/jobs.json",
            "language": "en"
          },
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
          },
          "sourceOverrides": {
            "strategyOverrides": [
              {
                "step": "postingDiscovery",
                "strategyKey": "missing_strategy",
                "acceptWhen": { "minResults": 0 }
              }
            ]
          }
        }
        "#,
    )
    .unwrap();

    let snapshot = load_source_profile_registry_snapshot(temp_dir.path());

    assert!(snapshot.source("persisted_invalid_status").is_none());
    assert_diagnostic(
        &snapshot.diagnostics,
        DiagnosticCategory::Schema,
        "invalid_document_shape",
    );
    assert_diagnostic(
        &snapshot.diagnostics,
        DiagnosticCategory::Compiler,
        "source_profile_not_found",
    );
    assert_diagnostic(
        &snapshot.diagnostics,
        DiagnosticCategory::Compiler,
        "access_path_not_found",
    );
    assert_diagnostic(
        &snapshot.diagnostics,
        DiagnosticCategory::Compiler,
        "unknown_strategy_override",
    );
}

fn assert_diagnostic(
    diagnostics: &[job_radar_lib::Diagnostic],
    category: DiagnosticCategory,
    code: &str,
) {
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.category == category
                && diagnostic.code == code
                && !diagnostic.message.is_empty()
                && matches!(diagnostic.severity, DiagnosticSeverity::Error)
                && diagnostic.strategy_key.is_none()
                && diagnostic.details.is_some()),
        "expected diagnostic {category:?}/{code}, got {diagnostics:#?}"
    );
}

fn fixture_path(relative_path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}
