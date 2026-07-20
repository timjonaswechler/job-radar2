use std::{fs, path::Path};

use serde_json::{json, Value};

use super::{AccessPathFragment, SupportLevel};
use crate::source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
use crate::source_profile::documents::{SourceProfileDocument, SourceProfileKind};

#[test]
fn simple_reusable_source_profile_fixture_deserializes() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");

    assert_eq!(profile.schema_version, 3);
    assert_eq!(profile.key, "example_jobs");
    assert_eq!(profile.name, "Example Jobs");
    assert_eq!(profile.kind, SourceProfileKind::Generic);
    assert_eq!(profile.access_paths.len(), 1);
    assert_eq!(profile.access_paths[0].key, "json_feed");
    assert_eq!(
        profile.access_paths[0].discovery.strategies[0].key,
        "json_api"
    );
}

#[test]
fn source_selecting_reusable_access_path_fixture_deserializes() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    assert_eq!(source.schema_version, 3);
    assert_eq!(source.key, "example_source");
    assert_eq!(source.status, SourceStatus::Active);
    assert_eq!(
        source.source_config["feedUrl"],
        "https://example.test/jobs.json"
    );

    let SelectedAccessPath::ProfileAccessPath {
        profile_key,
        path_key,
    } = source.selected_access_path
    else {
        panic!("expected source to select a reusable profile access path");
    };

    assert_eq!(profile_key, "example_jobs");
    assert_eq!(path_key, "json_feed");
}

#[test]
fn source_owned_access_path_fixture_deserializes() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");

    assert_eq!(source.key, "owned_source");
    assert_eq!(source.status, SourceStatus::Draft);
    assert_eq!(
        source.source_config["startUrl"],
        "https://example.test/careers"
    );

    let SelectedAccessPath::SourceOwnedAccessPath {
        key,
        name,
        discovery,
        ..
    } = source.selected_access_path
    else {
        panic!("expected source-owned access path");
    };

    assert_eq!(key, "html_page");
    assert_eq!(name, "HTML page");
    assert_eq!(discovery.strategies[0].key, "html_cards");
}

#[test]
fn direct_profile_fragments_are_typed_and_persisted_in_source_json() {
    let fragments: Vec<AccessPathFragment> = serde_json::from_value(json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "fetch": { "timeoutMs": 5000 },
                "acceptWhen": { "requiredFields": ["url"] }
            }]
        }
    }]))
    .expect("the final fragment vocabulary should deserialize independently");
    assert_eq!(fragments[0].key, "json_feed");

    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments);
    let serialized = serde_json::to_value(&source).unwrap();
    assert_eq!(serialized["accessPaths"][0]["key"], "json_feed");

    let authored = read_fixture_value(
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    let parsed = serde_json::from_value::<SourceDocument>(authored)
        .expect("schema-v3 direct fragments must be authorable");
    assert!(parsed.access_paths.is_some());
}

#[test]
fn fragment_serde_rejects_structural_null_and_control_fields() {
    for invalid in [
        json!([{ "key": "json_feed", "description": null }]),
        json!([{
            "key": "json_feed",
            "discovery": {
                "strategies": [{ "key": "json_api", "fetch": { "timeoutMs": null } }]
            }
        }]),
        json!([{
            "key": "json_feed",
            "discovery": { "acceptWhen": { "minResults": null } }
        }]),
        json!([{ "key": "json_feed", "disabled": true }]),
        json!([{ "key": "json_feed", "placement": "first" }]),
    ] {
        serde_json::from_value::<Vec<AccessPathFragment>>(invalid)
            .expect_err("structural null and control fields must be rejected");
    }

    serde_json::from_value::<Vec<AccessPathFragment>>(json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "fetch": { "body": { "value": { "literalNull": null } } }
            }]
        }
    }]))
    .expect("literal JSON null remains data inside an admitted request-body value");
}

#[test]
fn representative_documents_serialize_back_without_losing_modeled_fields() {
    assert_fixture_round_trips::<SourceProfileDocument>(
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    assert_fixture_round_trips::<SourceDocument>(
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    assert_fixture_round_trips::<SourceDocument>(
        "tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json",
    );
}

#[test]
fn support_level_values_deserialize_and_serialize() {
    for (raw, expected) in [
        ("stable", SupportLevel::Stable),
        ("best_effort", SupportLevel::BestEffort),
        ("experimental", SupportLevel::Experimental),
        ("unsupported", SupportLevel::Unsupported),
    ] {
        let mut profile_json = read_fixture_value(
            "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
        );
        profile_json["support"]["level"] = json!(raw);

        let profile: SourceProfileDocument = serde_json::from_value(profile_json)
            .unwrap_or_else(|error| panic!("support level {raw} should deserialize: {error}"));

        assert_eq!(profile.support.level, expected);
        assert_eq!(
            serde_json::to_value(&profile.support).unwrap()["level"],
            raw
        );
    }
}

#[test]
fn source_status_values_deserialize_and_serialize() {
    for (raw, expected) in [
        ("draft", SourceStatus::Draft),
        ("active", SourceStatus::Active),
        ("disabled", SourceStatus::Disabled),
    ] {
        let mut source_json = read_fixture_value(
            "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
        );
        source_json["status"] = json!(raw);

        let source: SourceDocument = serde_json::from_value(source_json)
            .unwrap_or_else(|error| panic!("source status {raw} should deserialize: {error}"));

        assert_eq!(source.status, expected);
        assert_eq!(serde_json::to_value(source.status).unwrap(), raw);
    }
}

#[test]
fn v1_vocabulary_does_not_deserialize_into_new_document_model() {
    assert_fixture_deserialize_rejected::<SourceProfileDocument>(
        "tests/fixtures/source-profile-dsl/invalid/v1-adapter-key.json",
        "adapterKey",
    );
    assert_fixture_deserialize_rejected::<SourceProfileDocument>(
        "tests/fixtures/source-profile-dsl/invalid/v1-inventory.json",
        "inventory",
    );
    assert_fixture_deserialize_rejected::<SourceDocument>(
        "tests/fixtures/source-profile-dsl/invalid/v1-source-specific.json",
        "source_specific",
    );
    assert_fixture_deserialize_rejected::<SourceDocument>(
        "tests/fixtures/source-profile-dsl/invalid/v1-source-specific-pascal.json",
        "SourceSpecific",
    );

    let mut profile =
        read_fixture_value("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile["accessPaths"][0]["adapter_key"] = json!("declarative_endpoint_inventory");
    let error = serde_json::from_value::<SourceProfileDocument>(profile)
        .expect_err("snake_case adapter_key should not deserialize");
    assert!(
        error.to_string().contains("adapter_key"),
        "expected error to mention adapter_key, got {error}"
    );
}

fn assert_fixture_round_trips<T>(relative_path: &str)
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let original = read_fixture_value(relative_path);
    let document: T = read_fixture(relative_path);
    let serialized = serde_json::to_value(document)
        .unwrap_or_else(|error| panic!("failed to serialize {relative_path}: {error}"));

    assert_eq!(
        serialized, original,
        "{relative_path} should round-trip semantically"
    );
}

fn assert_fixture_deserialize_rejected<T>(relative_path: &str, expected_fragment: &str)
where
    T: serde::de::DeserializeOwned,
{
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let error = match serde_json::from_str::<T>(&contents) {
        Ok(_) => panic!("expected {relative_path} to be rejected"),
        Err(error) => error,
    };

    assert!(
        error.to_string().contains(expected_fragment),
        "expected error for {relative_path} to mention `{expected_fragment}`, got {error}"
    );
}

fn read_fixture<T>(relative_path: &str) -> T
where
    T: serde::de::DeserializeOwned,
{
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to deserialize {}: {error}", path.display()))
}

fn read_fixture_value(relative_path: &str) -> Value {
    read_fixture(relative_path)
}
