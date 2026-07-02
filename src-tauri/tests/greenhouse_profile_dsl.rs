use std::{collections::BTreeMap, fs, future::Future, path::Path, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, execute_posting_detail_with_fetcher,
    execute_posting_discovery_with_fetcher, PostingDetailFetchError, PostingDetailFetchRequest,
    PostingDetailFetchResponse, PostingDetailFetcher, PostingDetailPostingOccurrence,
    PostingDiscoveryCandidate, PostingDiscoveryFetchError, PostingDiscoveryFetchRequest,
    PostingDiscoveryFetchResponse, PostingDiscoveryFetcher, ProfileCompilerSnapshot,
    SourceDocument, SourceProfileDocument,
};
use serde_json::{json, Value};

#[test]
fn greenhouse_builtin_profile_compiles_and_executes_offline_fixtures() {
    let profile_text = read_text("resources/profiles/greenhouse.json");
    assert_no_v1_profile_vocabulary(&profile_text);

    let profile: SourceProfileDocument = serde_json::from_str(&profile_text)
        .expect("Greenhouse built-in profile should be a Source Profile DSL document");
    assert_eq!(profile.schema_version, 2);
    assert_eq!(profile.support.level, job_radar_lib::SupportLevel::Verified);

    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "acme_robotics",
        "name": "Acme Robotics",
        "status": "active",
        "sourceConfig": {
            "boardSlug": "acmejobs"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "greenhouse",
            "pathKey": "boards_api"
        }
    }))
    .unwrap();

    let compile_result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "acme_robotics",
    );
    assert_eq!(compile_result.diagnostics, Vec::new());
    let plan = compile_result
        .execution_plan
        .expect("Greenhouse fixture source should compile");

    let fetcher = OfflineFetcher::new([
        (
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs",
            read_text("tests/fixtures/greenhouse/posting-discovery-response.json"),
        ),
        (
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs/9001",
            read_text("tests/fixtures/greenhouse/posting-detail-9001-response.json"),
        ),
    ]);

    let discovery = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<PostingDiscoveryCandidate> =
        read_json("tests/fixtures/greenhouse/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);

    let first_candidate = discovery.candidates.first().unwrap();
    let detail = block_on(execute_posting_detail_with_fetcher(
        &plan,
        &PostingDetailPostingOccurrence {
            url: first_candidate.url.clone(),
            title: Some(first_candidate.title.clone()),
            company: Some(first_candidate.company.clone()),
            locations: first_candidate.locations.clone(),
            description_text: first_candidate.description_text.clone(),
            posting_meta: first_candidate.posting_meta.clone(),
        },
        &fetcher,
    ));
    assert_eq!(detail.diagnostics, Vec::new());
    let expected_detail: Value =
        read_json("tests/fixtures/greenhouse/posting-detail-9001-expected.json");
    assert_eq!(
        detail.description_text.as_deref(),
        expected_detail["descriptionText"].as_str()
    );

    assert_eq!(
        fetcher.requested_urls(),
        vec![
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs".to_string(),
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs/9001".to_string(),
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
            "Greenhouse DSL profile must not contain v1 vocabulary `{forbidden}`"
        );
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

impl PostingDiscoveryFetcher for OfflineFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requested_urls
                .lock()
                .unwrap()
                .push(request.url.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                PostingDiscoveryFetchError::new(format!(
                    "missing offline discovery fixture for {}",
                    request.url
                ))
            })?;
            Ok(PostingDiscoveryFetchResponse { body })
        })
    }
}

impl PostingDetailFetcher for OfflineFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDetailFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDetailFetchResponse, PostingDetailFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requested_urls
                .lock()
                .unwrap()
                .push(request.url.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                PostingDetailFetchError::new(format!(
                    "missing offline detail fixture for {}",
                    request.url
                ))
            })?;
            Ok(PostingDetailFetchResponse { body })
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
