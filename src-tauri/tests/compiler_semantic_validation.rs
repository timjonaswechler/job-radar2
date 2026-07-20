use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, AccessPathFragment, CompileSourceOutcome, Fetch, FieldExpression,
    RegistrySourceProfile, Select, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
    SourceProfileRegistrySnapshot, SourceStatus,
};

#[derive(Debug)]
struct TestCompileResult {
    execution_plan: Option<SourceExecutionPlan>,
    diagnostics: job_radar_lib::Diagnostics,
}

fn compile_test_source(
    source: &SourceDocument,
    profile: Option<SourceProfileDocument>,
) -> TestCompileResult {
    let registry = SourceProfileRegistrySnapshot {
        profiles: profile
            .into_iter()
            .map(|document| RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document,
            })
            .collect(),
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };
    match compile_source(source, &registry) {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } => TestCompileResult {
            execution_plan: Some(source.execution_plan),
            diagnostics,
        },
        CompileSourceOutcome::Rejected { diagnostics } => TestCompileResult {
            execution_plan: None,
            diagnostics,
        },
    }
}

#[test]
fn compiler_validates_structural_capability_compatibility() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    profile.access_paths[0].discovery.strategies[0].select = Select::Css {
        selector: ".job".to_string(),
    };
    profile.access_paths[0].discovery.strategies[0]
        .extract
        .fields
        .title = FieldExpression::CssText {
        selector: ".title".to_string(),
        cardinality: None,
        transforms: None,
    };

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "incompatible_parse_select_capability"));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "incompatible_parse_extract_capability"));
}

#[test]
fn compiler_validates_template_variable_namespaces_keys_and_context() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    if let Fetch::Http { url, .. } = &mut profile.access_paths[0].discovery.strategies[0].fetch {
        *url = "{{posting:url}}?q={{sourceConfig:missing}}&x={{unknown:thing}}".to_string();
    }
    if let Fetch::Http { url, .. } =
        &mut profile.access_paths[0].detail.as_mut().unwrap().strategies[0].fetch
    {
        *url = "{{postingMeta:missingMeta}}".to_string();
    }

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    for expected_code in [
        "template_namespace_unavailable",
        "unknown_template_key",
        "invalid_template_namespace",
    ] {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "expected diagnostic code {expected_code}, got {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn compiler_validates_capabilities_after_direct_specialization() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(
        serde_json::from_value::<Vec<AccessPathFragment>>(serde_json::json!([{
            "key": "json_feed",
            "discovery": {
                "strategies": [{
                    "key": "json_api",
                    "parse": { "type": "html" }
                }]
            }
        }]))
        .unwrap(),
    );

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "incompatible_parse_select_capability"
                && diagnostic.path == "/accessPaths/0/discovery/strategies/0/select"
                && diagnostic.strategy_key.as_deref() == Some("json_api")
        }),
        "got diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn compiler_rejects_invalid_profile_schema_before_validating_source_config() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_config.remove("feedUrl");
    source
        .source_config
        .insert("radius".to_string(), serde_json::json!(25));
    profile
        .source_config_schema
        .as_mut()
        .unwrap()
        .get_mut("properties")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "keywords".to_string(),
            serde_json::json!({ "type": "string" }),
        );

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "forbidden_search_criteria_in_source_config_schema"
    }));
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "missing_source_config_required_property"
            && diagnostic.code != "forbidden_search_criteria_in_source_config"
    }));
}

#[test]
fn compiler_validates_required_support_metadata() {
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    source.status = SourceStatus::Active;
    source.source_support = None;

    let missing_source_support = compile_test_source(&source, None);

    assert_eq!(missing_source_support.execution_plan, None);
    assert!(missing_source_support.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "missing_source_support"
            && diagnostic.path == "/sourceSupport"
            && diagnostic.details.as_ref().unwrap()["sourceKey"] == "owned_source"
    }));
}

#[test]
fn compiler_reports_duplicate_strategy_keys_within_each_step() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let duplicate_discovery = profile.access_paths[0].discovery.strategies[0].clone();
    profile.access_paths[0]
        .discovery
        .strategies
        .push(duplicate_discovery);
    let duplicate_detail = profile.access_paths[0].detail.as_ref().unwrap().strategies[0].clone();
    profile.access_paths[0]
        .detail
        .as_mut()
        .unwrap()
        .strategies
        .push(duplicate_detail);

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "duplicate_strategy_key"
            && diagnostic.path == "/accessPaths/0/discovery/strategies/1/key"
            && diagnostic.strategy_key.as_deref() == Some("json_api")
    }));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "duplicate_strategy_key"
            && diagnostic.path == "/accessPaths/0/detail/strategies/1/key"
            && diagnostic.strategy_key.as_deref() == Some("detail_api")
    }));
}

#[test]
fn compiler_reports_duplicate_reusable_access_path_keys() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let duplicate = profile.access_paths[0].clone();
    profile.access_paths.push(duplicate);

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "duplicate_access_path_key")
        .expect("duplicate reusable Access Path key should be diagnosed");
    assert_eq!(diagnostic.path, "/accessPaths/1/key");
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["accessPathKey"],
        "json_feed"
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
