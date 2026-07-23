use crate::support::accepted_phase;
use std::{fs, path::Path};

use job_radar_lib::{
    PhaseBrowser,
    compile_source, compile_template, execute_discovery, CompileSourceOutcome, CompiledHttpFetch,
    CompiledPagination, DiagnosticCategory, DiagnosticSeverity, ExecutionPlanAccessPath,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ExecutionPlanFetch,
    RegistrySourceProfile, RuntimeExecutionContext, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
    SourceProfileRegistrySnapshot, SourceStatus, TemplateDescriptor,
};

#[test]
fn compiler_resolves_source_selecting_reusable_profile_access_path() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let plan = compiled_plan(&source, Some(profile));

    assert_eq!(plan.source.key, "example_source");
    assert_eq!(plan.source.name, "Example Source");
    let serialized_plan = serde_json::to_string(&plan).unwrap();
    assert!(!serialized_plan.contains("https://example.test/jobs.json"));
    let discovery_strategy = &plan.discovery.strategies[0];
    assert_eq!(discovery_strategy.key, "json_api");
    assert_eq!(
        discovery_strategy.fetch,
        ExecutionPlanFetch::Http(CompiledHttpFetch {
            method: job_radar_lib::HttpMethod::Get,
            url: compile_template(
                "{{sourceConfig:feedUrl}}",
                &TemplateDescriptor::new().allow_namespace("sourceConfig", ["feedUrl"])
            )
            .unwrap(),
            headers: std::collections::BTreeMap::from([(
                "accept".to_string(),
                compile_template("application/json", &TemplateDescriptor::new()).unwrap(),
            )]),
            body: None,
            timeout_ms: 10000,
        })
    );
    let Some(CompiledPagination::Page(pagination)) = &discovery_strategy.pagination else {
        panic!("expected compiled page pagination with concrete limits");
    };
    assert_eq!(pagination.limits.max_requests, 3);
    assert_eq!(pagination.limits.max_items, Some(100));
    assert_eq!(
        discovery_strategy.accept_when.as_ref().unwrap().min_results,
        Some(0),
        "direct specialization must be compiled into the effective plan"
    );
    assert_eq!(
        plan.detail.as_ref().unwrap().strategies[0].key,
        "detail_api"
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
fn resolved_source_config_is_ephemeral_runtime_input_and_absent_from_the_plan() {
    const SENTINEL: &str = "https://resolved-source-config-sentinel.invalid/jobs";
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.access_paths[0].discovery.strategies[0].pagination = None;
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source
        .source_config
        .insert("feedUrl".to_string(), serde_json::json!(SENTINEL));
    let plan = compiled_plan(&source, Some(profile));

    let serialized_plan = serde_json::to_string(&plan).unwrap();
    assert!(!serialized_plan.contains(SENTINEL));

    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: SENTINEL.to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            br#"{"jobs":[{"id":"42","title":"Rust Engineer","url":"https://example.test/jobs/42","locations":[]}] }"#.to_vec(),
        )],
        content_length: None,
    }]);
    let phase_result = tauri::async_runtime::block_on(execute_discovery(
        &plan,
        &source.source_config,
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    assert!(!serde_json::to_string(&phase_result)
        .unwrap()
        .contains(SENTINEL));
    let result = accepted_phase(phase_result);

    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(fetcher.requests()[0].url, SENTINEL);
}

#[test]
fn compiler_resolves_source_owned_access_path() {
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    source.status = SourceStatus::Active;

    let plan = compiled_plan(&source, None);

    assert_eq!(plan.source.key, "owned_source");
    let serialized_plan = serde_json::to_string(&plan).unwrap();
    assert!(!serialized_plan.contains("https://example.test/careers"));
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
            selector: ".job-card".to_string(),
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
fn missing_profile_and_access_path_return_structured_diagnostics() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let missing_profile = compile_source(&source, &SourceProfileRegistrySnapshot::default());
    let CompileSourceOutcome::Rejected { diagnostics } = missing_profile else {
        panic!("missing profile must reject");
    };
    assert_eq!(diagnostics[0].category, DiagnosticCategory::Compiler);
    assert_eq!(diagnostics[0].code, "source_profile_not_found");
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostics[0].path, "/selectedAccessPath/profileKey");

    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.access_paths.clear();
    let mut source = source;
    source.access_paths = None;
    let registry = registry(Some(profile));
    let CompileSourceOutcome::Rejected { diagnostics } = compile_source(&source, &registry) else {
        panic!("missing Access Path must reject");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "access_path_not_found")
        .expect("missing Access Path should produce its structured diagnostic");
    assert_eq!(diagnostic.path, "/selectedAccessPath/pathKey");
}

fn compiled_plan(
    source: &SourceDocument,
    profile: Option<SourceProfileDocument>,
) -> SourceExecutionPlan {
    match compile_source(source, &registry(profile)) {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } if diagnostics.is_empty() => source.execution_plan,
        outcome => panic!("expected compiled Source, got {outcome:?}"),
    }
}

fn registry(profile: Option<SourceProfileDocument>) -> SourceProfileRegistrySnapshot {
    SourceProfileRegistrySnapshot {
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
