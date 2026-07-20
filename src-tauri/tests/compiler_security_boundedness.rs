use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, CompileSourceOutcome, DiagnosticCategory, DiagnosticSeverity,
    RegistrySourceProfile, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
    SourceProfileRegistrySnapshot,
};
use serde_json::{json, Value};

#[derive(Debug)]
struct TestCompileResult {
    execution_plan: Option<SourceExecutionPlan>,
    diagnostics: job_radar_lib::Diagnostics,
}

impl From<CompileSourceOutcome> for TestCompileResult {
    fn from(outcome: CompileSourceOutcome) -> Self {
        match outcome {
            CompileSourceOutcome::Compiled {
                source,
                diagnostics,
            } => Self {
                execution_plan: Some(source.execution_plan),
                diagnostics,
            },
            CompileSourceOutcome::Rejected { diagnostics } => Self {
                execution_plan: None,
                diagnostics,
            },
        }
    }
}

#[test]
fn phase_limits_resolve_to_backend_ceiling_and_authored_values_can_only_tighten() {
    let result = compile_profile_value(simple_profile_value());
    let plan = result.execution_plan.expect("omitted limits compile");
    assert_eq!(plan.discovery.limits, job_radar_lib::PhaseLimits::BACKEND);
    assert!(!plan.discovery.limits_authored);

    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["limits"] = phase_limits(7);
    let result = compile_profile_value(profile);
    let plan = result.execution_plan.expect("tightened limits compile");
    assert_eq!(plan.discovery.limits.max_requests, 7);
    assert!(plan.discovery.limits_authored);
}

#[test]
fn compiler_rejects_above_ceiling_and_inherited_limit_raises() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["limits"] = phase_limits(1_001);
    let result = compile_profile_value(profile);
    assert!(result.execution_plan.is_none());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "phase_limit_out_of_bounds"
            && diagnostic.path.ends_with("/discovery/limits/maxRequests")));

    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["limits"] = phase_limits(4);
    let mut source: Value =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source["accessPaths"] =
        json!([{ "key": "json_feed", "discovery": { "limits": { "maxRequests": 5 } } }]);
    let result = compile_profile_and_source_values(profile, source);
    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "phase_limit_cannot_raise_inherited"
        && diagnostic.path == "/accessPaths/0/discovery/limits/maxRequests"));
}

#[test]
fn partial_direct_limit_fragment_inherits_backend_values_and_tightens_one_dimension() {
    let profile = simple_profile_value();
    let mut source: Value =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source["accessPaths"] =
        json!([{ "key": "json_feed", "discovery": { "limits": { "maxRequests": 3 } } }]);
    let result = compile_profile_and_source_values(profile, source);
    let limits = result
        .execution_plan
        .unwrap_or_else(|| panic!("partial tightening compiles: {:?}", result.diagnostics))
        .discovery
        .limits;
    assert_eq!(limits.max_requests, 3);
    assert_eq!(limits.max_produced_items, 100_000);
}

#[test]
fn browser_phase_duration_must_preserve_the_two_second_teardown_reserve() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "browser",
        "url": "{{sourceConfig:feedUrl}}",
        "timeoutMs": 10000
    });
    profile["accessPaths"][0]["discovery"]["limits"] = json!({
        "maxStrategyAttempts": 50,
        "maxRequests": 1000,
        "maxProducedItems": 100000,
        "maxDurationMs": 1999,
        "maxPages": 1000,
        "maxBrowserActions": 50,
        "maxFanOut": 100000
    });

    let result = compile_profile_value(profile);

    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "browser_phase_duration_below_teardown_reserve"
            && diagnostic.path.ends_with("/discovery/limits/maxDurationMs")
    }));
}

#[test]
fn compiler_diagnoses_missing_http_fetch_timeout() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]
        .as_object_mut()
        .unwrap()
        .remove("timeoutMs");

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "missing_fetch_timeout",
        "/accessPaths/0/discovery/strategies/0/fetch/timeoutMs",
    );
}

#[test]
fn compiler_allows_public_headers_and_static_technical_body_parameters() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "http",
        "method": "POST",
        "url": "{{sourceConfig:feedUrl}}",
        "headers": {
            "accept": "application/json",
            "content-type": "application/json",
            "user-agent": "Job Radar fixture",
            "x-requested-with": "XMLHttpRequest",
            "referer": "https://example.test/careers"
        },
        "body": {
            "type": "json",
            "value": {
                "limit": 25,
                "offset": 0,
                "tenant": "example",
                "locale": "en-US"
            }
        },
        "timeoutMs": 10000
    });

    let result = compile_profile_value(profile);

    assert_eq!(result.diagnostics, vec![]);
    assert!(result.execution_plan.is_some());
}

#[test]
fn compiler_diagnoses_forbidden_headers_secret_body_fields_and_collects_multiple_diagnostics() {
    let mut profile = simple_profile_value();
    let fetch = &mut profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"];
    fetch["headers"] = json!({
        "authorization": "Bearer secret",
        "cookie": "session=secret",
        "set-cookie": "session=secret",
        "x-api-key": "secret",
        "proxy-authorization": "Basic secret"
    });
    fetch["method"] = json!("POST");
    fetch["body"] = json!({
        "type": "json",
        "value": {
            "limit": 25,
            "password": "secret",
            "nested": {
                "apiKey": "secret",
                "sessionToken": "secret"
            }
        }
    });
    fetch.as_object_mut().unwrap().remove("timeoutMs");

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    for code in [
        "forbidden_request_header",
        "secret_like_request_body_field",
        "missing_fetch_timeout",
    ] {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "expected diagnostic code {code}, got {:?}",
            result.diagnostics
        );
    }
    assert!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "forbidden_request_header")
            .count()
            >= 5,
        "expected all forbidden headers to be diagnosed, got {:?}",
        result.diagnostics
    );
    assert_compiler_error(
        &result,
        "secret_like_request_body_field",
        "/accessPaths/0/discovery/strategies/0/fetch/body/value/nested/apiKey",
    );
}

#[test]
fn compiler_validates_security_and_boundedness_after_direct_specialization() {
    let profile = simple_profile_value();
    let mut source: Value =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source["accessPaths"] = json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "{{sourceConfig:feedUrl}}",
                    "headers": { "authorization": "Bearer secret" },
                    "timeoutMs": 0
                }
            }]
        }
    }]);

    let result = compile_profile_and_source_values(profile, source);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "missing_fetch_timeout",
        "/accessPaths/0/discovery/strategies/0/fetch/timeoutMs",
    );
    assert_compiler_error(
        &result,
        "forbidden_request_header",
        "/accessPaths/0/discovery/strategies/0/fetch/headers/authorization",
    );
}

#[test]
fn compiler_diagnoses_unbounded_pagination() {
    let mut profile = simple_profile_value();
    let strategy = &mut profile["accessPaths"][0]["discovery"]["strategies"][0];
    strategy["pagination"]
        .as_object_mut()
        .unwrap()
        .remove("limits");

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "unbounded_pagination",
        "/accessPaths/0/discovery/strategies/0/pagination/limits",
    );
}

#[test]
fn compiler_diagnoses_unbounded_recursive_sitemap_depth() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["pagination"] = json!({
        "type": "sitemap",
        "childSitemapSelector": { "type": "sitemap_urls" },
        "postingUrlSelector": { "type": "sitemap_urls" },
        "limits": { "maxRequests": 10 }
    });

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "unbounded_pagination_depth",
        "/accessPaths/0/discovery/strategies/0/pagination/limits/maxDepth",
    );
}

#[test]
fn compiler_diagnoses_unbounded_browser_waits_and_interactions() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "browser",
        "url": "{{sourceConfig:feedUrl}}",
        "timeoutMs": 10000,
        "waits": [
            { "type": "selector", "selector": ".jobs" }
        ],
        "interactions": [
            { "type": "click_until_gone", "selector": ".load-more" }
        ]
    });

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "unbounded_browser_wait",
        "/accessPaths/0/discovery/strategies/0/fetch/waits/0/timeoutMs",
    );
    assert_compiler_error(
        &result,
        "unbounded_browser_interaction",
        "/accessPaths/0/discovery/strategies/0/fetch/interactions/0/maxCount",
    );
}

#[test]
fn compiler_diagnoses_empty_fallback_strategy_lists() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"] = json!([]);
    profile["accessPaths"][0]["detail"]["strategies"] = json!([]);

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    assert_compiler_error(
        &result,
        "empty_fallback_strategy_list",
        "/accessPaths/0/discovery/strategies",
    );
    assert_compiler_error(
        &result,
        "empty_fallback_strategy_list",
        "/accessPaths/0/detail/strategies",
    );
}

#[test]
fn compiler_diagnoses_prohibited_browser_behaviors() {
    let mut profile = simple_profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"] = json!({
        "mode": "browser",
        "url": "{{sourceConfig:feedUrl}}",
        "timeoutMs": 10000,
        "interactions": [
            { "type": "execute_script", "script": "return window.__jobs" },
            { "type": "eval", "expression": "document.body.innerHTML" },
            { "type": "mutate_dom", "selector": "body", "mutation": "remove overlays" },
            { "type": "login_flow", "selector": "form.login" },
            { "type": "captcha_bypass", "provider": "example" }
        ]
    });

    let result = compile_profile_value(profile);

    assert_eq!(result.execution_plan, None);
    let prohibited = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "prohibited_browser_behavior")
        .count();
    assert_eq!(prohibited, 5, "got diagnostics: {:?}", result.diagnostics);
    assert_compiler_error(
        &result,
        "prohibited_browser_behavior",
        "/accessPaths/0/discovery/strategies/0/fetch/interactions/0",
    );
}

fn compile_profile_value(profile: Value) -> TestCompileResult {
    let mut source: Value =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.as_object_mut().unwrap().remove("accessPaths");
    compile_profile_and_source_values(profile, source)
}

fn compile_profile_and_source_values(profile: Value, source: Value) -> TestCompileResult {
    let profile: SourceProfileDocument = serde_json::from_value(profile)
        .unwrap_or_else(|error| panic!("profile should deserialize: {error}"));
    let source: SourceDocument = serde_json::from_value(source)
        .unwrap_or_else(|error| panic!("source should deserialize: {error}"));

    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile,
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };
    TestCompileResult::from(compile_source(&source, &registry))
}

fn assert_compiler_error(result: &TestCompileResult, expected_code: &str, expected_path: &str) {
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == expected_code && diagnostic.path == expected_path)
        .unwrap_or_else(|| {
            panic!(
                "expected compiler diagnostic {expected_code} at {expected_path}, got {:?}",
                result.diagnostics
            )
        });

    assert_eq!(diagnostic.category, DiagnosticCategory::Compiler);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    if expected_code != "empty_fallback_strategy_list" {
        assert_eq!(diagnostic.strategy_key.as_deref(), Some("json_api"));
    }
    assert!(
        diagnostic.details.is_some(),
        "diagnostic should carry machine-readable details: {diagnostic:?}"
    );
}

fn phase_limits(max_requests: u64) -> Value {
    json!({
        "maxStrategyAttempts": 50,
        "maxRequests": max_requests,
        "maxProducedItems": 100000,
        "maxDurationMs": 120000,
        "maxPages": 1000,
        "maxBrowserActions": 50,
        "maxFanOut": 100000
    })
}

fn simple_profile_value() -> Value {
    read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json")
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
