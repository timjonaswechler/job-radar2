use job_postings::{
    catalog::ReadState,
    detail::{Description, Error as DetailError},
    Catalog, Detail, Id,
};
use source_engine::test_support::{
    BrowserAcquisitionRequestSnapshot, ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient,
};
use std::{collections::BTreeMap, sync::Arc};

use super::support::{
    detail_profile, detail_profile_url, insert_posting, insert_source, installed, no_browser,
    no_http, pool, profile_without_detail, source_document, source_document_config, Posting,
};

async fn posting(pool: &sqlx::SqlitePool, title: &str) -> (i64, String) {
    let (id, _) = insert_posting(
        pool,
        Posting {
            title,
            read: "unread",
            interest: "undecided",
            preparation: "not_started",
            application: "not_applied",
            last_seen: "2026-06-01T00:00:00.000Z",
        },
    )
    .await;
    (id, format!("source_{}", title.to_ascii_lowercase()))
}

fn http_response(url: &str, body: &str) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: url.to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(body.as_bytes().to_vec())],
        content_length: None,
    }
}

#[tokio::test]
async fn cached_open_marks_read_without_loading_installed_sources_or_external_work() {
    let pool = pool().await;
    let (id, _) = posting(&pool, "Cached").await;
    sqlx::query("UPDATE job_postings SET description_text = 'Stored' WHERE id = ?1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let invalid_root = tempfile::NamedTempFile::new().unwrap();
    let http = no_http();
    let browser = no_browser();
    let detail = Detail::new(
        Catalog::new(pool),
        sources::installed::Store::new(invalid_root.path()),
        http.clone(),
        browser.clone(),
    );

    let opened = detail.open(Id::new(id)).await.unwrap();

    assert_eq!(opened.posting.read_state, ReadState::Read);
    assert_eq!(
        opened.description_state,
        Description::Loaded {
            text: "Stored".to_string(),
            diagnostics: Vec::new()
        }
    );
    assert!(http.requests().is_empty());
    assert!(browser.requests().is_empty());
}

#[tokio::test]
async fn open_loads_http_description_and_persists_first_success() {
    let pool = pool().await;
    let (id, source_key) = posting(&pool, "Loaded").await;
    let url = "https://example.test/jobs/loaded";
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[source_document(&source_key, "profile", "path")],
    );
    let http = Arc::new(ScriptedProfileHttpClient::new([http_response(
        url,
        "<div class=\"description\">Loaded description</div>",
    )]));
    let detail = Detail::new(
        Catalog::new(pool.clone()),
        installed.store,
        http.clone(),
        no_browser(),
    );

    let opened = detail.open(Id::new(id)).await.unwrap();

    assert_eq!(opened.posting.read_state, ReadState::Read);
    assert_eq!(
        opened.posting.description_text.as_deref(),
        Some("Loaded description")
    );
    assert_eq!(http.requests().len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT description_text FROM job_postings WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "Loaded description"
    );
}

#[tokio::test]
async fn open_reconstructs_provider_identity_url_and_posting_meta_context() {
    let pool = pool().await;
    let (id, source_key) = posting(&pool, "Context").await;
    sqlx::query(
        "UPDATE job_posting_sources
         SET identity_kind = 'provider_posting_id', identity_value = 'Provider-42',
             posting_meta_json = '{\"jobId\":\"meta-42\"}'
         WHERE posting_id = ?1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let installed = installed(
        &[detail_profile_url(
            "profile",
            "path",
            "http",
            "{{posting:url}}?job={{postingMeta:jobId}}",
        )],
        &[source_document(&source_key, "profile", "path")],
    );
    let expected_url = "https://example.test/jobs/context?job=meta-42";
    let http = Arc::new(ScriptedProfileHttpClient::new([http_response(
        expected_url,
        "<div class=\"description\">Context loaded</div>",
    )]));
    let opened = Detail::new(
        Catalog::new(pool),
        installed.store,
        http.clone(),
        no_browser(),
    )
    .open(Id::new(id))
    .await
    .unwrap();

    assert!(
        matches!(opened.description_state, Description::Loaded { ref text, .. } if text == "Context loaded")
    );
    assert_eq!(http.requests()[0].url, expected_url);
}

#[tokio::test]
async fn open_uses_immutable_primary_first_then_falls_back_with_contextual_diagnostics() {
    let pool = pool().await;
    let (id, primary_key) = posting(&pool, "Fallback").await;
    let fallback_key = "fallback_source";
    let fallback_url = "https://fallback.test/jobs/1";
    insert_source(
        &pool,
        id,
        fallback_key,
        fallback_url,
        false,
        BTreeMap::new(),
    )
    .await;
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[
            source_document(&primary_key, "profile", "path"),
            source_document(fallback_key, "profile", "path"),
        ],
    );
    let primary_url = "https://example.test/jobs/fallback";
    let http = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 500,
            final_url: primary_url.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            content_length: None,
        },
        http_response(
            fallback_url,
            "<div class=\"description\">Fallback description</div>",
        ),
    ]));
    let detail = Detail::new(
        Catalog::new(pool),
        installed.store,
        http.clone(),
        no_browser(),
    );

    let opened = detail.open(Id::new(id)).await.unwrap();

    assert_eq!(
        http.requests()
            .iter()
            .map(|request| request.url.as_str())
            .collect::<Vec<_>>(),
        vec![primary_url, fallback_url]
    );
    match opened.description_state {
        Description::Loaded { text, diagnostics } => {
            assert_eq!(text, "Fallback description");
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.details.as_ref().unwrap()["postingSourceKey"] == primary_key
            }));
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[tokio::test]
async fn malformed_primary_occurrence_is_diagnostic_and_fallback_remains_usable() {
    let pool = pool().await;
    let (id, primary_key) = posting(&pool, "Malformed").await;
    let fallback_key = "valid_fallback";
    let fallback_url = "https://fallback.test/jobs/valid";
    insert_source(
        &pool,
        id,
        fallback_key,
        fallback_url,
        false,
        BTreeMap::new(),
    )
    .await;
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE job_posting_sources SET identity_kind = 'broken' WHERE posting_id = ?1 AND is_primary = 1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&pool)
        .await
        .unwrap();
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[
            source_document(&primary_key, "profile", "path"),
            source_document(fallback_key, "profile", "path"),
        ],
    );
    let http = Arc::new(ScriptedProfileHttpClient::new([http_response(
        fallback_url,
        "<div class=\"description\">Recovered</div>",
    )]));
    let detail = Detail::new(
        Catalog::new(pool),
        installed.store,
        http.clone(),
        no_browser(),
    );

    let opened = detail.open(Id::new(id)).await.unwrap();

    match opened.description_state {
        Description::Loaded { text, diagnostics } => {
            assert_eq!(text, "Recovered");
            assert!(diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "posting_occurrence_invalid"));
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn unsupported_and_failed_outcomes_still_mark_read() {
    let pool = pool().await;
    let (unsupported_id, _) = posting(&pool, "Unsupported").await;
    insert_source(
        &pool,
        unsupported_id,
        "invalid_source",
        "https://invalid.test/jobs/1",
        false,
        BTreeMap::new(),
    )
    .await;
    insert_source(
        &pool,
        unsupported_id,
        "list_source",
        "https://list.test/jobs/1",
        false,
        BTreeMap::new(),
    )
    .await;
    let unsupported_installed = installed(
        &[
            detail_profile("detail_profile", "path", "http"),
            profile_without_detail("list_profile", "path"),
        ],
        &[
            source_document_config(
                "invalid_source",
                "detail_profile",
                "path",
                serde_json::json!({ "unexpected": true }),
            ),
            source_document("list_source", "list_profile", "path"),
        ],
    );
    let unsupported = Detail::new(
        Catalog::new(pool.clone()),
        unsupported_installed.store,
        no_http(),
        no_browser(),
    )
    .open(Id::new(unsupported_id))
    .await
    .unwrap();
    match unsupported.description_state {
        Description::Unsupported { diagnostics, .. } => {
            for code in [
                "source_not_found",
                "unknown_source_config_property",
                "detail_missing",
            ] {
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic.code == code),
                    "missing {code}: {diagnostics:?}"
                );
            }
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
    assert_eq!(unsupported.posting.read_state, ReadState::Read);

    let (failed_id, failed_key) = posting(&pool, "Failed").await;
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[source_document(&failed_key, "profile", "path")],
    );
    let http = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 500,
            final_url: "https://example.test/jobs/failed".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            content_length: None,
        },
    ]));
    let failed = Detail::new(Catalog::new(pool), installed.store, http, no_browser())
        .open(Id::new(failed_id))
        .await
        .unwrap();
    assert!(matches!(
        failed.description_state,
        Description::Failed { .. }
    ));
    assert_eq!(failed.posting.read_state, ReadState::Read);
}

#[tokio::test]
async fn browser_detail_uses_injected_browser_adapter() {
    let pool = pool().await;
    let (id, source_key) = posting(&pool, "Browser").await;
    let url = "https://example.test/jobs/browser";
    let installed = installed(
        &[detail_profile("profile", "path", "browser")],
        &[source_document(&source_key, "profile", "path")],
    );
    let browser = Arc::new(ScriptedBrowserAcquisition::new([
        ScriptedBrowserAcquisitionExpectation {
            request: BrowserAcquisitionRequestSnapshot {
                target: url.to_string(),
                timeout_ms: 1000,
                waits: Vec::new(),
                interactions: Vec::new(),
                browser_rendered_bytes_remaining: 67_108_864,
            },
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Content(
                    "<div class=\"description\">Rendered</div>".to_string(),
                ),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        },
    ]));
    let detail = Detail::new(
        Catalog::new(pool),
        installed.store,
        no_http(),
        browser.clone(),
    );

    let opened = detail.open(Id::new(id)).await.unwrap();

    assert!(
        matches!(opened.description_state, Description::Loaded { ref text, .. } if text == "Rendered")
    );
    assert!(
        browser.expectations_satisfied(),
        "{:?}",
        browser.mismatches()
    );
}

#[tokio::test]
async fn concurrent_successes_keep_and_reload_the_first_cached_description() {
    let pool = pool().await;
    let (id, source_key) = posting(&pool, "Concurrent").await;
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[source_document(&source_key, "profile", "path")],
    );
    let client = |gate: &str, text: &str| {
        Arc::new(ScriptedProfileHttpClient::new([
            ScriptedHttpEvent::Response {
                status: 200,
                final_url: "https://example.test/jobs/concurrent".to_string(),
                headers: Vec::new(),
                body: vec![
                    ScriptedHttpBodyEvent::Gate(gate.to_string()),
                    ScriptedHttpBodyEvent::Chunk(
                        format!("<div class=\"description\">{text}</div>").into_bytes(),
                    ),
                ],
                content_length: None,
            },
        ]))
    };
    let first_http = client("first", "First contender");
    let second_http = client("second", "Second contender");
    let first = Detail::new(
        Catalog::new(pool.clone()),
        installed.store.clone(),
        first_http.clone(),
        no_browser(),
    );
    let second = Detail::new(
        Catalog::new(pool.clone()),
        installed.store,
        second_http.clone(),
        no_browser(),
    );
    let first_task = tokio::spawn(async move { first.open(Id::new(id)).await.unwrap() });
    let second_task = tokio::spawn(async move { second.open(Id::new(id)).await.unwrap() });
    while !first_http.gate_is_waiting("first") || !second_http.gate_is_waiting("second") {
        tokio::task::yield_now().await;
    }
    assert!(first_http.release_gate("first"));
    assert!(second_http.release_gate("second"));
    let first = first_task.await.unwrap();
    let second = second_task.await.unwrap();
    let persisted =
        sqlx::query_scalar::<_, String>("SELECT description_text FROM job_postings WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        first.posting.description_text.as_deref(),
        Some(persisted.as_str())
    );
    assert_eq!(
        second.posting.description_text.as_deref(),
        Some(persisted.as_str())
    );
    assert!(matches!(
        persisted.as_str(),
        "First contender" | "Second contender"
    ));
}

#[tokio::test]
async fn errors_distinguish_before_and_after_mark_read_effect() {
    let pool = pool().await;
    let invalid_root = tempfile::NamedTempFile::new().unwrap();
    let detail = Detail::new(
        Catalog::new(pool.clone()),
        sources::installed::Store::new(invalid_root.path()),
        no_http(),
        no_browser(),
    );
    assert!(matches!(
        detail.open(Id::new(999)).await.unwrap_err(),
        DetailError::BeforeRead(_)
    ));

    let (id, source_key) = posting(&pool, "AfterEffect").await;
    let installed = installed(
        &[detail_profile("profile", "path", "http")],
        &[source_document(&source_key, "profile", "path")],
    );
    let http = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs/aftereffect".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Gate("after-read".to_string()),
                ScriptedHttpBodyEvent::Chunk(b"<div class=\"description\">Loaded</div>".to_vec()),
            ],
            content_length: None,
        },
    ]));
    let after = Detail::new(
        Catalog::new(pool.clone()),
        installed.store,
        http.clone(),
        no_browser(),
    );
    let task = tokio::spawn(async move { after.open(Id::new(id)).await });
    while !http.gate_is_waiting("after-read") {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT read_state FROM job_postings WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "read"
    );
    sqlx::query("DROP TABLE job_posting_sources")
        .execute(&pool)
        .await
        .unwrap();
    assert!(http.release_gate("after-read"));
    assert!(matches!(
        task.await.unwrap().unwrap_err(),
        DetailError::AfterRead { posting, .. } if posting == Id::new(id)
    ));
}
