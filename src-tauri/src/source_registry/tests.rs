use super::*;
use serde_json::json;
use std::path::Path;

#[test]
fn loads_migrated_builtin_source_registry_documents() {
    let temp_dir = tempfile::tempdir().unwrap();

    let snapshot = load_snapshot(temp_dir.path());

    assert!(
        snapshot.diagnostics.is_empty(),
        "built-in registry diagnostics: {:#?}",
        snapshot.diagnostics
    );
    assert_eq!(
        sorted_profile_keys(&snapshot),
        vec![
            "ashby",
            "greenhouse",
            "lever",
            "magnolia_esmp_job_search",
            "muz_global_jobboard",
            "personio",
            "phenom",
            "stepstone_de",
            "successfactors",
            "workday",
        ]
    );
    assert_eq!(sorted_source_keys(&snapshot), vec!["stepstone_de"]);

    let stepstone = snapshot.source("stepstone_de").unwrap();
    assert_eq!(stepstone.origin, SourceRegistryDocumentOrigin::BuiltIn);
    assert_eq!(stepstone.document.status, SourceDocumentStatus::Active);
    assert!(matches!(
        &stepstone.document.selected_access_path,
        SelectedAccessPath::Profile { profile_key, path_key }
            if profile_key == "stepstone_de" && path_key == "browser_inventory"
    ));

    let stepstone_profile = snapshot.profile("stepstone_de").unwrap();
    assert_eq!(
        stepstone_profile.document.kind,
        SourceProfileKind::JobPortal
    );
    assert!(stepstone_profile
        .document
        .access_paths
        .iter()
        .any(|path| path.key == "browser_inventory"
            && path.adapter_key == "declarative_browser_inventory"));

    let greenhouse = snapshot.profile("greenhouse").unwrap();
    assert!(greenhouse
        .document
        .access_paths
        .iter()
        .any(|path| path.key == "endpoint_inventory"
            && path.adapter_key == "declarative_endpoint_inventory"));
}

#[test]
fn muz_global_jobboard_builtin_schema_and_inventory_are_hardened() {
    let temp_dir = tempfile::tempdir().unwrap();
    let snapshot = load_snapshot(temp_dir.path());
    let profile = snapshot.profile("muz_global_jobboard").unwrap();
    let access_path = profile
        .document
        .access_paths
        .iter()
        .find(|access_path| access_path.key == "endpoint_inventory")
        .unwrap();

    let source_config_schema = access_path.source_config_schema.as_ref().unwrap();
    assert_eq!(source_config_schema["additionalProperties"], false);
    assert_eq!(
        source_config_schema["properties"]["apiBaseUrl"]["pattern"],
        "^https?://.+/$"
    );
    assert_eq!(
        source_config_schema["properties"]["configUrl"]["pattern"],
        "^https?://.+/assets/js/jobboard\\.config\\.json$"
    );
    assert_eq!(
        access_path.inventory.as_ref().unwrap()["fields"]["locations"],
        json!([{
            "jsonPath": "$.MatchedObjectDescriptor.PositionLocation",
            "objectFields": ["CityName", "CountryName"]
        }])
    );
}

#[test]
fn personio_builtin_schema_and_inventory_are_hardened() {
    let temp_dir = tempfile::tempdir().unwrap();
    let snapshot = load_snapshot(temp_dir.path());
    let profile = snapshot.profile("personio").unwrap();
    let access_path = profile
        .document
        .access_paths
        .iter()
        .find(|access_path| access_path.key == "endpoint_inventory")
        .unwrap();

    let source_config_schema = access_path.source_config_schema.as_ref().unwrap();
    assert_eq!(source_config_schema["additionalProperties"], false);
    assert_eq!(
        source_config_schema["required"],
        json!(["boardSlug", "personioHost", "language"])
    );
    assert_eq!(
        source_config_schema["properties"]["language"]["enum"],
        json!(["de", "en", "fr", "es", "nl", "it", "pt"])
    );
    assert_eq!(
        access_path.inventory.as_ref().unwrap()["items"]["select"],
        json!({ "xmlElement": "position" })
    );
    assert_eq!(
        access_path.inventory.as_ref().unwrap()["fields"]["url"],
        json!({ "template": "https://{{sourceConfig:personioHost}}/job/{{itemJson:$.id}}" })
    );
}

#[test]
fn loads_valid_profile_backed_and_source_specific_documents() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("source-profiles/greenhouse.json"),
        &profile_json("greenhouse", &["boards_api"]),
    );
    write_json(
        temp_dir.path().join("sources/helsing.json"),
        &profile_source_json("helsing", "greenhouse", "boards_api"),
    );
    write_json(
        temp_dir.path().join("sources/example_company.json"),
        &source_specific_source_json("example_company"),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    assert!(snapshot.diagnostics.is_empty());
    assert_eq!(snapshot.valid_profiles.len(), 1);
    assert_eq!(snapshot.valid_profiles[0].document.key, "greenhouse");
    assert_eq!(
        snapshot.valid_profiles[0].document.kind,
        SourceProfileKind::RecruitingSystem
    );
    assert_eq!(
        snapshot.valid_profiles[0].document.access_paths[0].key,
        "boards_api"
    );
    assert_eq!(snapshot.valid_sources.len(), 2);
    assert_eq!(snapshot.valid_sources[0].document.key, "example_company");
    assert!(matches!(
        snapshot.valid_sources[0].document.selected_access_path,
        SelectedAccessPath::SourceSpecific { .. }
    ));
    assert_eq!(snapshot.valid_sources[1].document.key, "helsing");
    assert!(matches!(
        snapshot.valid_sources[1].document.selected_access_path,
        SelectedAccessPath::Profile { .. }
    ));
}

#[test]
fn reports_invalid_json_invalid_shape_and_does_not_create_missing_directories() {
    let temp_dir = tempfile::tempdir().unwrap();
    let app_data_dir = temp_dir.path().join("app-data");
    let invalid_json_path = app_data_dir.join("source-profiles/broken.json");
    let missing_kind_path = app_data_dir.join("source-profiles/missing_kind.json");
    write_raw(&invalid_json_path, "{not json");
    write_json(
        &missing_kind_path,
        &json!({
            "schemaVersion": 1,
            "key": "missing_kind",
            "name": "Missing Kind",
            "accessPaths": [{ "key": "api", "adapterKey": "declarative_endpoint_inventory" }]
        })
        .to_string(),
    );

    let snapshot = load_custom_only_snapshot(&app_data_dir);

    assert_eq!(snapshot.valid_profiles.len(), 0);
    assert_diagnostic_codes(
        &snapshot,
        &[
            SourceRegistryDiagnosticCode::InvalidJson,
            SourceRegistryDiagnosticCode::InvalidShape,
        ],
    );
    assert_eq!(
        std::fs::read_to_string(&invalid_json_path).unwrap(),
        "{not json"
    );

    let missing_app_data_dir = temp_dir.path().join("does-not-exist");
    let empty_snapshot = load_custom_only_snapshot(&missing_app_data_dir);
    assert!(empty_snapshot.diagnostics.is_empty());
    assert!(!missing_app_data_dir.exists());
}

#[test]
fn reports_filename_key_mismatch_and_builtin_duplicate_keys() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("source-profiles/greenhouse.json"),
        &profile_json("greenhouse", &["custom_api"]),
    );
    write_json(
        temp_dir.path().join("source-profiles/wrong_name.json"),
        &profile_json("right_name", &["api"]),
    );
    write_json(
        temp_dir.path().join("sources/stepstone_de.json"),
        &source_specific_source_json("stepstone_de"),
    );

    let snapshot = load_snapshot_with_builtins(
        temp_dir.path(),
        &[(
            "source-profiles/builtin/greenhouse.json",
            &profile_json("greenhouse", &["boards_api"]),
        )],
        &[(
            "sources/builtin/stepstone_de.json",
            &source_specific_source_json("stepstone_de"),
        )],
    );

    assert_eq!(snapshot.valid_profiles.len(), 1);
    assert_eq!(
        snapshot.valid_profiles[0].origin,
        SourceRegistryDocumentOrigin::BuiltIn
    );
    assert_eq!(
        snapshot.valid_profiles[0].document.access_paths[0].key,
        "boards_api"
    );
    assert_eq!(snapshot.valid_sources.len(), 1);
    assert_eq!(
        snapshot.valid_sources[0].origin,
        SourceRegistryDocumentOrigin::BuiltIn
    );
    assert_diagnostic_codes(
        &snapshot,
        &[
            SourceRegistryDiagnosticCode::DuplicateKey,
            SourceRegistryDiagnosticCode::FilenameKeyMismatch,
            SourceRegistryDiagnosticCode::DuplicateKey,
        ],
    );
    assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic.code
        == SourceRegistryDiagnosticCode::DuplicateKey
        && diagnostic.document_kind == SourceRegistryDocumentKind::SourceProfile
        && diagnostic.key.as_deref() == Some("greenhouse")));
    assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic.code
        == SourceRegistryDiagnosticCode::DuplicateKey
        && diagnostic.document_kind == SourceRegistryDocumentKind::Source
        && diagnostic.key.as_deref() == Some("stepstone_de")));
}

#[test]
fn reports_missing_profile_and_missing_path_references() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("source-profiles/greenhouse.json"),
        &profile_json("greenhouse", &["boards_api"]),
    );
    write_json(
        temp_dir.path().join("sources/missing_profile_source.json"),
        &profile_source_json("missing_profile_source", "unknown_profile", "boards_api"),
    );
    write_json(
        temp_dir.path().join("sources/missing_path_source.json"),
        &profile_source_json("missing_path_source", "greenhouse", "unknown_path"),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    assert_eq!(snapshot.valid_sources.len(), 0);
    assert_diagnostic_codes(
        &snapshot,
        &[
            SourceRegistryDiagnosticCode::MissingPathRef,
            SourceRegistryDiagnosticCode::MissingProfileRef,
        ],
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.key.as_deref() == Some("missing_profile_source")));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.key.as_deref() == Some("missing_path_source")));
}

#[test]
fn reports_invalid_selected_access_path_variants_and_source_specific_availability() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
            temp_dir.path().join("sources/invalid_variant.json"),
            &json!({
                "schemaVersion": 1,
                "key": "invalid_variant",
                "name": "Invalid Variant",
                "status": "draft",
                "sourceConfig": {},
                "selectedAccessPath": { "type": "browser", "adapterKey": "declarative_browser_inventory" }
            })
            .to_string(),
        );
    write_json(
        temp_dir
            .path()
            .join("sources/source_specific_with_availability.json"),
        &json!({
            "schemaVersion": 1,
            "key": "source_specific_with_availability",
            "name": "Source Specific With Availability",
            "status": "draft",
            "sourceConfig": {},
            "selectedAccessPath": {
                "type": "source_specific",
                "adapterKey": "declarative_browser_inventory",
                "availability": { "requiredCaptures": [] }
            }
        })
        .to_string(),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    assert!(snapshot.valid_sources.is_empty());
    assert_diagnostic_codes(
        &snapshot,
        &[
            SourceRegistryDiagnosticCode::InvalidShape,
            SourceRegistryDiagnosticCode::InvalidShape,
        ],
    );
    assert!(snapshot.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("must be profile or source_specific")));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("availability is not allowed")));
}

#[test]
fn reports_profile_access_paths_with_duplicate_keys_as_invalid_shape() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("source-profiles/greenhouse.json"),
        &profile_json("greenhouse", &["boards_api", "boards_api"]),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    assert!(snapshot.valid_profiles.is_empty());
    assert_diagnostic_codes(&snapshot, &[SourceRegistryDiagnosticCode::InvalidShape]);
    assert!(snapshot.diagnostics[0]
        .message
        .contains("accessPaths contains duplicate key `boards_api`"));
}

#[test]
fn resolves_profile_backed_execution_plan_with_access_path_definition() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("source-profiles/example_profile.json"),
        &profile_with_execution_plan_json(),
    );
    write_json(
        temp_dir.path().join("sources/example_source.json"),
        &json!({
            "schemaVersion": 1,
            "key": "example_source",
            "name": "Example Source",
            "status": "active",
            "sourceConfig": {
                "tenant": "acme",
                "startUrl": "https://example.test/jobs"
            },
            "selectedAccessPath": {
                "type": "profile",
                "profileKey": "example_profile",
                "pathKey": "endpoint_inventory"
            }
        })
        .to_string(),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    let plan = snapshot.resolve_source("example_source").unwrap();
    assert_eq!(plan.key, "example_source");
    assert_eq!(plan.name, "Example Source");
    assert_eq!(plan.adapter_key, "declarative_endpoint_inventory");
    assert_eq!(
        plan.source_config,
        json!({
            "tenant": "acme",
            "startUrl": "https://example.test/jobs"
        })
    );
    assert_eq!(
        plan.effective_source_config_schema,
        json!({
            "allOf": [
                {
                    "type": "object",
                    "required": ["tenant"],
                    "properties": {
                        "tenant": { "type": "string" }
                    }
                },
                {
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                        "startUrl": { "type": "string", "format": "uri" }
                    }
                }
            ]
        })
    );
    assert_eq!(
        plan.inventory(),
        Some(&json!({
            "fetch": { "url": "{{sourceConfig:startUrl}}" },
            "parse": { "as": "json" },
            "items": { "select": { "jsonPath": "$.jobs" } },
            "fields": {
                "title": { "jsonPath": "$.title" },
                "url": { "jsonPath": "$.url" },
                "company": { "template": "{{sourceName}}" },
                "locations": []
            }
        }))
    );
    assert_eq!(
        plan.query(),
        Some(&json!({
            "baseUrl": "{{sourceConfig:startUrl}}",
            "path": "/jobs",
            "params": []
        }))
    );
    assert!(matches!(
        &plan.selected_access_path,
        ResolvedSelectedAccessPath::Profile { profile_key, path_key, .. }
            if profile_key == "example_profile" && path_key == "endpoint_inventory"
    ));
}

#[test]
fn resolves_source_specific_execution_plan_from_inline_selected_access_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_json(
        temp_dir.path().join("sources/example_company.json"),
        &json!({
            "schemaVersion": 1,
            "key": "example_company",
            "name": "Example Company",
            "status": "active",
            "sourceConfig": { "startUrl": "https://example.test/jobs" },
            "selectedAccessPath": {
                "type": "source_specific",
                "adapterKey": "declarative_browser_inventory",
                "sourceConfigSchema": {
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                        "startUrl": { "type": "string", "format": "uri" }
                    }
                },
                "query": {
                    "baseUrl": "{{sourceConfig:startUrl}}",
                    "path": "/jobs",
                    "params": []
                },
                "interactions": [
                    { "type": "waitFor", "selector": ".job-card", "timeoutMs": 5000 }
                ],
                "inventory": {
                    "items": { "select": ".job-card" },
                    "fields": {
                        "title": { "selectorText": ".title" },
                        "company": { "selectorText": ".company" },
                        "url": {
                            "selectorAttribute": { "selector": "a", "attribute": "href" }
                        },
                        "locations": []
                    }
                }
            }
        })
        .to_string(),
    );

    let snapshot = load_custom_only_snapshot(temp_dir.path());

    let plan = snapshot.resolve_source("example_company").unwrap();
    assert_eq!(plan.key, "example_company");
    assert_eq!(plan.adapter_key, "declarative_browser_inventory");
    assert_eq!(
        plan.effective_source_config_schema,
        json!({
            "type": "object",
            "required": ["startUrl"],
            "properties": {
                "startUrl": { "type": "string", "format": "uri" }
            }
        })
    );
    assert_eq!(
        plan.query(),
        Some(&json!({
            "baseUrl": "{{sourceConfig:startUrl}}",
            "path": "/jobs",
            "params": []
        }))
    );
    assert_eq!(
        plan.inventory(),
        Some(&json!({
            "items": { "select": ".job-card" },
            "fields": {
                "title": { "selectorText": ".title" },
                "company": { "selectorText": ".company" },
                "url": {
                    "selectorAttribute": { "selector": "a", "attribute": "href" }
                },
                "locations": []
            }
        }))
    );
    assert!(matches!(
        &plan.selected_access_path,
        ResolvedSelectedAccessPath::SourceSpecific { interactions, .. }
            if interactions.as_ref().is_some_and(|interactions| interactions.len() == 1)
    ));
}

fn load_custom_only_snapshot(app_data_dir: impl AsRef<Path>) -> SourceRegistrySnapshot {
    load_snapshot_with_builtins(app_data_dir, &[], &[])
}

fn sorted_profile_keys(snapshot: &SourceRegistrySnapshot) -> Vec<&str> {
    let mut keys = snapshot
        .valid_profiles
        .iter()
        .map(|profile| profile.document.key.as_str())
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn sorted_source_keys(snapshot: &SourceRegistrySnapshot) -> Vec<&str> {
    let mut keys = snapshot
        .valid_sources
        .iter()
        .map(|source| source.document.key.as_str())
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn profile_json(key: &str, access_path_keys: &[&str]) -> String {
    json!({
        "schemaVersion": 1,
        "key": key,
        "name": title_from_key(key),
        "kind": "recruiting_system",
        "detect": { "phases": ["http"], "required": [] },
        "identity": {
            "keyCandidates": ["{{capture:boardSlug|technicalKey}}"],
            "nameCandidates": ["{{capture:boardSlug|titleCase}}"]
        },
        "sourceConfigSchema": { "type": "object" },
        "accessPaths": access_path_keys.iter().map(|path_key| json!({
            "key": path_key,
            "adapterKey": "declarative_endpoint_inventory",
            "availability": {
                "requiredCaptures": ["boardSlug"],
                "checks": [],
                "sourceConfig": { "boardSlug": "{{capture:boardSlug}}" }
            },
            "sourceConfigSchema": { "type": "object" },
            "inventory": {}
        })).collect::<Vec<_>>()
    })
    .to_string()
}

fn profile_source_json(key: &str, profile_key: &str, path_key: &str) -> String {
    json!({
        "schemaVersion": 1,
        "key": key,
        "name": title_from_key(key),
        "status": "draft",
        "sourceConfig": { "boardSlug": key },
        "selectedAccessPath": {
            "type": "profile",
            "profileKey": profile_key,
            "pathKey": path_key
        }
    })
    .to_string()
}

fn profile_with_execution_plan_json() -> String {
    json!({
        "schemaVersion": 1,
        "key": "example_profile",
        "name": "Example Profile",
        "kind": "recruiting_system",
        "sourceConfigSchema": {
            "type": "object",
            "required": ["tenant"],
            "properties": {
                "tenant": { "type": "string" }
            }
        },
        "accessPaths": [
            {
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "sourceConfigSchema": {
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                        "startUrl": { "type": "string", "format": "uri" }
                    }
                },
                "query": {
                    "baseUrl": "{{sourceConfig:startUrl}}",
                    "path": "/jobs",
                    "params": []
                },
                "inventory": {
                    "fetch": { "url": "{{sourceConfig:startUrl}}" },
                    "parse": { "as": "json" },
                    "items": { "select": { "jsonPath": "$.jobs" } },
                    "fields": {
                        "title": { "jsonPath": "$.title" },
                        "url": { "jsonPath": "$.url" },
                        "company": { "template": "{{sourceName}}" },
                        "locations": []
                    }
                }
            }
        ]
    })
    .to_string()
}

fn source_specific_source_json(key: &str) -> String {
    json!({
        "schemaVersion": 1,
        "key": key,
        "name": title_from_key(key),
        "status": "draft",
        "sourceConfig": { "startUrl": "https://example.com/jobs" },
        "selectedAccessPath": {
            "type": "source_specific",
            "adapterKey": "declarative_browser_inventory",
            "sourceConfigSchema": { "type": "object" },
            "interactions": [
                { "type": "waitFor", "selector": ".job-card", "timeoutMs": 1000 }
            ],
            "inventory": {}
        }
    })
    .to_string()
}

fn title_from_key(key: &str) -> String {
    key.split('_')
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_json(path: impl AsRef<Path>, contents: &str) {
    write_raw(path, contents);
}

fn write_raw(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn assert_diagnostic_codes(
    snapshot: &SourceRegistrySnapshot,
    expected_codes: &[SourceRegistryDiagnosticCode],
) {
    let codes = snapshot
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert_eq!(codes, expected_codes);
}
