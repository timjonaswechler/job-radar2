use job_radar_lib::{
    compile_detection_plan, compile_source, production_primitive_inventories,
    validate_primitive_completeness, CompileSourceOutcome, ExecutionPlanFetch, Family,
    RegistrySourceProfile, SourceDocument, SourceProfileDocument, SourceProfileRegistrySnapshot,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[test]
fn built_in_profiles_and_detection_consume_only_final_typed_catalogue() {
    let catalogue = validate_primitive_completeness(&production_primitive_inventories()).unwrap();
    for family in [
        Family::Fetch,
        Family::Browser,
        Family::Pagination,
        Family::Parse,
        Family::Select,
        Family::Detection,
    ] {
        assert!(catalogue.iter().any(|record| record.family == family));
    }

    for (key, source) in [
        (
            "greenhouse",
            include_str!("../resources/profiles/greenhouse.json"),
        ),
        (
            "workday",
            include_str!("../resources/profiles/workday.json"),
        ),
        (
            "successfactors",
            include_str!("../resources/profiles/successfactors.json"),
        ),
    ] {
        let profile: SourceProfileDocument = serde_json::from_str(source)
            .unwrap_or_else(|error| panic!("{key} typed profile ingestion failed: {error}"));
        let plan = compile_detection_plan(&profile).unwrap_or_else(|errors| {
            panic!("{key} typed Detection compilation failed: {errors:?}")
        });
        assert_eq!(plan.profile_key(), key);
        assert!(plan.strategy_keys().next().is_some());
    }
}

fn compile_profile(value: Value) -> job_radar_lib::CompiledSource {
    let profile: SourceProfileDocument = serde_json::from_value(value).unwrap();
    let source: SourceDocument = serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json"),
        )
        .unwrap(),
    )
    .unwrap();
    match compile_source(
        &source,
        &SourceProfileRegistrySnapshot {
            profiles: vec![RegistrySourceProfile {
                origin: "g02".into(),
                path: String::new(),
                document: profile,
            }],
            sources: Vec::new(),
            diagnostics: Vec::new(),
        },
    ) {
        CompileSourceOutcome::Compiled { source, .. } => source,
        CompileSourceOutcome::Rejected { diagnostics } => {
            panic!("typed composition rejected: {diagnostics:?}")
        }
    }
}

fn profile_fixture() -> Value {
    serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn discovery_detail_http_post_browser_and_sitemap_compile_to_typed_owners() {
    let baseline = compile_profile(profile_fixture());
    assert!(matches!(
        baseline.execution_plan.discovery.strategies[0].fetch,
        ExecutionPlanFetch::Http(_)
    ));
    assert!(baseline
        .execution_plan
        .detail
        .as_ref()
        .is_some_and(|detail| matches!(detail.strategies[0].fetch, ExecutionPlanFetch::Http(_))));

    let mut post = profile_fixture();
    for phase in ["discovery", "detail"] {
        let fetch = &mut post["accessPaths"][0][phase]["strategies"][0]["fetch"];
        fetch["method"] = json!("POST");
        fetch["body"] = json!({"type":"json","value":{"query":"jobs"}});
    }
    let post = compile_profile(post);
    assert!(matches!(
        post.execution_plan.discovery.strategies[0].fetch,
        ExecutionPlanFetch::Http(_)
    ));
    assert!(post
        .execution_plan
        .detail
        .as_ref()
        .is_some_and(|detail| matches!(detail.strategies[0].fetch, ExecutionPlanFetch::Http(_))));

    let mut sitemap = profile_fixture();
    let successfactors: Value =
        serde_json::from_str(include_str!("../resources/profiles/successfactors.json")).unwrap();
    let mut sitemap_strategy =
        successfactors["accessPaths"][0]["discovery"]["strategies"][0].clone();
    sitemap_strategy["key"] = json!("json_api");
    sitemap_strategy["fetch"]["url"] = json!("https://example.test/sitemap.xml");
    sitemap["accessPaths"][0]["discovery"]["strategies"][0] = sitemap_strategy;
    assert!(
        compile_profile(sitemap).execution_plan.discovery.strategies[0]
            .pagination
            .is_some()
    );

    let mut browser = profile_fixture();
    for phase in ["discovery", "detail"] {
        browser["accessPaths"][0][phase]["strategies"][0]["fetch"] = json!({
            "mode":"browser","url":"https://example.test","timeoutMs":3000,
            "waits":[{"type":"selector","selector":"main","timeoutMs":1000}],"interactions":[]
        });
    }
    let browser = compile_profile(browser);
    assert!(matches!(
        browser.execution_plan.discovery.strategies[0].fetch,
        ExecutionPlanFetch::Browser { .. }
    ));
    assert!(browser
        .execution_plan
        .detail
        .as_ref()
        .is_some_and(|detail| matches!(
            detail.strategies[0].fetch,
            ExecutionPlanFetch::Browser { .. }
        )));

    let mut detection_browser = profile_fixture();
    detection_browser["detection"] = json!({
        "policy":{"type":"all_required"},
        "strategies":[
            {"type":"url","key":"url","input":{"type":"absolute_url"}},
            {"type":"browser","key":"render","fetch":{
                "mode":"browser","url":"{{inputUrl}}","timeoutMs":3000
            },"contains":"jobs"}
        ]
    });
    let detection_browser: SourceProfileDocument =
        serde_json::from_value(detection_browser).unwrap();
    let detection_plan = compile_detection_plan(&detection_browser).unwrap();
    assert_eq!(
        detection_plan.strategy_keys().collect::<Vec<_>>(),
        ["url", "render"]
    );
}

#[test]
fn application_callers_type_check_at_source_live_check_and_search_run_boundaries() {
    use job_radar_lib::{BrowserAcquisition, ProfileHttpClient};

    fn source_live_check_boundary<D, T, A>(discovery: &D, detail: &T, browser: &A)
    where
        D: ProfileHttpClient + Sync + ?Sized,
        T: ProfileHttpClient + Sync + ?Sized,
        A: BrowserAcquisition + Sync,
    {
        let result = job_radar_lib::check_source_with_runtime(
            "/tmp/g02-typecheck",
            "source",
            discovery,
            detail,
            browser,
        );
        drop(result);
    }
    let _ = source_live_check_boundary::<
        job_radar_lib::ScriptedProfileHttpClient,
        job_radar_lib::ScriptedProfileHttpClient,
        job_radar_lib::ScriptedBrowserAcquisition,
    >;
}

#[test]
fn positive_catalogue_distinguishes_http_and_browser_byte_dimensions() {
    let catalogue = validate_primitive_completeness(&production_primitive_inventories()).unwrap();
    assert!(catalogue.iter().any(
        |record| record.family == Family::PhaseLimit && record.key == "maxBrowserRenderedBytes"
    ));
    let phase_schema =
        include_str!("../crates/source-profile-dsl/src/schema/profile-dsl/policy.schema.json");
    let detection_source =
        include_str!("../crates/source-profile-dsl/src/source_profile/detection/runtime.rs");
    assert!(phase_schema.contains("maxBrowserRenderedBytes"));
    assert!(detection_source.contains("response_bytes"));
    assert!(detection_source.contains("browser_rendered_bytes"));
    assert_ne!("response_bytes", "browser_rendered_bytes");
    assert!(
        !include_str!("../crates/source-profile-dsl/src/schema/profile-dsl/fetch.schema.json")
            .contains("maxBrowserRenderedBytes")
    );
}
