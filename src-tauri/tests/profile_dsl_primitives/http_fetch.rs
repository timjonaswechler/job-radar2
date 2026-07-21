use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, http_fetch_descriptors, validate_http_fetch_descriptors, CompileSourceOutcome,
    ExecutionPlanFetch, HttpMethod, RegistrySourceProfile, SourceDocument, SourceProfileDocument,
    SourceProfileRegistrySnapshot, HTTP_FETCH_DESCRIPTOR,
};
use serde_json::{json, Value};

#[test]
fn http_fetch_catalogue_has_one_complete_owner_and_rejects_missing_or_duplicate_sets() {
    assert_eq!(
        validate_http_fetch_descriptors(http_fetch_descriptors()),
        Ok(())
    );
    assert!(validate_http_fetch_descriptors(&[]).is_err());
    assert!(
        validate_http_fetch_descriptors(&[HTTP_FETCH_DESCRIPTOR, HTTP_FETCH_DESCRIPTOR,]).is_err()
    );
    assert_eq!(HTTP_FETCH_DESCRIPTOR.methods, &["GET", "POST"]);
    assert_eq!(HTTP_FETCH_DESCRIPTOR.body_types, &["json", "text", "form"]);
}

#[test]
fn direct_serde_requires_http_timeout_within_the_authored_ceiling() {
    let base = profile_value();
    for (label, timeout) in [
        ("zero", Some(json!(0))),
        ("above ceiling", Some(json!(60_001))),
        ("missing", None),
    ] {
        let mut profile = base.clone();
        let fetch = profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]
            .as_object_mut()
            .unwrap();
        match timeout {
            Some(timeout) => {
                fetch.insert("timeoutMs".to_string(), timeout);
            }
            None => {
                fetch.remove("timeoutMs");
            }
        }
        assert!(
            serde_json::from_value::<SourceProfileDocument>(profile).is_err(),
            "{label} timeout must reject in direct Serde"
        );
    }

    let mut profile = base;
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["timeoutMs"] = json!(60_000);
    assert!(serde_json::from_value::<SourceProfileDocument>(profile).is_ok());
}

#[test]
fn compiler_defaults_omitted_method_to_concrete_get_and_rejects_get_body() {
    let mut profile = profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]
        .as_object_mut()
        .unwrap()
        .remove("method");
    let CompileSourceOutcome::Compiled { source, .. } = compile_profile(profile) else {
        panic!("omitted method should compile as GET");
    };
    let ExecutionPlanFetch::Http(fetch) = &source.execution_plan.discovery.strategies[0].fetch
    else {
        panic!("fixture must compile an HTTP Fetch");
    };
    assert_eq!(fetch.method, HttpMethod::Get);

    let mut profile = profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["body"] =
        json!({ "type": "text", "value": "illegal" });
    let CompileSourceOutcome::Rejected { diagnostics } = compile_profile(profile) else {
        panic!("GET body must reject during compilation");
    };
    assert_eq!(diagnostics[0].code, "unsupported_http_body_for_method");
}

#[test]
fn compiler_owns_public_header_and_recursive_json_body_security() {
    let mut profile = profile_value();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["headers"] =
        json!({ "Accept": "application/json" });
    let CompileSourceOutcome::Rejected { diagnostics } = compile_profile(profile) else {
        panic!("mixed-case header outside the exact authored allowlist must reject");
    };
    assert_eq!(diagnostics[0].code, "forbidden_request_header");

    let mut profile = profile_value();
    let fetch = &mut profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"];
    fetch["method"] = json!("POST");
    fetch["body"] = json!({
        "type": "json",
        "value": { "nested": [{ "sessionToken": "secret" }] }
    });
    let CompileSourceOutcome::Rejected { diagnostics } = compile_profile(profile) else {
        panic!("recursive secret-like JSON key must reject");
    };
    assert_eq!(diagnostics[0].code, "secret_like_request_body_field");
    assert!(diagnostics[0]
        .path
        .ends_with("/fetch/body/value/nested/0/sessionToken"));
}

fn compile_profile(value: Value) -> CompileSourceOutcome {
    let profile: SourceProfileDocument = serde_json::from_value(value).unwrap();
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    compile_source(
        &source,
        &SourceProfileRegistrySnapshot {
            profiles: vec![RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document: profile,
            }],
            sources: Vec::new(),
            diagnostics: Vec::new(),
        },
    )
}

fn profile_value() -> Value {
    read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json")
}

fn read_fixture<T: serde::de::DeserializeOwned>(relative_path: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}
