use jsonschema::{Draft, Registry};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const ENGINE_SCHEMAS: &[&str] = &[
    "common.schema.json",
    "fetch.schema.json",
    "parse.schema.json",
    "predicate.schema.json",
    "select.schema.json",
    "extract.schema.json",
    "transform.schema.json",
    "pagination.schema.json",
    "strategy.schema.json",
    "policy.schema.json",
    "fragments.schema.json",
    "diagnostics.schema.json",
];

#[test]
fn check_report_schema_accepts_live_check_evidence_and_rejects_mismatched_subjects() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = engine_registry(crate_dir);
    let schema = read(crate_dir.join("schema/check-report.schema.json"));
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .build(&schema)
        .unwrap();
    let report = serde_json::json!({
        "schemaVersion": 1,
        "kind": "source_live_check",
        "subject": { "type": "source", "key": "acme_jobs" },
        "checkedAt": "2026-07-07T12:00:00Z",
        "logicVersion": "source-live-check/v2",
        "result": "failed",
        "fingerprints": [],
        "diagnostics": [],
        "details": {}
    });
    assert!(validator.is_valid(&report));
    let mut invalid = report;
    invalid["subject"]["type"] = serde_json::json!("source_profile");
    assert!(!validator.is_valid(&invalid));
}

#[test]
fn source_schema_resolves_the_engine_definition_catalogue_without_copying_json() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (registry, source_schema) = source_schema(crate_dir);
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .build(&source_schema)
        .unwrap();
    for fixture in [
        "tests/fixtures/sources/valid/source-selecting-access-path.json",
        "tests/fixtures/sources/valid/source-owned-access-path.json",
    ] {
        let document = read(crate_dir.join(fixture));
        let errors = validator
            .iter_errors(&document)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "{fixture}: {errors:#?}");
    }
    assert!(!crate_dir.join("schema/source-behavior").exists());
}

#[test]
fn source_schema_rejects_migrated_source_fixtures_for_the_expected_reason() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (registry, source_schema) = source_schema(crate_dir);
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .build(&source_schema)
        .unwrap();
    for (fixture, expected) in [
        (
            "tests/fixtures/sources/invalid/invalid-source-status.json",
            "invalid",
        ),
        (
            "tests/fixtures/sources/invalid/v2-source-overrides.json",
            "sourceOverrides",
        ),
        (
            "tests/fixtures/sources/invalid/v1-source-specific.json",
            "source_specific",
        ),
        (
            "tests/fixtures/sources/invalid/v1-source-specific-pascal.json",
            "SourceSpecific",
        ),
    ] {
        let errors = validator
            .iter_errors(&read(crate_dir.join(fixture)))
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            errors.contains(expected),
            "{fixture}: expected {expected}, got {errors}"
        );
    }
}

#[test]
fn source_schema_rejects_owner_specific_authored_invariant_violations() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (registry, source_schema) = source_schema(crate_dir);
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .build(&source_schema)
        .unwrap();
    let selected =
        read(crate_dir.join("tests/fixtures/sources/valid/source-selecting-access-path.json"));
    let owned = read(crate_dir.join("tests/fixtures/sources/valid/source-owned-access-path.json"));
    let invalid = [
        {
            let mut value = selected.clone();
            value["key"] = serde_json::json!("_leading");
            value
        },
        {
            let mut value = selected.clone();
            value["name"] = serde_json::json!("");
            value
        },
        {
            let mut value = selected.clone();
            value["sourceSupport"] = serde_json::json!({"level": "experimental"});
            value
        },
        {
            let mut value = owned.clone();
            value.as_object_mut().unwrap().remove("sourceSupport");
            value
        },
        {
            let mut value = owned;
            value["accessPaths"] = serde_json::json!([]);
            value
        },
    ];
    for value in invalid {
        assert!(
            validator.iter_errors(&value).next().is_some(),
            "schema unexpectedly accepted {value}"
        );
    }
}

fn source_schema(crate_dir: &Path) -> (Registry<'static>, Value) {
    (
        engine_registry(crate_dir),
        read(crate_dir.join("schema/source.schema.json")),
    )
}

fn engine_registry(crate_dir: &Path) -> Registry<'static> {
    let engine_dir = crate_dir
        .parent()
        .unwrap()
        .join("source-engine/schema/source-behavior");
    let mut registry = Registry::new();
    for name in ENGINE_SCHEMAS {
        let schema = read(engine_dir.join(name));
        let id = schema["$id"].as_str().unwrap().to_string();
        registry = registry.add(&id, schema).unwrap();
    }
    registry.prepare().unwrap()
}

fn read(path: PathBuf) -> Value {
    serde_json::from_slice(
        &fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap()
}
