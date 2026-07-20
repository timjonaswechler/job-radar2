mod support;

use support::{compile_test_source, execute_detail_test, execute_discovery_test, unwrap_plan};

use std::{fs, future::Future, path::Path};

use job_radar_lib::{
    DetailPostingOccurrence, DiagnosticCategory, DiagnosticSeverity, DiscoveryCandidate,
    PhaseCompletion, ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
    SourceDocument, SourceProfileDocument, SupportLevel,
};
use serde_json::{json, Value};

#[test]
fn successfactors_builtin_profile_compiles_and_executes_sitemap_html_fallback_fixtures() {
    let profile_text = read_text("resources/profiles/successfactors.json");
    assert_no_v1_profile_vocabulary(&profile_text);

    let profile_value: Value = serde_json::from_str(&profile_text).unwrap();
    assert_detects_named_successfactors_source_config_captures(&profile_value);
    let profile: SourceProfileDocument = serde_json::from_value(profile_value)
        .expect("SAP SuccessFactors built-in profile should be a Source Profile DSL document");
    assert_eq!(profile.schema_version, 3);
    assert_eq!(profile.support.level, SupportLevel::Stable);

    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "acme_successfactors",
        "name": "Acme Robotics",
        "status": "active",
        "sourceConfig": {
            "baseUrl": "https://jobs.example-successfactors.test",
            "sitemapUrl": "https://jobs.example-successfactors.test/sitemap.xml"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "successfactors",
            "pathKey": "rmk_sitemap_html"
        }
    }))
    .unwrap();

    let compile_result = compile_test_source(&source, Some(profile));
    let plan = unwrap_plan(compile_result);

    let fetcher = offline_fetcher([
        (
            "https://jobs.example-successfactors.test/sitemap.xml",
            read_text("tests/fixtures/successfactors/posting-discovery-sitemap.xml"),
        ),
        (
            "https://jobs.example-successfactors.test/job/berlin-senior-platform-engineer-1001",
            read_text("tests/fixtures/successfactors/posting-detail-1001-primary.html"),
        ),
        (
            "https://jobs.example-successfactors.test/job/munich-data-product-manager-2002",
            read_text("tests/fixtures/successfactors/posting-detail-2002-fallback.html"),
        ),
        (
            "https://jobs.example-successfactors.test/job/munich-data-product-manager-2002",
            read_text("tests/fixtures/successfactors/posting-detail-2002-fallback.html"),
        ),
        (
            "https://jobs.example-successfactors.test/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/",
            read_text("tests/fixtures/successfactors/posting-detail-1405371733-schott.html"),
        ),
        (
            "https://jobs.example-successfactors.test/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/",
            read_text("tests/fixtures/successfactors/posting-detail-1405371733-schott.html"),
        ),
    ]);

    let discovery = block_on(execute_discovery_test(&plan, &fetcher));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<DiscoveryCandidate> =
        read_json("tests/fixtures/successfactors/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);
    let discovery_report = discovery.report.as_ref().expect("Discovery report");
    assert_eq!(discovery_report.completion, PhaseCompletion::Accepted);
    assert_eq!(discovery_report.usage.strategy_attempts, 1);
    assert_eq!(discovery_report.usage.requests, 1);
    assert_eq!(discovery_report.usage.pages, 1);
    assert_eq!(
        discovery_report.usage.produced_items,
        discovery.candidates.len() as u64
    );

    let primary_candidate = &discovery.candidates[0];
    let primary_detail = block_on(execute_detail_test(
        &plan,
        &posting_occurrence(primary_candidate),
        &fetcher,
    ));
    assert_eq!(primary_detail.diagnostics, Vec::new());
    let primary_report = primary_detail.report.as_ref().expect("Detail report");
    assert_eq!(primary_report.completion, PhaseCompletion::Accepted);
    assert_eq!(primary_report.usage.strategy_attempts, 1);
    assert_eq!(primary_report.usage.requests, 1);
    assert_eq!(primary_report.usage.produced_items, 1);
    let expected_primary_detail: Value =
        read_json("tests/fixtures/successfactors/posting-detail-1001-expected.json");
    assert_eq!(
        primary_detail.description_text.as_deref(),
        expected_primary_detail["descriptionText"].as_str()
    );

    let fallback_candidate = &discovery.candidates[1];
    let fallback_detail = block_on(execute_detail_test(
        &plan,
        &posting_occurrence(fallback_candidate),
        &fetcher,
    ));
    let expected_fallback_detail: Value =
        read_json("tests/fixtures/successfactors/posting-detail-2002-expected.json");
    assert_eq!(
        fallback_detail.description_text.as_deref(),
        expected_fallback_detail["descriptionText"].as_str()
    );
    let fallback_report = fallback_detail
        .report
        .as_ref()
        .expect("fallback Detail report");
    assert_eq!(fallback_report.completion, PhaseCompletion::Accepted);
    assert_eq!(fallback_report.usage.strategy_attempts, 2);
    assert_eq!(fallback_report.usage.requests, 2);
    assert_eq!(fallback_report.usage.produced_items, 1);
    assert_eq!(fallback_detail.diagnostics.len(), 1);
    assert_eq!(
        fallback_detail.diagnostics[0].category,
        DiagnosticCategory::Runtime
    );
    assert_eq!(
        fallback_detail.diagnostics[0].severity,
        DiagnosticSeverity::Error
    );
    assert_eq!(fallback_detail.diagnostics[0].code, "description_empty");
    assert_eq!(
        fallback_detail.diagnostics[0].strategy_key.as_deref(),
        Some("primary_html_description")
    );

    let schott_style_candidate = &discovery.candidates[3];
    let schott_style_detail = block_on(execute_detail_test(
        &plan,
        &posting_occurrence(schott_style_candidate),
        &fetcher,
    ));
    let expected_schott_style_detail: Value =
        read_json("tests/fixtures/successfactors/posting-detail-1405371733-expected.json");
    assert_eq!(
        schott_style_detail.description_text.as_deref(),
        expected_schott_style_detail["descriptionText"].as_str()
    );
    assert_eq!(schott_style_detail.diagnostics.len(), 1);
    assert_eq!(schott_style_detail.diagnostics[0].code, "description_empty");
    assert_eq!(
        schott_style_detail.diagnostics[0].strategy_key.as_deref(),
        Some("primary_html_description")
    );

    assert_eq!(
        fetcher.requests().into_iter().map(|request| request.url).collect::<Vec<_>>(),
        vec![
            "https://jobs.example-successfactors.test/sitemap.xml".to_string(),
            "https://jobs.example-successfactors.test/job/berlin-senior-platform-engineer-1001"
                .to_string(),
            "https://jobs.example-successfactors.test/job/munich-data-product-manager-2002"
                .to_string(),
            "https://jobs.example-successfactors.test/job/munich-data-product-manager-2002"
                .to_string(),
            "https://jobs.example-successfactors.test/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/"
                .to_string(),
            "https://jobs.example-successfactors.test/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/"
                .to_string(),
        ]
    );
}

fn assert_no_v1_profile_vocabulary(profile_text: &str) {
    for forbidden in [
        "adapterKey",
        "inventory",
        "SourceSpecific",
        "source_specific",
    ] {
        assert!(
            !profile_text.contains(forbidden),
            "SAP SuccessFactors DSL profile must not contain v1 vocabulary `{forbidden}`"
        );
    }
}

fn assert_detects_named_successfactors_source_config_captures(profile: &Value) {
    let captures = profile["detection"]["inputUrlPatterns"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|pattern| pattern["captures"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    for expected in ["successFactorsHost"] {
        assert!(
            captures.contains(&expected),
            "SAP SuccessFactors profile should declare detection capture `{expected}`"
        );
    }
}

fn posting_occurrence(candidate: &DiscoveryCandidate) -> DetailPostingOccurrence {
    DetailPostingOccurrence {
        url: candidate.url.clone(),
        title: Some(candidate.title.clone()),
        company: Some(candidate.company.clone()),
        locations: candidate.locations.clone(),
        description_text: candidate.description_text.clone(),
        posting_meta: candidate.posting_meta.clone(),
    }
}

fn offline_fetcher(
    responses: impl IntoIterator<Item = (&'static str, String)>,
) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new(responses.into_iter().map(|(url, body)| {
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: url.to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(body.into_bytes())],
            content_length: None,
        }
    }))
}

fn read_text(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn read_json<T>(relative_path: &str) -> T
where
    T: serde::de::DeserializeOwned,
{
    let contents = read_text(relative_path);
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {relative_path}: {error}"))
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
