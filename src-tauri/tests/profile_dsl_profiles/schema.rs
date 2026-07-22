use std::{fs, path::Path};

use jsonschema::{Draft, Registry};
use serde_json::{json, Value};

const SCHEMA_FILES: &[&str] = &[
    "src/schema/check-report.schema.json",
    "src/schema/source-profile.schema.json",
    "src/schema/source.schema.json",
    "src/schema/profile-dsl/common.schema.json",
    "src/schema/profile-dsl/fetch.schema.json",
    "src/schema/profile-dsl/parse.schema.json",
    "src/schema/profile-dsl/predicate.schema.json",
    "src/schema/profile-dsl/select.schema.json",
    "src/schema/profile-dsl/extract.schema.json",
    "src/schema/profile-dsl/transform.schema.json",
    "src/schema/profile-dsl/pagination.schema.json",
    "src/schema/profile-dsl/strategy.schema.json",
    "src/schema/profile-dsl/policy.schema.json",
    "src/schema/profile-dsl/fragments.schema.json",
    "src/schema/profile-dsl/diagnostics.schema.json",
];

#[derive(Clone, Copy)]
enum SchemaEntrypoint {
    CheckReport,
    PolicyStrategySet,
    Select,
    SourceProfile,
    Source,
}

impl SchemaEntrypoint {
    fn path(self) -> &'static str {
        match self {
            Self::CheckReport => "src/schema/check-report.schema.json",
            Self::PolicyStrategySet => "src/schema/profile-dsl/policy.schema.json",
            Self::Select => "src/schema/profile-dsl/select.schema.json",
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
fn capture_keys_are_valid_named_group_identifiers_in_complete_and_fragment_documents() {
    let harness = SchemaHarness::new();
    let mut profile = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    profile["accessPaths"][0]["discovery"]["strategies"][0]["captures"] = json!({
        "tenant_2": {
            "from": { "type": "const", "value": "acme" },
            "pattern": "^(?<tenant_2>.+)$"
        }
    });
    harness.assert_json_valid(
        SchemaEntrypoint::SourceProfile,
        profile.clone(),
        "named Capture key",
    );

    profile["accessPaths"][0]["discovery"]["strategies"][0]["captures"] = json!({
        "not/a/group": {
            "from": { "type": "const", "value": "acme" },
            "pattern": "^(?<value>.+)$"
        }
    });
    harness.assert_json_invalid(SchemaEntrypoint::SourceProfile, profile, &["captures"]);

    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    source["accessPaths"] = json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "captures": {
                    "not/a/group": { "pattern": "^(?<value>.+)$" }
                }
            }]
        }
    }]);
    harness.assert_json_invalid(SchemaEntrypoint::Source, source, &["captures"]);
}

#[test]
fn first_non_empty_candidate_loading_limit_is_enforced_by_schema() {
    let harness = SchemaHarness::new();
    let mut profile = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    let candidates = (0..16)
        .map(|index| json!({ "type": "const", "value": index }))
        .collect::<Vec<_>>();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["extract"]["providerValues"]["title"] =
        json!({ "type": "first_non_empty", "candidates": candidates });
    harness.assert_json_valid(
        SchemaEntrypoint::SourceProfile,
        profile.clone(),
        "first_non_empty exact candidate limit",
    );

    profile["accessPaths"][0]["discovery"]["strategies"][0]["extract"]["providerValues"]["title"]
        ["candidates"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "type": "const", "value": "over" }));
    harness.assert_json_invalid(SchemaEntrypoint::SourceProfile, profile, &["candidates"]);
}

#[test]
fn xml_text_select_allows_empty_current_node_path() {
    SchemaHarness::new().assert_json_valid(
        SchemaEntrypoint::Select,
        json!({ "type": "xml_text", "textPath": "" }),
        "xml_text current-node selector",
    );
}

#[test]
fn explicit_http_fetch_fragment_uses_the_canonical_timeout_ceiling() {
    let harness = SchemaHarness::new();
    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    source["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "http",
        "timeoutMs": 60000
    });
    harness.assert_json_valid(
        SchemaEntrypoint::Source,
        source.clone(),
        "explicit HTTP fragment exact timeout ceiling",
    );

    source["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["timeoutMs"] = json!(60001);
    harness.assert_json_invalid(SchemaEntrypoint::Source, source, &["60000"]);
}

#[test]
fn direct_pagination_fragment_uses_canonical_json_body_parameter_location() {
    let harness = SchemaHarness::new();
    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    source["accessPaths"][0]["discovery"]["strategies"][0]["pagination"] = json!({
        "type": "page",
        "parameterLocation": "json_body"
    });
    harness.assert_json_valid(
        SchemaEntrypoint::Source,
        source.clone(),
        "direct pagination fragment with json_body",
    );

    source["accessPaths"][0]["discovery"]["strategies"][0]["pagination"]["parameterLocation"] =
        json!("body");
    harness.assert_json_invalid(SchemaEntrypoint::Source, source, &["body"]);
}

#[test]
fn production_agent_document_schema_examples_match_schema_entrypoints() {
    let harness = SchemaHarness::new();
    let document = read_repo_file("docs/source-profile-production-agent.md");

    harness.assert_json_valid(
        SchemaEntrypoint::SourceProfile,
        extract_marked_json_block(&document, "source-profile"),
        "docs/source-profile-production-agent.md schema-test:source-profile",
    );
    harness.assert_json_valid(
        SchemaEntrypoint::Source,
        extract_marked_json_block(&document, "source"),
        "docs/source-profile-production-agent.md schema-test:source",
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
        "tests/fixtures/source-profile-dsl/invalid/detail-pagination.json",
        &["pagination"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/invalid-source-status.json",
        &["invalid"],
    );
    harness.assert_invalid(
        SchemaEntrypoint::Source,
        "tests/fixtures/source-profile-dsl/invalid/v2-source-overrides.json",
        &["sourceOverrides"],
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
        SchemaEntrypoint::SourceProfile,
        "tests/fixtures/source-profile-dsl/invalid/template-pipe.json",
        &["not"],
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
fn schema_rejects_removed_text_parse_in_profiles_and_direct_fragments() {
    let harness = SchemaHarness::new();

    let mut profile = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    profile["accessPaths"][0]["discovery"]["strategies"][0]["parse"]["type"] = json!("text");
    harness.assert_json_invalid(SchemaEntrypoint::SourceProfile, profile, &["text"]);

    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    source["accessPaths"][0]["discovery"]["strategies"][0]["parse"] = json!({ "type": "text" });
    harness.assert_json_invalid(SchemaEntrypoint::Source, source, &["text"]);
}

#[test]
fn discovery_occurrence_sections_are_disjoint_and_hint_use_is_closed() {
    let harness = SchemaHarness::new();
    let mut profile = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    let extract = &mut profile["accessPaths"][0]["discovery"]["strategies"][0]["extract"];
    extract["reference"]["providerPostingId"] =
        json!({ "type": "json_path", "jsonPath": "$.id", "cardinality": "optional" });
    extract["hints"] = json!({
        "title": {
            "value": { "type": "json_path", "jsonPath": "$.label", "cardinality": "optional" },
            "hintUse": "search_prefilter"
        }
    });
    harness.assert_json_valid(
        SchemaEntrypoint::SourceProfile,
        profile.clone(),
        "disjoint Discovery occurrence output",
    );

    let mut invalid_use = profile.clone();
    invalid_use["accessPaths"][0]["discovery"]["strategies"][0]["extract"]["hints"]["title"]
        ["hintUse"] = json!("canonical_value");
    harness.assert_json_invalid(
        SchemaEntrypoint::SourceProfile,
        invalid_use,
        &["search_prefilter"],
    );

    let mut unknown = profile.clone();
    unknown["accessPaths"][0]["discovery"]["strategies"][0]["extract"]["providerValues"]["url"] =
        json!({ "type": "const", "value": "https://example.test" });
    harness.assert_json_invalid(SchemaEntrypoint::SourceProfile, unknown, &["Additional"]);

    profile["accessPaths"][0]["discovery"]["strategies"][0]["extract"]["hints"]["title"]["value"] =
        Value::Null;
    harness.assert_json_invalid(SchemaEntrypoint::SourceProfile, profile, &["null"]);
}

#[test]
fn final_strategy_set_schema_requires_an_exact_closed_policy_object() {
    let harness = SchemaHarness::new();
    let strategy = json!({
        "key": "json_api",
        "fetch": {
            "mode": "http",
            "method": "GET",
            "url": "https://example.test/jobs",
            "timeoutMs": 1000
        },
        "parse": { "type": "json" },
        "select": { "type": "json_path", "jsonPath": "$.jobs" },
        "extract": {
            "reference": {
                "url": { "type": "json_path", "jsonPath": "$.url" }
            },
            "providerValues": {
                "title": { "type": "json_path", "jsonPath": "$.title" },
                "company": { "type": "json_path", "jsonPath": "$.company" }
            }
        }
    });

    for policy_type in ["first_accepted", "all_required"] {
        harness.assert_json_valid(
            SchemaEntrypoint::PolicyStrategySet,
            json!({
                "policy": { "type": policy_type },
                "strategies": [strategy.clone()]
            }),
            "final Strategy Set with a closed Policy",
        );
    }
    harness.assert_json_invalid(
        SchemaEntrypoint::PolicyStrategySet,
        json!({ "strategies": [strategy.clone()] }),
        &["oneOf"],
    );
    harness.assert_json_invalid(
        SchemaEntrypoint::PolicyStrategySet,
        json!({ "policy": "first_accepted", "strategies": [strategy.clone()] }),
        &["first_accepted"],
    );
    harness.assert_json_invalid(
        SchemaEntrypoint::PolicyStrategySet,
        json!({
            "policy": { "type": "first_accepted", "extra": true },
            "strategies": [strategy.clone()]
        }),
        &["extra"],
    );
    harness.assert_json_invalid(
        SchemaEntrypoint::PolicyStrategySet,
        json!({ "policy": { "type": "unknown" }, "strategies": [strategy.clone()] }),
        &["unknown"],
    );
    for invalid_policy in [
        Value::Null,
        json!({ "type": "allRequired" }),
        json!({ "type": "all_required", "count": 1 }),
        json!({ "type": "all_required", "threshold": 1 }),
        json!({ "type": "all_required", "mode": "strict" }),
        json!({ "type": "all_required", "continueAfterFailure": true }),
    ] {
        harness.assert_json_invalid(
            SchemaEntrypoint::PolicyStrategySet,
            json!({ "policy": invalid_policy, "strategies": [strategy.clone()] }),
            &["oneOf"],
        );
    }

    let limits = json!({
        "maxStrategyAttempts": 50,
        "maxRequests": 1000,
        "maxProducedItems": 100000,
        "maxDurationMs": 120000,
        "maxPages": 1000,
        "maxBrowserActions": 50,
        "maxFanOut": 100000,
        "maxResponseBytes": 67108864
    });
    harness.assert_json_valid(
        SchemaEntrypoint::PolicyStrategySet,
        json!({ "policy": { "type": "first_accepted" }, "strategies": [strategy.clone()], "limits": limits.clone() }),
        "Strategy Set with all eight phase limits",
    );
    for invalid_limits in [
        {
            let mut value = limits.clone();
            value["maxRequests"] = json!(0);
            value
        },
        {
            let mut value = limits.clone();
            value["maxRequests"] = Value::Null;
            value
        },
        {
            let mut value = limits.clone();
            value["maxRequests"] = json!(1001);
            value
        },
        {
            let mut value = limits.clone();
            value["unknownLimit"] = json!(1);
            value
        },
        {
            let mut value = limits.clone();
            value.as_object_mut().unwrap().remove("maxFanOut");
            value
        },
    ] {
        harness.assert_json_invalid(
            SchemaEntrypoint::PolicyStrategySet,
            json!({ "policy": { "type": "first_accepted" }, "strategies": [strategy.clone()], "limits": invalid_limits }),
            &["oneOf"],
        );
    }
}

#[test]
fn source_schema_accepts_direct_profile_fragments() {
    let harness = SchemaHarness::new();
    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    );
    source["accessPaths"] = json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{ "key": "json_api", "acceptWhen": { "minResults": 0 } }]
        }
    }]);

    harness.assert_json_valid(
        SchemaEntrypoint::Source,
        source,
        "schema-v3 Source with direct Access Path fragments",
    );
}

#[test]
fn source_schema_rejects_titles_on_source_owned_config_properties() {
    let harness = SchemaHarness::new();
    let mut source = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json",
    );
    source["selectedAccessPath"]["sourceConfigSchema"]["properties"]["startUrl"]["title"] =
        json!("Start URL");

    harness.assert_json_invalid(SchemaEntrypoint::Source, source, &["title", "not"]);
}

#[test]
fn schema_rejects_prohibited_browser_interactions_kept_only_for_compiler_diagnostics() {
    let harness = SchemaHarness::new();
    let mut profile = read_json(
        env!("CARGO_MANIFEST_DIR"),
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    );
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "browser",
        "url": "{{sourceConfig:feedUrl}}",
        "timeoutMs": 10000,
        "interactions": [
            { "type": "execute_script", "script": "return window.__jobs" }
        ]
    });

    harness.assert_json_invalid(
        SchemaEntrypoint::SourceProfile,
        profile,
        &["execute_script", "oneOf"],
    );
}

#[test]
fn check_report_schema_accepts_source_live_check_reports() {
    let harness = SchemaHarness::new();

    harness.assert_json_valid(
        SchemaEntrypoint::CheckReport,
        json!({
            "schemaVersion": 1,
            "kind": "source_live_check",
            "subject": {
                "type": "source",
                "key": "acme_jobs"
            },
            "checkedAt": "2026-07-07T12:00:00Z",
            "logicVersion": "source-live-check/v1",
            "result": "failed",
            "fingerprints": [],
            "diagnostics": [
                {
                    "category": "runtime",
                    "code": "request_failed",
                    "message": "Discovery request failed",
                    "severity": "error",
                    "path": ""
                }
            ],
            "details": {
                "sourceStatusAtCheck": "draft",
                "liveCheckState": "live_check_failed"
            }
        }),
        "representative Source Live Check Report",
    );
}

#[test]
fn check_report_schema_rejects_unsupported_result_and_mismatched_subject() {
    let harness = SchemaHarness::new();

    harness.assert_json_invalid(
        SchemaEntrypoint::CheckReport,
        json!({
            "schemaVersion": 1,
            "kind": "source_live_check",
            "subject": {
                "type": "source_profile",
                "key": "greenhouse"
            },
            "checkedAt": "2026-07-07T12:00:00Z",
            "logicVersion": "source-live-check/v1",
            "result": "passed",
            "fingerprints": [],
            "diagnostics": [],
            "details": {}
        }),
        &["source"],
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

    fn assert_json_valid(&self, entrypoint: SchemaEntrypoint, instance: Value, label: &str) {
        let schema_path = entrypoint.path();
        let schema = read_json(self.manifest_dir, schema_path);
        let errors = self.validate_instance(schema_path, schema, instance);
        assert!(
            errors.is_empty(),
            "expected {label} to validate against {}, but got:\n{}",
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

    fn assert_json_invalid(
        &self,
        entrypoint: SchemaEntrypoint,
        instance: Value,
        expected_fragments: &[&str],
    ) {
        let schema_path = entrypoint.path();
        let schema = read_json(self.manifest_dir, schema_path);
        let errors = self.validate_instance(schema_path, schema, instance);
        assert!(
            !errors.is_empty(),
            "expected inline instance to fail validation against {}",
            entrypoint.path()
        );

        let joined_errors = errors.join("\n");
        for expected_fragment in expected_fragments {
            assert!(
                joined_errors.contains(expected_fragment),
                "expected validation errors to mention `{expected_fragment}`, got:\n{joined_errors}"
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

fn read_repo_file(relative_path: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("src-tauri manifest directory should have a repository parent");
    let path = repo_root.join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn extract_marked_json_block(document: &str, marker: &str) -> Value {
    let marker = format!("<!-- schema-test:{marker} -->");
    let after_marker = document
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing marked JSON example {marker}"))
        .1;
    let after_fence = after_marker
        .split_once("```json")
        .unwrap_or_else(|| panic!("missing json fence after {marker}"))
        .1;
    let after_fence = after_fence.strip_prefix('\n').unwrap_or(after_fence);
    let block = after_fence
        .split_once("\n```")
        .unwrap_or_else(|| panic!("missing closing json fence after {marker}"))
        .0;

    serde_json::from_str(block)
        .unwrap_or_else(|error| panic!("failed to parse marked JSON example {marker}: {error}"))
}
