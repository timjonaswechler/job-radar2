use std::{fs, path::Path};

use jsonschema::{Draft, Registry};
use serde_json::{json, Value};

const SCHEMA_FILES: &[&str] = &[
    "src/schema/source-profile.schema.json",
    "src/schema/source.schema.json",
    "src/schema/profile-dsl/common.schema.json",
    "src/schema/profile-dsl/fetch.schema.json",
    "src/schema/profile-dsl/parse.schema.json",
    "src/schema/profile-dsl/select.schema.json",
    "src/schema/profile-dsl/extract.schema.json",
    "src/schema/profile-dsl/transform.schema.json",
    "src/schema/profile-dsl/pagination.schema.json",
    "src/schema/profile-dsl/strategy.schema.json",
    "src/schema/profile-dsl/overrides.schema.json",
    "src/schema/profile-dsl/diagnostics.schema.json",
];

#[derive(Clone, Copy)]
enum SchemaEntrypoint {
    SourceProfile,
    Source,
}

impl SchemaEntrypoint {
    fn path(self) -> &'static str {
        match self {
            Self::SourceProfile => "src/schema/source-profile.schema.json",
            Self::Source => "src/schema/source.schema.json",
        }
    }
}

#[test]
fn valid_profile_dsl_examples_match_schema_entrypoints() {
    let harness = SchemaHarness::new();

    harness.assert_valid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    harness.assert_valid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    harness.assert_valid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json",
    );
    harness.assert_valid(
        SchemaEntrypoint::SourceProfile,
        "resources/profiles/greenhouse.json",
    );
}

#[test]
fn invalid_profile_dsl_examples_are_rejected_for_expected_reason() {
    let harness = SchemaHarness::new();

    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/unbounded-pagination.json",
        &["pagination", "oneOf"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/forbidden-secrets.json",
        &["authorization", "cookie"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/missing-support.json",
        &["support", "required"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/posting-detail-pagination.json",
        &["pagination"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/invalid-source-status.json",
        &["invalid"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/source-override-transforms.json",
        &["transforms"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/v1-adapter-key.json",
        &["adapterKey"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/v1-inventory.json",
        &["inventory"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/detection-missing-timeouts.json",
        &["timeoutMs", "required"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/v1-source-specific.json",
        &["source_specific"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/v1-source-specific-pascal.json",
        &["SourceSpecific"],
    );
}

#[test]
fn structured_diagnostics_schema_matches_shared_contract() {
    let harness = SchemaHarness::new();

    harness.assert_diagnostics_valid(json!([
        {
            "category": "compiler",
            "code": "missing_template_variable",
            "message": "Source Config is missing required template variable tenant",
            "severity": "error",
            "path": "",
            "strategyKey": "json_api",
            "details": {
                "missingVariable": "tenant",
                "requiredBy": "fetch.url"
            }
        }
    ]));

    harness.assert_diagnostics_invalid(
        json!([
            {
                "code": "missing_template_variable",
                "message": "Source Config is missing required template variable tenant",
                "severity": "error",
                "path": "/sourceConfig/tenant"
            }
        ]),
        &["category"],
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
            let schema_id = schema
                .get("$id")
                .and_then(Value::as_str)
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
                .expect("schema registry should prepare without unresolved refs"),
        }
    }

    fn assert_valid(&self, entrypoint: SchemaEntrypoint, fixture_path: &str) {
        let errors = self.validate(entrypoint, fixture_path);
        assert!(
            errors.is_empty(),
            "expected {fixture_path} to validate against {}, but got:\n{}",
            entrypoint.path(),
            errors.join("\n")
        );
    }

    fn assert_invalid(
        &self,
        entrypoint: SchemaEntrypoint,
        fixture_path: &str,
        expected_fragments: &[&str],
    ) {
        let errors = self.validate(entrypoint, fixture_path);
        assert!(
            !errors.is_empty(),
            "expected {fixture_path} to fail validation against {}",
            entrypoint.path()
        );

        let joined_errors = errors.join("\n");
        for expected_fragment in expected_fragments {
            assert!(
                joined_errors.contains(expected_fragment),
                "expected validation errors for {fixture_path} to mention `{expected_fragment}`, got:\n{joined_errors}"
            );
        }
    }

    fn assert_diagnostics_valid(&self, diagnostics: Value) {
        let errors = self.validate_instance(
            "src/schema/profile-dsl/diagnostics.schema.json",
            json!({ "$ref": "https://job-radar.local/schemas/profile-dsl/diagnostics.schema.json#/$defs/diagnostics" }),
            diagnostics,
        );
        assert!(
            errors.is_empty(),
            "expected diagnostics to validate, but got:\n{}",
            errors.join("\n")
        );
    }

    fn assert_diagnostics_invalid(&self, diagnostics: Value, expected_fragments: &[&str]) {
        let errors = self.validate_instance(
            "src/schema/profile-dsl/diagnostics.schema.json",
            json!({ "$ref": "https://job-radar.local/schemas/profile-dsl/diagnostics.schema.json#/$defs/diagnostics" }),
            diagnostics,
        );
        assert!(
            !errors.is_empty(),
            "expected diagnostics to fail validation"
        );

        let joined_errors = errors.join("\n");
        for expected_fragment in expected_fragments {
            assert!(
                joined_errors.contains(expected_fragment),
                "expected diagnostic validation errors to mention `{expected_fragment}`, got:\n{joined_errors}"
            );
        }
    }

    fn validate(&self, entrypoint: SchemaEntrypoint, fixture_path: &str) -> Vec<String> {
        let schema_path = entrypoint.path();
        let schema = read_json(self.manifest_dir, schema_path);
        let instance = read_json(self.manifest_dir, fixture_path);

        self.validate_instance(schema_path, schema, instance)
    }

    fn validate_instance(&self, schema_path: &str, schema: Value, instance: Value) -> Vec<String> {
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_registry(&self.registry)
            .build(&schema)
            .unwrap_or_else(|error| panic!("schema {schema_path} should compile: {error}"));

        validator
            .iter_errors(&instance)
            .map(|error| {
                format!(
                    "instance_path={} schema_path={} error={error}",
                    error.instance_path(),
                    error.evaluation_path()
                )
            })
            .collect()
    }
}

fn read_json(manifest_dir: &str, relative_path: &str) -> Value {
    let path = Path::new(manifest_dir).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {} as JSON: {error}", path.display()))
}
