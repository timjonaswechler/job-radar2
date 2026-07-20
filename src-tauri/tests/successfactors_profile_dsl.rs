mod support;

use support::{compile_test_source, unwrap_plan};

use std::{collections::BTreeMap, fs, future::Future, path::Path, pin::Pin};

use job_radar_lib::{
    execute_detail_with_fetcher, execute_discovery_with_fetcher, DetailFetchError,
    DetailFetchRequest, DetailFetchResponse, DetailFetcher, DetailPostingOccurrence,
    DiagnosticCategory, DiagnosticSeverity, DiscoveryCandidate, DiscoveryFetchError,
    DiscoveryFetchRequest, DiscoveryFetchResponse, DiscoveryFetcher, SourceDocument,
    SourceProfileDocument, SupportLevel,
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

    let fetcher = OfflineFetcher::new([
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
            "https://jobs.example-successfactors.test/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/",
            read_text("tests/fixtures/successfactors/posting-detail-1405371733-schott.html"),
        ),
    ]);

    let discovery = block_on(execute_discovery_with_fetcher(&plan, &fetcher));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<DiscoveryCandidate> =
        read_json("tests/fixtures/successfactors/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);

    let primary_candidate = &discovery.candidates[0];
    let primary_detail = block_on(execute_detail_with_fetcher(
        &plan,
        &posting_occurrence(primary_candidate),
        &fetcher,
    ));
    assert_eq!(primary_detail.diagnostics, Vec::new());
    let expected_primary_detail: Value =
        read_json("tests/fixtures/successfactors/posting-detail-1001-expected.json");
    assert_eq!(
        primary_detail.description_text.as_deref(),
        expected_primary_detail["descriptionText"].as_str()
    );

    let fallback_candidate = &discovery.candidates[1];
    let fallback_detail = block_on(execute_detail_with_fetcher(
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
    let schott_style_detail = block_on(execute_detail_with_fetcher(
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
        fetcher.requested_urls(),
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

#[derive(Default)]
struct OfflineFetcher {
    responses: BTreeMap<String, String>,
    requested_urls: std::sync::Mutex<Vec<String>>,
}

impl OfflineFetcher {
    fn new(responses: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body))
                .collect(),
            requested_urls: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls.lock().unwrap().clone()
    }
}

impl DiscoveryFetcher for OfflineFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.requested_urls
                .lock()
                .unwrap()
                .push(request.url.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                DiscoveryFetchError::new(format!(
                    "missing offline discovery fixture for {}",
                    request.url
                ))
            })?;
            Ok(DiscoveryFetchResponse { body })
        })
    }
}

impl DetailFetcher for OfflineFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.requested_urls
                .lock()
                .unwrap()
                .push(request.url.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                DetailFetchError::new(format!(
                    "missing offline detail fixture for {}",
                    request.url
                ))
            })?;
            Ok(DetailFetchResponse { body })
        })
    }
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
