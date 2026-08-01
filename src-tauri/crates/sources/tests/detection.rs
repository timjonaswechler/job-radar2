use std::{fs, path::Path, sync::Arc, time::Duration};

use serde_json::{json, Value};
use source_engine::{
    definition::DiagnosticCategory,
    detection::DetectionRunStatus,
    execution::RuntimeCancellation,
    test_support::{
        ScriptedBrowserAcquisition, ScriptedHttpBodyEvent, ScriptedHttpEvent,
        ScriptedProfileHttpClient,
    },
};
use sources::{
    detection::{Context, Operation, Request},
    installed::Store,
};

fn write_profile(app_data_dir: &Path, profile: &Value) {
    let directory = app_data_dir.join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    let key = profile["key"].as_str().unwrap();
    fs::write(
        directory.join(format!("{key}.json")),
        serde_json::to_vec_pretty(profile).unwrap(),
    )
    .unwrap();
}

fn detection_profile(key: &str, support_level: &str, strategies: Value) -> Value {
    let mut profile: Value =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    profile["key"] = json!(key);
    profile["name"] = json!(key);
    profile["support"]["level"] = json!(support_level);
    profile["accessPaths"].as_array_mut().unwrap().truncate(1);
    profile["detection"] = json!({
        "recommendedAccessPathKey": "boards_api",
        "policy": { "type": "all_required" },
        "strategies": strategies,
        "sourceConfig": { "boardSlug": "fixture" }
    });
    profile
}

fn operation(app_data_dir: &Path) -> Operation {
    operation_with_client(app_data_dir, Arc::new(ScriptedProfileHttpClient::new([])))
}

fn operation_with_client(app_data_dir: &Path, client: Arc<ScriptedProfileHttpClient>) -> Operation {
    Operation::new(
        Store::new(app_data_dir),
        client,
        Arc::new(ScriptedBrowserAcquisition::new([])),
    )
}

struct Cancelled;

impl RuntimeCancellation for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn admitted_profile_detection_returns_a_proposal_without_persisting_a_source() {
    let app_data = tempfile::tempdir().unwrap();
    write_profile(
        app_data.path(),
        &detection_profile(
            "matched_fixture",
            "stable",
            json!([{ "type": "url", "key": "url", "input": { "type": "absolute_url" } }]),
        ),
    );

    let outcome = operation(app_data.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, DetectionRunStatus::Matched);
    assert_eq!(outcome.proposals.len(), 1);
    assert_eq!(outcome.proposals[0].profile_key, "matched_fixture");
    let transport = serde_json::to_value(&outcome).unwrap();
    assert!(transport.get("profileDiagnostics").is_some());
    assert!(transport.get("report").is_some());
    assert!(transport.get("attempts").is_none());
    assert!(transport.get("profileOutcomes").is_none());
    assert!(!app_data.path().join("sources").exists());
}

#[tokio::test]
async fn rejected_custom_profile_is_diagnosed_without_poisoning_admitted_detection() {
    let app_data = tempfile::tempdir().unwrap();
    write_profile(
        app_data.path(),
        &detection_profile(
            "matched_fixture",
            "stable",
            json!([{ "type": "url", "key": "url", "input": { "type": "absolute_url" } }]),
        ),
    );
    let mut rejected = detection_profile(
        "broken_fixture",
        "stable",
        json!([{ "type": "url", "key": "url", "input": {
            "type": "pattern_alternatives",
            "alternatives": [{ "pattern": "(" }]
        } }]),
    );
    rejected["name"] = json!("Broken fixture");
    write_profile(app_data.path(), &rejected);

    let outcome = operation(app_data.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, DetectionRunStatus::Matched);
    assert_eq!(outcome.proposals.len(), 1);
    assert_eq!(outcome.proposals[0].profile_key, "matched_fixture");
    assert!(outcome
        .profile_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.category == DiagnosticCategory::Compiler));
}

#[tokio::test]
async fn one_snapshot_supplies_the_complete_prepared_detection_set() {
    let app_data = tempfile::tempdir().unwrap();
    write_profile(
        app_data.path(),
        &detection_profile(
            "initial_fixture",
            "stable",
            json!([
                { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
                { "type": "http", "key": "probe", "fetch": {
                    "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
                }}
            ]),
        ),
    );
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Gate("snapshot_loaded".to_string())],
            content_length: None,
        },
    ]));
    let operation = operation_with_client(app_data.path(), Arc::clone(&client));
    let running = tokio::spawn(async move {
        operation
            .run(
                Request {
                    url: "https://example.test/jobs".to_string(),
                },
                Context::default(),
            )
            .await
            .unwrap()
    });

    tokio::time::timeout(Duration::from_secs(1), async {
        while !client.gate_is_waiting("snapshot_loaded") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Detection should reach HTTP after loading its Snapshot");
    write_profile(
        app_data.path(),
        &detection_profile(
            "late_fixture",
            "stable",
            json!([{ "type": "url", "key": "url", "input": { "type": "absolute_url" } }]),
        ),
    );
    assert!(client.release_gate("snapshot_loaded"));

    let outcome = running.await.unwrap();
    assert_eq!(outcome.status, DetectionRunStatus::Matched);
    assert!(outcome
        .proposals
        .iter()
        .any(|proposal| proposal.profile_key == "initial_fixture"));
    assert!(outcome
        .proposals
        .iter()
        .all(|proposal| proposal.profile_key != "late_fixture"));
}

#[tokio::test]
async fn terminal_outcomes_and_bounded_usage_remain_distinct() {
    let absolute = json!([
        { "type": "url", "key": "url", "input": { "type": "absolute_url" } }
    ]);

    let ambiguous_dir = tempfile::tempdir().unwrap();
    write_profile(
        ambiguous_dir.path(),
        &detection_profile("first_fixture", "stable", absolute.clone()),
    );
    write_profile(
        ambiguous_dir.path(),
        &detection_profile("second_fixture", "stable", absolute.clone()),
    );
    let ambiguous = operation(ambiguous_dir.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();
    assert_eq!(ambiguous.status, DetectionRunStatus::Ambiguous);
    assert_eq!(ambiguous.proposals.len(), 2);

    let unsupported_dir = tempfile::tempdir().unwrap();
    write_profile(
        unsupported_dir.path(),
        &detection_profile("unsupported_fixture", "unsupported", absolute.clone()),
    );
    let unsupported_among_failed = operation(unsupported_dir.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        unsupported_among_failed.status,
        DetectionRunStatus::Unsupported
    );
    assert_eq!(unsupported_among_failed.unsupported_profiles.len(), 1);

    let mixed_dir = tempfile::tempdir().unwrap();
    write_profile(
        mixed_dir.path(),
        &detection_profile("unsupported_fixture", "unsupported", absolute),
    );
    write_profile(
        mixed_dir.path(),
        &detection_profile(
            "failed_fixture",
            "stable",
            json!([
                { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
                { "type": "http", "key": "probe", "fetch": {
                    "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
                }}
            ]),
        ),
    );
    let mixed = operation(mixed_dir.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();
    assert_eq!(mixed.status, DetectionRunStatus::Failed);
    assert!(!mixed.diagnostics.is_empty());

    let failed_dir = tempfile::tempdir().unwrap();
    let client = Arc::new(ScriptedProfileHttpClient::new([]));
    let failed = operation_with_client(failed_dir.path(), Arc::clone(&client))
        .run(
            Request {
                url: "relative".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();
    assert_eq!(failed.status, DetectionRunStatus::Failed);
    assert_eq!(client.request_count(), 0);
    assert_eq!(failed.report.usage.requests, 0);

    let cancelled_dir = tempfile::tempdir().unwrap();
    let cancellation = Cancelled;
    let cancelled = operation(cancelled_dir.path())
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context {
                cancellation: Some(&cancellation),
            },
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, DetectionRunStatus::Cancelled);

    let exhausted_dir = tempfile::tempdir().unwrap();
    write_profile(
        exhausted_dir.path(),
        &detection_profile(
            "budget_fixture",
            "stable",
            json!([
                { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
                { "type": "http", "key": "probe", "fetch": {
                    "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
                }}
            ]),
        ),
    );
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            content_length: Some(67_108_865),
        },
    ]));
    let exhausted = operation_with_client(exhausted_dir.path(), client)
        .run(
            Request {
                url: "https://example.test/jobs".to_string(),
            },
            Context::default(),
        )
        .await
        .unwrap();
    assert_eq!(exhausted.status, DetectionRunStatus::BudgetExhausted);
    assert_eq!(exhausted.report.usage.requests, 0);
    assert_eq!(exhausted.report.usage.response_bytes, 0);
}
