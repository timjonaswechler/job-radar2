use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use job_radar_lib::{
    compile_source_execution_plan, load_source_profile_registry_snapshot, DiagnosticCategory,
    DiagnosticSeverity, ProfileCompilerSnapshot, SourceDocument, SourceProfileDocument,
};

#[test]
fn resource_directory_matches_embedded_new_dsl_builtins_and_contains_no_v1_documents() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot = load_source_profile_registry_snapshot(tempfile::tempdir().unwrap().path());

    let profile_paths = json_files(&crate_dir.join("resources/profiles"));
    let resource_profile_keys = file_stems(&profile_paths);
    let embedded_profile_keys = snapshot
        .profiles
        .iter()
        .filter(|profile| profile.origin == "built_in")
        .map(|profile| profile.document.key.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(resource_profile_keys, embedded_profile_keys);

    for path in profile_paths {
        let text = fs::read_to_string(&path).unwrap();
        assert_no_v1_resource_vocabulary(&path, &text);
        let document: SourceProfileDocument = serde_json::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "{} should be a Source Profile DSL document: {error}",
                path.display()
            )
        });
        assert_eq!(document.schema_version, 2, "{}", path.display());
        assert!(!document.access_paths.is_empty(), "{}", path.display());
    }

    let source_paths = json_files(&crate_dir.join("resources/sources"));
    let resource_source_keys = file_stems(&source_paths);
    let embedded_source_keys = snapshot
        .sources
        .iter()
        .filter(|source| source.origin == "built_in")
        .map(|source| source.document.key.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(resource_source_keys, embedded_source_keys);

    for path in source_paths {
        let text = fs::read_to_string(&path).unwrap();
        assert_no_v1_resource_vocabulary(&path, &text);
        let document: SourceDocument = serde_json::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "{} should be a Source DSL document: {error}",
                path.display()
            )
        });
        assert_eq!(document.schema_version, 2, "{}", path.display());
    }
}

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
fn backend_v1_adapter_registry_and_runtime_entrypoints_are_removed() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!crate_dir.join("src/adapter_registry.rs").exists());
    assert!(!crate_dir.join("src/declarative").exists());
    assert!(!crate_dir.join("src/source/registry").exists());
    assert!(!crate_dir.join("src/source/detection").exists());

    let snapshot = load_source_profile_registry_snapshot(tempfile::tempdir().unwrap().path());
    let compiler_snapshot = ProfileCompilerSnapshot {
        profiles: snapshot
            .profiles
            .iter()
            .map(|profile| profile.document.clone())
            .collect(),
        sources: vec![serde_json::from_str(
            r#"{
          "schemaVersion": 2,
          "key": "greenhouse_fixture",
          "name": "Greenhouse Fixture",
          "status": "active",
          "sourceConfig": {
            "boardSlug": "example"
          },
          "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "greenhouse",
            "pathKey": "boards_api"
          }
        }"#,
        )
        .unwrap()],
    };
    let result = compile_source_execution_plan(&compiler_snapshot, "greenhouse_fixture");
    let plan = result
        .execution_plan
        .expect("new DSL Source should compile into an Execution Plan");
    let serialized_plan = serde_json::to_string(&plan).unwrap();
    assert!(!serialized_plan.contains("adapterKey"));
    assert!(!serialized_plan.contains("list_adapters"));
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
fn registry_compiler_validates_unreferenced_custom_profiles() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_profile_dir = temp_dir.path().join("source-profiles");
    fs::create_dir_all(&custom_profile_dir).unwrap();

    let mut profile: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture_path(
            "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
        ))
        .unwrap(),
    )
    .unwrap();
    profile["accessPaths"][0]["postingDiscovery"]["strategies"][0]["fetch"]
        .as_object_mut()
        .unwrap()
        .remove("timeoutMs");
    fs::write(
        custom_profile_dir.join("example_jobs.json"),
        serde_json::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();

    let snapshot = load_source_profile_registry_snapshot(temp_dir.path());

    assert!(snapshot.profile("example_jobs").is_some());
    assert!(snapshot.source("example_jobs").is_none());
    assert!(snapshot.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Compiler
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "missing_fetch_timeout"
            && diagnostic.path == "/accessPaths/0/postingDiscovery/strategies/0/fetch/timeoutMs"
            && diagnostic.strategy_key.as_deref() == Some("json_api")
    }));
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

fn json_files(directory: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn file_stems(paths: &[PathBuf]) -> BTreeSet<String> {
    paths
        .iter()
        .map(|path| path.file_stem().unwrap().to_string_lossy().to_string())
        .collect()
}

fn assert_no_v1_resource_vocabulary(path: &Path, text: &str) {
    for forbidden in [
        "adapterKey",
        "adapter_key",
        "declarative_endpoint_inventory",
        "declarative_sitemap_inventory",
        "declarative_browser_inventory",
        "inventory",
        "SourceSpecific",
        "source_specific",
        "\"status\": \"invalid\"",
    ] {
        assert!(
            !text.contains(forbidden),
            "{} contains removed v1 resource vocabulary `{forbidden}`",
            path.display()
        );
    }
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
