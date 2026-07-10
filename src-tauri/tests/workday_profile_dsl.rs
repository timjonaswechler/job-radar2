use std::{collections::BTreeMap, fs, future::Future, path::Path, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, detect_source_proposal, execute_posting_detail_with_fetcher,
    execute_posting_discovery_with_fetcher, HttpMethod, PostingDetailFetchError,
    PostingDetailFetchRequest, PostingDetailFetchResponse, PostingDetailFetcher,
    PostingDetailPostingOccurrence, PostingDiscoveryCandidate, PostingDiscoveryFetchError,
    PostingDiscoveryFetchRequest, PostingDiscoveryFetchResponse, PostingDiscoveryFetcher,
    ProfileCompilerSnapshot, RequestBody, SourceDocument, SourceProfileDocument,
    SourceProposalDetectionStatus, SupportLevel,
};
use serde_json::{json, Value};

#[test]
fn workday_builtin_profile_compiles_and_executes_cxs_offline_fixtures() {
    let profile_text = read_text("resources/profiles/workday.json");
    assert_no_v1_profile_vocabulary(&profile_text);

    let profile_value: Value = serde_json::from_str(&profile_text).unwrap();
    assert_detects_named_workday_source_config_captures(&profile_value);
    let profile: SourceProfileDocument = serde_json::from_value(profile_value)
        .expect("Workday built-in profile should be a Source Profile DSL document");
    assert_eq!(profile.schema_version, 2);
    assert_eq!(profile.support.level, SupportLevel::Stable);

    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "acme_robotics_workday",
        "name": "Acme Robotics",
        "status": "active",
        "sourceConfig": {
            "workdayHost": "acme.wd3.myworkdayjobs.com",
            "tenant": "acme",
            "site": "External",
            "startUrl": "https://acme.wd3.myworkdayjobs.com/en-US/External"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "workday",
            "pathKey": "cxs_api"
        }
    }))
    .unwrap();

    let compile_result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "acme_robotics_workday",
    );
    assert_eq!(compile_result.diagnostics, Vec::new());
    let plan = compile_result
        .execution_plan
        .expect("Workday fixture source should compile");

    let discovery_url = "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/jobs";
    let fetcher = OfflineCxsFetcher::new(
        [
            (
                FetchKey::post_json(
                    discovery_url,
                    json!({ "appliedFacets": {}, "limit": 20, "offset": 0 }),
                ),
                read_text("tests/fixtures/workday/posting-discovery-page-0-response.json"),
            ),
            (
                FetchKey::post_json(
                    discovery_url,
                    json!({ "appliedFacets": {}, "limit": 20, "offset": 20 }),
                ),
                read_text("tests/fixtures/workday/posting-discovery-page-20-response.json"),
            ),
            (
                FetchKey::get("https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/job/Germany-Berlin/Senior-Platform-Engineer_JR-1001"),
                read_text("tests/fixtures/workday/posting-detail-jr-1001-response.json"),
            ),
        ],
    );

    let discovery = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<PostingDiscoveryCandidate> =
        read_json("tests/fixtures/workday/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);

    let requests = fetcher.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].url, discovery_url);
    assert_eq!(requests[0].timeout_ms, 10000);
    assert_eq!(
        requests[0].headers,
        BTreeMap::from_iter([
            ("accept".to_string(), "application/json".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ])
    );
    assert_eq!(
        requests[0].body,
        Some(RequestBody::Json {
            value: serde_json::Map::from_iter([
                ("appliedFacets".to_string(), json!({})),
                ("limit".to_string(), json!(20)),
                ("offset".to_string(), json!(0)),
            ])
        })
    );
    assert_eq!(
        requests[1].body,
        Some(RequestBody::Json {
            value: serde_json::Map::from_iter([
                ("appliedFacets".to_string(), json!({})),
                ("limit".to_string(), json!(20)),
                ("offset".to_string(), json!(20)),
            ])
        })
    );

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
        read_json("tests/fixtures/workday/posting-detail-jr-1001-expected.json");
    assert_eq!(
        detail.description_text.as_deref(),
        expected_detail["descriptionText"].as_str()
    );

    let requests = fetcher.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].method, HttpMethod::Get);
    assert_eq!(
        requests[2].url,
        "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/job/Germany-Berlin/Senior-Platform-Engineer_JR-1001"
    );
    assert_eq!(requests[2].body, None);
}

#[test]
fn workday_offset_limit_pagination_retains_the_initial_total_when_followup_total_is_zero() {
    let mut profile_value: Value =
        serde_json::from_str(&read_text("resources/profiles/workday.json")).unwrap();
    profile_value["accessPaths"][0]["postingDiscovery"]["strategies"][0]["pagination"]["limits"]
        ["maxRequests"] = json!(2);
    let profile: SourceProfileDocument = serde_json::from_value(profile_value).unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "acme_robotics_workday",
        "name": "Acme Robotics",
        "status": "active",
        "sourceConfig": {
            "workdayHost": "acme.wd3.myworkdayjobs.com",
            "tenant": "acme",
            "site": "External"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "workday",
            "pathKey": "cxs_api"
        }
    }))
    .unwrap();
    let compile_result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "acme_robotics_workday",
    );
    assert!(compile_result.diagnostics.is_empty());
    let plan = compile_result.execution_plan.unwrap();

    let discovery_url = "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/jobs";
    let fetcher = OfflineCxsFetcher::new([
        (
            FetchKey::post_json(
                discovery_url,
                json!({ "appliedFacets": {}, "limit": 20, "offset": 0 }),
            ),
            json!({
                "total": 373,
                "jobPostings": [
                    { "title": "Job 1", "externalPath": "/job/1" },
                    { "title": "Job 2", "externalPath": "/job/2" }
                ]
            })
            .to_string(),
        ),
        (
            FetchKey::post_json(
                discovery_url,
                json!({ "appliedFacets": {}, "limit": 20, "offset": 20 }),
            ),
            json!({
                "total": 0,
                "jobPostings": [
                    { "title": "Job 3", "externalPath": "/job/3" },
                    { "title": "Job 4", "externalPath": "/job/4" }
                ]
            })
            .to_string(),
        ),
    ]);

    let discovery = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(discovery.candidates.len(), 4);
    assert_eq!(fetcher.requests().len(), 2);
    assert_eq!(discovery.diagnostics.len(), 1);
    assert_eq!(
        discovery.diagnostics[0].code,
        "pagination_max_requests_reached"
    );
    assert_eq!(
        discovery.diagnostics[0].path,
        "/postingDiscovery/strategies/0/pagination/limits/maxRequests"
    );
    assert_eq!(
        discovery.diagnostics[0].strategy_key.as_deref(),
        Some("cxs_jobs_api")
    );
}

#[test]
fn workday_builtin_detection_proposes_source_config_and_recommended_profile_path() {
    let profile_text = read_text("resources/profiles/workday.json");
    let profile: SourceProfileDocument = serde_json::from_str(&profile_text)
        .expect("Workday built-in profile should be a Source Profile DSL document");

    let input_url = "https://acme.wd3.myworkdayjobs.com/en-US/External?source=job-radar";
    let result = block_on(detect_source_proposal(input_url, &[profile]));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let proposal = result.proposal.expect("Workday detection should match");
    assert_eq!(proposal.profile_key, "workday");
    assert_eq!(proposal.profile_name, "Workday Recruiting");
    assert_eq!(proposal.recommended_access_path_key, "cxs_api");
    assert_eq!(proposal.recommended_access_path_name, "Workday CXS API");
    assert_eq!(
        proposal.source_config,
        json!({
            "workdayHost": "acme.wd3.myworkdayjobs.com",
            "tenant": "acme",
            "site": "External",
            "startUrl": input_url
        })
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
            "Workday DSL profile must not contain v1 vocabulary `{forbidden}`"
        );
    }
}

fn assert_detects_named_workday_source_config_captures(profile: &Value) {
    let captures = profile["detect"]["inputUrlPatterns"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|pattern| pattern["captures"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    for expected in ["workdayHost", "tenant", "site"] {
        assert!(
            captures.contains(&expected),
            "Workday profile should declare detection capture `{expected}`"
        );
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FetchKey {
    method: String,
    url: String,
    body: Option<String>,
}

impl FetchKey {
    fn get(url: impl Into<String>) -> Self {
        Self {
            method: "GET".to_string(),
            url: url.into(),
            body: None,
        }
    }

    fn post_json(url: impl Into<String>, body: Value) -> Self {
        Self {
            method: "POST".to_string(),
            url: url.into(),
            body: Some(serde_json::to_string(&body).unwrap()),
        }
    }

    fn from_discovery_request(request: &PostingDiscoveryFetchRequest) -> Self {
        Self {
            method: http_method_key(request.method),
            url: request.url.clone(),
            body: request_body_key(request.body.as_ref()),
        }
    }

    fn from_detail_request(request: &PostingDetailFetchRequest) -> Self {
        Self {
            method: http_method_key(request.method),
            url: request.url.clone(),
            body: request_body_key(request.body.as_ref()),
        }
    }
}

#[derive(Default)]
struct OfflineCxsFetcher {
    responses: BTreeMap<FetchKey, String>,
    requests: std::sync::Mutex<Vec<UnifiedRequest>>,
}

impl OfflineCxsFetcher {
    fn new(responses: impl IntoIterator<Item = (FetchKey, String)>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<UnifiedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UnifiedRequest {
    method: HttpMethod,
    url: String,
    headers: BTreeMap<String, String>,
    body: Option<RequestBody>,
    timeout_ms: u64,
}

impl From<PostingDiscoveryFetchRequest> for UnifiedRequest {
    fn from(request: PostingDiscoveryFetchRequest) -> Self {
        Self {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout_ms: request.timeout_ms,
        }
    }
}

impl From<PostingDetailFetchRequest> for UnifiedRequest {
    fn from(request: PostingDetailFetchRequest) -> Self {
        Self {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout_ms: request.timeout_ms,
        }
    }
}

impl PostingDiscoveryFetcher for OfflineCxsFetcher {
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
            let key = FetchKey::from_discovery_request(&request);
            self.requests.lock().unwrap().push(request.into());
            let body = self.responses.get(&key).cloned().ok_or_else(|| {
                PostingDiscoveryFetchError::new(format!(
                    "missing offline discovery fixture for {:?}",
                    key
                ))
            })?;
            Ok(PostingDiscoveryFetchResponse { body })
        })
    }
}

impl PostingDetailFetcher for OfflineCxsFetcher {
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
            let key = FetchKey::from_detail_request(&request);
            self.requests.lock().unwrap().push(request.into());
            let body = self.responses.get(&key).cloned().ok_or_else(|| {
                PostingDetailFetchError::new(format!(
                    "missing offline detail fixture for {:?}",
                    key
                ))
            })?;
            Ok(PostingDetailFetchResponse { body })
        })
    }
}

fn http_method_key(method: HttpMethod) -> String {
    match method {
        HttpMethod::Get => "GET".to_string(),
        HttpMethod::Post => "POST".to_string(),
    }
}

fn request_body_key(body: Option<&RequestBody>) -> Option<String> {
    match body {
        Some(RequestBody::Json { value }) => {
            Some(serde_json::to_string(&Value::Object(value.clone())).unwrap())
        }
        Some(RequestBody::Text { value }) => Some(value.clone()),
        Some(RequestBody::Form { fields }) => Some(serde_json::to_string(fields).unwrap()),
        None => None,
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
