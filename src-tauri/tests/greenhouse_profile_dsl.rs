mod support;

use support::{
    compile_test_source, execute_detail_test_with_config, execute_discovery_test_with_config,
    unwrap_plan,
};

use std::{fs, future::Future, path::Path};

use job_radar_lib::{
    PostingOccurrence, ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
    SourceDocument, SourceProfileDocument,
};
use serde_json::{json, Value};

#[test]
fn greenhouse_builtin_profile_compiles_and_executes_offline_fixtures() {
    let profile_text = read_text("resources/profiles/greenhouse.json");
    assert_no_v1_profile_vocabulary(&profile_text);

    let profile: SourceProfileDocument = serde_json::from_str(&profile_text)
        .expect("Greenhouse built-in profile should be a Source Profile DSL document");
    assert_eq!(profile.schema_version, 3);
    assert_eq!(profile.support.level, job_radar_lib::SupportLevel::Stable);

    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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

    let compile_result = compile_test_source(&source, Some(profile));
    let plan = unwrap_plan(compile_result);

    let fetcher = offline_fetcher([
        (
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs",
            read_text("tests/fixtures/greenhouse/posting-discovery-response.json"),
        ),
        (
            "https://boards-api.greenhouse.io/v1/boards/acmejobs/jobs/9001",
            read_text("tests/fixtures/greenhouse/posting-detail-9001-response.json"),
        ),
    ]);

    let discovery = block_on(execute_discovery_test_with_config(
        &plan,
        &source.source_config,
        &fetcher,
    ));
    assert_eq!(discovery.diagnostics, Vec::new());
    let expected_candidates: Vec<PostingOccurrence> =
        read_json("tests/fixtures/greenhouse/posting-discovery-expected-candidates.json");
    assert_eq!(discovery.candidates, expected_candidates);

    let first_candidate = discovery.candidates.first().unwrap();
    let detail = block_on(execute_detail_test_with_config(
        &plan,
        &source.source_config,
        first_candidate,
        &fetcher,
    ));
    assert_eq!(detail.diagnostics, Vec::new());
    let expected_detail: Value =
        read_json("tests/fixtures/greenhouse/posting-detail-9001-expected.json");
    assert_eq!(
        detail.patch.description_text.as_deref(),
        expected_detail["descriptionText"].as_str()
    );

    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
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
