use std::{fs, path::Path};

use job_radar_lib::{
    compile_source_execution_plan, DiagnosticCategory, DiagnosticSeverity, ExecutionPlanAccessPath,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ExecutionPlanFetch,
    ExecutionPlanPagination, ProfileCompilerSnapshot, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument, SourceStatus,
};

#[test]
fn compiler_resolves_source_selecting_reusable_profile_access_path() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let snapshot = ProfileCompilerSnapshot {
        profiles: vec![profile],
        sources: vec![source],
    };

    let result = compile_source_execution_plan(&snapshot, "example_source");

    assert_eq!(result.source_key, "example_source");
    assert_eq!(result.diagnostics, Vec::new());

    let plan: SourceExecutionPlan = result
        .execution_plan
        .expect("active source with reusable access path should compile");
    assert_eq!(plan.source.key, "example_source");
    assert_eq!(plan.source.name, "Example Source");
    assert_eq!(
        plan.source_config["feedUrl"],
        "https://example.test/jobs.json"
    );
    assert_eq!(
        serde_json::to_value(&plan).unwrap().get("sourceOverrides"),
        None,
        "Execution Plan must expose the effective plan, not raw Source Overrides"
    );
    let discovery_strategy = &plan.discovery.strategies[0];
    assert_eq!(discovery_strategy.key, "json_api");
    assert_eq!(
        discovery_strategy.fetch,
        ExecutionPlanFetch::Http {
            method: Some(job_radar_lib::HttpMethod::Get),
            url: "{{sourceConfig:feedUrl}}".to_string(),
            headers: Some(std::collections::BTreeMap::from([(
                "accept".to_string(),
                "application/json".to_string(),
            )])),
            body: None,
            timeout_ms: 10000,
            retry: None,
        }
    );
    let Some(ExecutionPlanPagination::Page { limits, .. }) = &discovery_strategy.pagination else {
        panic!("expected compiled page pagination with concrete limits");
    };
    assert_eq!(limits.max_requests, Some(3));
    assert_eq!(limits.max_items, Some(100));
    assert_eq!(
        discovery_strategy.accept_when.as_ref().unwrap().min_results,
        Some(0),
        "Source Overrides must be applied before compiling the effective Execution Plan"
    );
    let detail_strategy = &plan.detail.as_ref().unwrap().strategies[0];
    assert_eq!(detail_strategy.key, "detail_api");
    assert_eq!(
        detail_strategy.fetch,
        ExecutionPlanFetch::Http {
            method: Some(job_radar_lib::HttpMethod::Get),
            url: "{{postingMeta:jobId}}".to_string(),
            headers: Some(std::collections::BTreeMap::from([(
                "accept".to_string(),
                "application/json".to_string(),
            )])),
            body: None,
            timeout_ms: 10000,
            retry: None,
        }
    );
    assert_eq!(
        plan.selected_access_path,
        ExecutionPlanAccessPath::ProfileAccessPath {
            profile_key: "example_jobs".to_string(),
            profile_name: "Example Jobs".to_string(),
            path_key: "json_feed".to_string(),
            path_name: "JSON feed".to_string(),
        }
    );
}

#[test]
fn compiler_resolves_source_owned_access_path() {
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    source.status = SourceStatus::Active;
    let snapshot = ProfileCompilerSnapshot {
        profiles: Vec::new(),
        sources: vec![source],
    };

    let result = compile_source_execution_plan(&snapshot, "owned_source");

    assert_eq!(result.diagnostics, Vec::new());
    let plan = result
        .execution_plan
        .expect("active source-owned access path should compile");
    assert_eq!(plan.source.key, "owned_source");
    assert_eq!(
        plan.source_config["startUrl"],
        "https://example.test/careers"
    );
    assert_eq!(
        serde_json::to_value(&plan).unwrap().get("sourceOverrides"),
        None,
        "Source-owned Execution Plans also must not carry raw Source Overrides"
    );
    let discovery_strategy = &plan.discovery.strategies[0];
    assert_eq!(discovery_strategy.key, "html_cards");
    let ExecutionPlanFetch::Browser {
        timeout_ms,
        waits,
        interactions,
        ..
    } = &discovery_strategy.fetch
    else {
        panic!("expected source-owned Access Path to compile a strict browser fetch");
    };
    assert_eq!(*timeout_ms, 30000);
    assert_eq!(
        waits,
        &vec![ExecutionPlanBrowserWait::Selector {
            selector: Some(".job-card".to_string()),
            timeout_ms: 10000,
        }]
    );
    assert_eq!(
        interactions,
        &vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: "button.load-more".to_string(),
            max_count: 2,
            wait_after_ms: Some(500),
        }]
    );
    assert_eq!(plan.detail, None);
    assert_eq!(
        plan.selected_access_path,
        ExecutionPlanAccessPath::SourceOwnedAccessPath {
            key: "html_page".to_string(),
            name: "HTML page".to_string(),
        }
    );
}

#[test]
fn missing_source_returns_structured_diagnostic() {
    let result = compile_source_execution_plan(&ProfileCompilerSnapshot::default(), "missing");

    assert_eq!(result.source_key, "missing");
    assert_eq!(result.execution_plan, None);
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.category, DiagnosticCategory::Compiler);
    assert_eq!(diagnostic.code, "source_not_found");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.path, "");
    assert_eq!(diagnostic.details.as_ref().unwrap()["sourceKey"], "missing");
}

#[test]
fn missing_profile_and_access_path_return_structured_diagnostics() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let missing_profile_result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: Vec::new(),
            sources: vec![source.clone()],
        },
        "example_source",
    );

    assert_eq!(missing_profile_result.execution_plan, None);
    assert_eq!(
        missing_profile_result.diagnostics[0].code,
        "source_profile_not_found"
    );
    assert_eq!(
        missing_profile_result.diagnostics[0].path,
        "/selectedAccessPath/profileKey"
    );
    assert_eq!(
        missing_profile_result.diagnostics[0]
            .details
            .as_ref()
            .unwrap()["profileKey"],
        "example_jobs"
    );

    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.access_paths.clear();
    let missing_path_result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "example_source",
    );

    assert_eq!(missing_path_result.execution_plan, None);
    let missing_path_diagnostic = missing_path_result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "access_path_not_found")
        .expect("missing Access Path should produce its structured diagnostic");
    assert_eq!(missing_path_diagnostic.path, "/selectedAccessPath/pathKey");
    assert_eq!(
        missing_path_diagnostic.details.as_ref().unwrap()["pathKey"],
        "json_feed"
    );
}

#[test]
fn draft_and_disabled_sources_do_not_produce_executable_plans() {
    for (status, expected) in [
        (SourceStatus::Draft, "draft"),
        (SourceStatus::Disabled, "disabled"),
    ] {
        let mut source: SourceDocument = read_fixture(
            "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
        );
        source.status = status;
        let profile: SourceProfileDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
        let result = compile_source_execution_plan(
            &ProfileCompilerSnapshot {
                profiles: vec![profile],
                sources: vec![source],
            },
            "example_source",
        );

        assert_eq!(result.execution_plan, None);
        assert_eq!(result.diagnostics[0].code, "source_not_executable");
        assert_eq!(result.diagnostics[0].path, "/status");
        assert_eq!(
            result.diagnostics[0].details.as_ref().unwrap()["status"],
            expected
        );
    }
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
