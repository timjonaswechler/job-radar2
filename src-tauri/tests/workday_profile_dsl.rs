mod support;

use support::{compile_test_source, execute_detail_test, execute_discovery_test, unwrap_plan};

use std::{collections::BTreeMap, fs, future::Future, path::Path};

use job_radar_lib::{
    detect_source_proposal, DetailPostingOccurrence, DiscoveryCandidate, HttpMethod,
    ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient, SourceDocument,
    SourceProfileDocument, SourceProposalDetectionStatus, SupportLevel,
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
    assert_eq!(profile.schema_version, 3);
    assert_eq!(profile.support.level, SupportLevel::Stable);

    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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

    let compile_result = compile_test_source(&source, Some(profile));
    let plan = unwrap_plan(compile_result);

    let discovery_url = "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/jobs";
    let fetcher = offline_cxs_fetcher(
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

    let discovery = block_on(execute_discovery_test(&plan, &fetcher));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<DiscoveryCandidate> =
        read_json("tests/fixtures/workday/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);

    let requests = fetcher.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].url, discovery_url);
    assert_eq!(requests[0].timeout_ms, 10000);
    assert_eq!(
        requests[0]
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), String::from_utf8(value.clone()).unwrap()))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from_iter([
            ("accept".to_string(), "application/json".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ])
    );
    assert_eq!(
        requests[0].body.as_ref().map(|body| body.bytes()),
        Some(br#"{"appliedFacets":{},"limit":20,"offset":0}"#.as_slice())
    );
    assert_eq!(
        requests[1].body.as_ref().map(|body| body.bytes()),
        Some(br#"{"appliedFacets":{},"limit":20,"offset":20}"#.as_slice())
    );

    let first_candidate = discovery.candidates.first().unwrap();
    let detail = block_on(execute_detail_test(
        &plan,
        &DetailPostingOccurrence {
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
    assert!(requests[2].body.is_none());
}

#[test]
fn workday_offset_limit_pagination_retains_the_initial_total_when_followup_total_is_zero() {
    let mut profile_value: Value =
        serde_json::from_str(&read_text("resources/profiles/workday.json")).unwrap();
    profile_value["accessPaths"][0]["discovery"]["strategies"][0]["pagination"]["limits"]
        ["maxRequests"] = json!(2);
    let profile: SourceProfileDocument = serde_json::from_value(profile_value).unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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
    let plan = unwrap_plan(compile_test_source(&source, Some(profile)));

    let discovery_url = "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/jobs";
    let fetcher = offline_cxs_fetcher([
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

    let discovery = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(discovery.candidates.len(), 4);
    assert_eq!(fetcher.requests().len(), 2);
    assert_eq!(discovery.diagnostics.len(), 1);
    assert_eq!(
        discovery.diagnostics[0].code,
        "pagination_max_requests_reached"
    );
    assert_eq!(
        discovery.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxRequests"
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
    let captures = profile["detection"]["inputUrlPatterns"]
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
}

fn offline_cxs_fetcher(
    responses: impl IntoIterator<Item = (FetchKey, String)>,
) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new(responses.into_iter().map(|(key, body)| {
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: key.url,
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
