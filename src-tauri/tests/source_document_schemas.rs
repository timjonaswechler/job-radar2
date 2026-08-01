use std::{fs, path::Path};

use jsonschema::{Draft, Registry};
use serde_json::{json, Value};

const SCHEMA_FILES: &[&str] = &[
    "src/schema/check-report.schema.json",
    "crates/source-profile-dsl/schema/source-profile.schema.json",
    "crates/sources/schema/source.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/common.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/fetch.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/parse.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/predicate.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/select.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/extract.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/transform.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/pagination.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/strategy.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/policy.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/fragments.schema.json",
    "crates/source-profile-dsl/schema/source-behavior/diagnostics.schema.json",
];

#[test]
fn packaged_profile_and_source_schemas_resolve_across_owner_catalogues() {
    let harness = SchemaHarness::new();
    harness.assert_valid(
        "crates/source-profile-dsl/schema/source-profile.schema.json",
        "crates/sources/resources/profiles/greenhouse.json",
    );
    harness.assert_valid(
        "crates/sources/schema/source.schema.json",
        "crates/sources/tests/fixtures/sources/valid/source-selecting-access-path.json",
    );
    harness.assert_valid(
        "crates/sources/schema/source.schema.json",
        "crates/sources/tests/fixtures/sources/valid/source-owned-access-path.json",
    );
}

#[test]
fn check_report_schema_accepts_source_live_check_reports() {
    SchemaHarness::new().assert_json_valid(
        "src/schema/check-report.schema.json",
        json!({
            "schemaVersion": 1,
            "kind": "source_live_check",
            "subject": { "type": "source", "key": "acme_jobs" },
            "checkedAt": "2026-07-07T12:00:00Z",
            "logicVersion": "source-live-check/v1",
            "result": "failed",
            "fingerprints": [],
            "diagnostics": [{
                "category": "runtime",
                "code": "request_failed",
                "message": "Discovery request failed",
                "severity": "error",
                "path": ""
            }],
            "details": {
                "sourceStatusAtCheck": "draft",
                "liveCheckState": "live_check_failed"
            }
        }),
        true,
    );
}

#[test]
fn check_report_schema_rejects_mismatched_source_subject() {
    SchemaHarness::new().assert_json_valid(
        "src/schema/check-report.schema.json",
        json!({
            "schemaVersion": 1,
            "kind": "source_live_check",
            "subject": { "type": "source_profile", "key": "greenhouse" },
            "checkedAt": "2026-07-07T12:00:00Z",
            "logicVersion": "source-live-check/v1",
            "result": "passed",
            "fingerprints": [],
            "diagnostics": [],
            "details": {}
        }),
        false,
    );
}

struct SchemaHarness {
    manifest_dir: &'static str,
    registry: Registry<'static>,
}

impl SchemaHarness {
    fn new() -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut registry = Registry::new();
        for relative_path in SCHEMA_FILES {
            let schema = read_json(manifest_dir, relative_path);
            let schema_id = schema["$id"]
                .as_str()
                .unwrap_or_else(|| panic!("schema {relative_path} must declare $id"))
                .to_string();
            registry = registry
                .add(&schema_id, schema)
                .unwrap_or_else(|error| panic!("failed to add schema {relative_path}: {error}"));
        }
        Self {
            manifest_dir,
            registry: registry
                .prepare()
                .expect("schema registry should resolve cross-owner references"),
        }
    }

    fn assert_valid(&self, schema_path: &str, fixture_path: &str) {
        let instance = read_json(self.manifest_dir, fixture_path);
        self.assert_json_valid(schema_path, instance, true);
    }

    fn assert_json_valid(&self, schema_path: &str, instance: Value, expected_valid: bool) {
        let schema = read_json(self.manifest_dir, schema_path);
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_registry(&self.registry)
            .build(&schema)
            .unwrap_or_else(|error| panic!("schema {schema_path} should compile: {error}"));
        let errors = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            errors.is_empty(),
            expected_valid,
            "{schema_path}: {errors:#?}"
        );
    }
}

fn read_json(manifest_dir: &str, relative_path: &str) -> Value {
    let path = Path::new(manifest_dir).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {} as JSON: {error}", path.display()))
}
