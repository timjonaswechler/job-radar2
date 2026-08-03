use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use search_requests::{Catalog, Input, Status};
use search_resolution::{SearchRule, SearchRuleKind, SearchRuleTarget};
use search_runs::{Context, Runner, SourceAdmission, Status as RunStatus};
use source_engine::test_support::{
    ProfileHttpFailureKind, ScriptedBrowserAcquisition, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

#[tokio::test]
async fn missing_source_commits_failed_run_and_releases_request_lease() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: vec!["missing_source".into()],
        })
        .await
        .unwrap();
    let installed_dir = tempfile::tempdir().unwrap();
    let runner = Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_dir.path()),
        Arc::new(ScriptedProfileHttpClient::new([])),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.source_runs.len(), 1);
    assert_eq!(outcome.source_runs[0].source_key, "missing_source");
    assert_eq!(
        outcome.source_runs[0].diagnostics[0].code,
        "source_not_found"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT last_run_status FROM search_requests WHERE id = ?1"
        )
        .bind(request.id.get())
        .fetch_one(&pool)
        .await
        .unwrap(),
        "failed"
    );

    catalog.delete(request.id).await.unwrap();
}

#[tokio::test]
async fn finalized_candidate_is_committed_as_posting_and_match() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: vec!["fixture".into()],
        })
        .await
        .unwrap();
    let http = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs.json".into(),
        headers: vec![("content-type".into(), b"application/json".to_vec())],
        body: vec![ScriptedHttpBodyEvent::Chunk(
            br#"{"jobs":[{"title":"Platform Engineer","company":"ACME","locations":["Mainz"],"url":"https://example.test/jobs/1"}]}"#.to_vec(),
        )],
        content_length: None,
    }]);
    let runner = Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_dir.path()),
        Arc::new(http),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Completed);
    assert_eq!(outcome.matched_posting_count, 1);
    assert_eq!(outcome.source_runs[0].source_key, "fixture");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matches")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn authored_order_and_partial_success_are_preserved() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: vec!["missing".into(), "fixture".into()],
        })
        .await
        .unwrap();
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [jobs_response("https://example.test/jobs/partial")],
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::CompletedWithErrors);
    assert_eq!(outcome.source_runs[0].source_key, "missing");
    assert_eq!(outcome.source_runs[1].source_key, "fixture");
    assert_eq!(outcome.matched_posting_count, 1);
    let latest_error =
        sqlx::query_scalar::<_, String>("SELECT last_run_error FROM search_requests WHERE id = ?1")
            .bind(request.id.get())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(latest_error.contains("missing"));
}

#[tokio::test]
async fn rerun_reuses_durable_posting_and_creates_a_new_match() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = create_request(&catalog, "fixture").await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [
            jobs_response("https://example.test/jobs/reused"),
            jobs_response("https://example.test/jobs/reused"),
        ],
    );

    runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE job_postings
         SET read_state = 'read',
             interest_state = 'interested',
             preparation_state = 'in_progress',
             application_state = 'submitted'",
    )
    .execute(&pool)
    .await
    .unwrap();

    runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_posting_sources")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matches")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let workflow = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT read_state, interest_state, preparation_state, application_state
         FROM job_postings",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        workflow,
        (
            "read".into(),
            "interested".into(),
            "in_progress".into(),
            "submitted".into(),
        )
    );
}

#[tokio::test]
async fn deleting_request_removes_runs_and_matches_but_retains_posting_occurrences() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = create_request(&catalog, "fixture").await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [jobs_response("https://example.test/jobs/retained")],
    );
    runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    catalog.delete(request.id).await.unwrap();

    for table in ["search_requests", "search_runs", "matches"] {
        assert_eq!(row_count(&pool, table).await, 0, "{table} must cascade");
    }
    assert_eq!(row_count(&pool, "job_postings").await, 1);
    assert_eq!(row_count(&pool, "job_posting_sources").await, 1);
}

#[tokio::test]
async fn cross_source_merge_respects_overlapping_missing_and_disjoint_locations() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "first", "active");
    write_source(installed_dir.path(), "second", "active");
    let request = create_request_for_sources(&catalog, &["first", "second"]).await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [
            jobs_response_values(serde_json::json!([
                {"title":"Laser Engineer","company":"ACME","locations":["Mainz"],"url":"https://first.test/laser"},
                {"title":"Remote Engineer","company":"ACME","locations":[],"url":"https://first.test/remote"},
                {"title":"Optics Engineer","company":"ACME","locations":["Berlin"],"url":"https://first.test/optics"}
            ])),
            jobs_response_values(serde_json::json!([
                {"title":"Laser Engineer","company":"ACME","locations":[" mainz ","Wiesbaden"],"url":"https://second.test/laser"},
                {"title":"Remote Engineer","company":"ACME","locations":["Berlin"],"url":"https://second.test/remote"},
                {"title":"Optics Engineer","company":"ACME","locations":["Hamburg"],"url":"https://second.test/optics"}
            ])),
        ],
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Completed);
    assert_eq!(outcome.matched_posting_count, 4);
    let postings = sqlx::query_as::<_, (String, String)>(
        "SELECT title, locations_json FROM job_postings ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(postings.len(), 4);
    assert_eq!(
        postings[0],
        ("Laser Engineer".into(), r#"["Mainz","Wiesbaden"]"#.into())
    );
    assert_eq!(
        postings[1],
        ("Remote Engineer".into(), r#"["Berlin"]"#.into())
    );
    assert_eq!(
        postings[2],
        ("Optics Engineer".into(), r#"["Berlin"]"#.into())
    );
    assert_eq!(
        postings[3],
        ("Optics Engineer".into(), r#"["Hamburg"]"#.into())
    );
    assert_eq!(row_count(&pool, "job_posting_sources").await, 6);
    let provenance = sqlx::query_as::<_, (String, String, String)>(
        "SELECT postings.title, sources.source_key, sources.url
         FROM job_postings postings
         JOIN job_posting_sources sources ON sources.posting_id = postings.id
         ORDER BY postings.title, sources.source_key, sources.url",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        provenance,
        vec![
            (
                "Laser Engineer".into(),
                "first".into(),
                "https://first.test/laser".into()
            ),
            (
                "Laser Engineer".into(),
                "second".into(),
                "https://second.test/laser".into()
            ),
            (
                "Optics Engineer".into(),
                "first".into(),
                "https://first.test/optics".into()
            ),
            (
                "Optics Engineer".into(),
                "second".into(),
                "https://second.test/optics".into()
            ),
            (
                "Remote Engineer".into(),
                "first".into(),
                "https://first.test/remote".into()
            ),
            (
                "Remote Engineer".into(),
                "second".into(),
                "https://second.test/remote".into()
            ),
        ]
    );
}

#[tokio::test]
async fn invalid_disabled_and_active_sources_keep_authored_order_and_only_active_executes() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_invalid_source(installed_dir.path(), "invalid");
    write_source(installed_dir.path(), "disabled", "disabled");
    write_source(installed_dir.path(), "active", "active");
    let request = create_request_for_sources(&catalog, &["invalid", "disabled", "active"]).await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [jobs_response("https://example.test/jobs/active")],
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::CompletedWithErrors);
    assert_eq!(
        outcome
            .source_runs
            .iter()
            .map(|source| (source.source_key.as_str(), source.status))
            .collect::<Vec<_>>(),
        vec![
            ("invalid", search_runs::SourceStatus::Failed),
            ("disabled", search_runs::SourceStatus::Skipped),
            ("active", search_runs::SourceStatus::Completed),
        ]
    );
    assert_eq!(
        outcome.source_runs[0].diagnostics[0].code,
        "source_not_found"
    );
    assert_eq!(
        outcome.source_runs[1].diagnostics[0].code,
        "source_not_active"
    );
    assert_eq!(outcome.matched_posting_count, 1);
}

#[tokio::test]
async fn only_finalized_candidates_cross_into_posting_and_match_persistence() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = create_request(&catalog, "fixture").await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [jobs_response_values(serde_json::json!([
            {"title":"Platform Engineer","company":"ACME","locations":["Mainz"],"url":"https://example.test/jobs/match"},
            {"title":"Fixture Analyst","company":"ACME","locations":["Mainz"],"url":"https://example.test/jobs/rejected"}
        ]))],
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    let counts = outcome.source_runs[0].resolution.as_ref().unwrap().counts;
    assert_eq!(counts.discovered, 2);
    assert_eq!(counts.finalized, 1);
    assert_eq!(outcome.matched_posting_count, 1);
    assert_eq!(row_count(&pool, "job_postings").await, 1);
    assert_eq!(row_count(&pool, "matches").await, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "Platform Engineer"
    );
}

#[tokio::test]
async fn candidate_resolution_failure_is_projected_without_discarding_successful_sources() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "successful", "active");
    write_source(installed_dir.path(), "failed", "active");
    let request = create_request_for_sources(&catalog, &["successful", "failed"]).await;
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [
            jobs_response("https://example.test/jobs/successful"),
            ScriptedHttpEvent::Response {
                status: 200,
                final_url: "https://example.test/jobs.json".into(),
                headers: vec![("content-type".into(), b"application/json".to_vec())],
                body: vec![ScriptedHttpBodyEvent::Failure(
                    ProfileHttpFailureKind::Connect,
                )],
                content_length: None,
            },
        ],
    );

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::CompletedWithErrors);
    assert_eq!(
        outcome.source_runs[0].status,
        search_runs::SourceStatus::Completed
    );
    assert_eq!(
        outcome.source_runs[1].status,
        search_runs::SourceStatus::Failed
    );
    assert!(outcome.source_runs[1]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("Candidate Resolution failed: DiscoveryExecution")));
    assert_eq!(outcome.matched_posting_count, 1);
    assert_eq!(row_count(&pool, "matches").await, 1);
    let latest_error =
        sqlx::query_scalar::<_, String>("SELECT last_run_error FROM search_requests WHERE id = ?1")
            .bind(request.id.get())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(latest_error.contains("failed: Candidate Resolution failed: DiscoveryExecution"));
}

#[tokio::test]
async fn explicit_radius_uses_geo_resolver_and_persists_in_radius_match() {
    struct Mainz;
    impl geo::GeoResolver for Mainz {
        fn resolve<'a>(&'a self, input: &'a str) -> geo::GeoResolveFuture<'a> {
            Box::pin(async move {
                Ok(vec![geo::ResolvedLocation {
                    input: input.to_string(),
                    label: "Mainz".into(),
                    point: geo::GeoPoint {
                        latitude: 49.9929,
                        longitude: 8.2473,
                    },
                }])
            })
        }
    }

    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec!["Mainz".into()],
            radius_km: Some(25),
            source_keys: vec!["fixture".into()],
        })
        .await
        .unwrap();
    let runner = runner_with_responses(
        &pool,
        installed_dir.path(),
        [jobs_response("https://example.test/jobs/radius")],
    );
    let geo = Mainz;

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: Some(&geo),
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Completed);
    assert_eq!(outcome.matched_posting_count, 1);
}

#[tokio::test]
async fn unresolved_authored_radius_location_preserves_requirements_message() {
    struct Unresolved;
    impl geo::GeoResolver for Unresolved {
        fn resolve<'a>(&'a self, _input: &'a str) -> geo::GeoResolveFuture<'a> {
            Box::pin(async { Ok(vec![]) })
        }
    }

    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec!["Atlantis".into()],
            radius_km: Some(25),
            source_keys: vec!["missing".into()],
        })
        .await
        .unwrap();
    let runner = runner_with_responses(&pool, installed_dir.path(), []);

    let error = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: Some(&Unresolved),
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        search_runs::Error::Requirements(
            "Search Request location could not be resolved: Atlantis".to_string()
        )
    );
    assert_eq!(row_count(&pool, "search_runs").await, 0);
    catalog.delete(request.id).await.unwrap();
}

#[tokio::test]
async fn radius_resolver_failure_remains_source_owned_geo_resolution_failure() {
    struct Failed;
    impl geo::GeoResolver for Failed {
        fn resolve<'a>(&'a self, _input: &'a str) -> geo::GeoResolveFuture<'a> {
            Box::pin(async {
                Err("Search Request location could not be resolved: resolver unavailable".into())
            })
        }
    }

    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec!["Berlin".into()],
            radius_km: Some(25),
            source_keys: vec!["fixture".into()],
        })
        .await
        .unwrap();
    let runner = runner_with_responses(&pool, installed_dir.path(), []);

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: Some(&Failed),
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(
        outcome.source_runs[0].status,
        search_runs::SourceStatus::Failed
    );
    assert_eq!(
        outcome.source_runs[0].error.as_deref(),
        Some("Candidate Resolution failed: GeoResolution")
    );
    assert_eq!(row_count(&pool, "search_runs").await, 1);
}

#[tokio::test]
async fn explicit_radius_without_geo_resolver_fails_before_terminal_persistence() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let request = catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec!["Mainz".into()],
            radius_km: Some(25),
            source_keys: vec!["missing".into()],
        })
        .await
        .unwrap();
    let installed_dir = tempfile::tempdir().unwrap();
    let runner = runner_with_responses(&pool, installed_dir.path(), []);

    let error = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap_err();

    assert!(
        matches!(error, search_runs::Error::Requirements(message) if message.contains("GeoResolver"))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    catalog.delete(request.id).await.unwrap();
}

#[tokio::test]
async fn draft_source_requires_explicit_development_smoke_admission() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "draft_fixture", "draft");
    let http = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs.json".into(),
        headers: vec![("content-type".into(), b"application/json".to_vec())],
        body: vec![ScriptedHttpBodyEvent::Chunk(
            br#"{"jobs":[{"title":"Platform Engineer","company":"ACME","locations":[],"url":"https://example.test/jobs/draft"}]}"#.to_vec(),
        )],
        content_length: None,
    }]);
    let runner = Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_dir.path()),
        Arc::new(http),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );

    let blocked = create_request(&catalog, "draft_fixture").await;
    let blocked_outcome = runner
        .run(
            catalog.begin_execution(blocked.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();
    assert_eq!(blocked_outcome.status, RunStatus::Failed);
    assert_eq!(
        blocked_outcome.source_runs[0].status,
        search_runs::SourceStatus::Skipped
    );

    let admitted = create_request(&catalog, "draft_fixture").await;
    let admitted_outcome = runner
        .run(
            catalog.begin_execution(admitted.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::DevelopmentSmokeAllowDraft,
            },
        )
        .await
        .unwrap();
    assert_eq!(admitted_outcome.status, RunStatus::Completed);
    assert_eq!(admitted_outcome.matched_posting_count, 1);
}

#[tokio::test]
async fn cancellation_after_terminal_cutoff_cannot_change_committed_outcome() {
    struct CancelAfterCutoff {
        calls: AtomicUsize,
        cancelled: AtomicBool,
    }
    impl source_engine::execution::RuntimeCancellation for CancelAfterCutoff {
        fn is_cancelled(&self) -> bool {
            let observed_before_call = self.cancelled.load(Ordering::SeqCst);
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == 2 {
                self.cancelled.store(true, Ordering::SeqCst);
            }
            observed_before_call
        }
    }

    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    let request = create_request(&catalog, "missing").await;
    let runner = runner_with_responses(&pool, installed_dir.path(), []);
    let cancellation = CancelAfterCutoff {
        calls: AtomicUsize::new(0),
        cancelled: AtomicBool::new(false),
    };

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: Some(&cancellation),
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert!(cancellation.cancelled.load(Ordering::SeqCst));
    assert_eq!(cancellation.calls.load(Ordering::SeqCst), 2);
    assert!(source_engine::execution::RuntimeCancellation::is_cancelled(
        &cancellation
    ));
    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "failed"
    );
}

#[tokio::test]
async fn request_lease_is_held_while_terminal_commit_waits_and_released_after_outcome() {
    struct ObserveCutoff {
        calls: AtomicUsize,
        observed: tokio::sync::Notify,
    }
    impl source_engine::execution::RuntimeCancellation for ObserveCutoff {
        fn is_cancelled(&self) -> bool {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == 2 {
                self.observed.notify_one();
            }
            false
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("lease.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    let catalog = Catalog::new(pool.clone());
    let request = create_request(&catalog, "missing").await;
    let execution = catalog.begin_execution(request.id).await.unwrap();
    let runner = runner_with_responses(&pool, temp.path(), []);
    let mut writer = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let cancellation = Arc::new(ObserveCutoff {
        calls: AtomicUsize::new(0),
        observed: tokio::sync::Notify::new(),
    });
    let task_cancellation = cancellation.clone();
    let run = tokio::spawn(async move {
        runner
            .run(
                execution,
                Context {
                    cancellation: Some(task_cancellation.as_ref()),
                    geo: None,
                    source_admission: SourceAdmission::ActiveOnly,
                },
            )
            .await
    });

    tokio::time::timeout(Duration::from_secs(2), cancellation.observed.notified())
        .await
        .expect("Runner must reach the terminal cancellation cutoff");
    tokio::task::yield_now().await;
    assert!(
        !run.is_finished(),
        "terminal commit must wait for writer lock"
    );
    assert!(matches!(
        catalog.delete(request.id).await,
        Err(search_requests::Error::Busy { id }) if id == request.id
    ));

    sqlx::query("ROLLBACK").execute(&mut *writer).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .expect("Runner must finish after the writer lock is released")
        .unwrap()
        .unwrap();
    assert_eq!(cancellation.calls.load(Ordering::SeqCst), 2);
    catalog.delete(request.id).await.unwrap();
}

#[tokio::test]
async fn cancellation_before_source_execution_commits_cancelled_without_matches() {
    struct Cancelled;
    impl source_engine::execution::RuntimeCancellation for Cancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    write_source(installed_dir.path(), "fixture", "active");
    let request = create_request(&catalog, "fixture").await;
    let runner = Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_dir.path()),
        Arc::new(ScriptedProfileHttpClient::new([])),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );
    let cancellation = Cancelled;

    let outcome = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: Some(&cancellation),
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.status, RunStatus::Cancelled);
    assert_eq!(outcome.matched_posting_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matches")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn terminal_commit_rolls_back_when_latest_projection_update_fails() {
    let pool = migrated_pool().await;
    let catalog = Catalog::new(pool.clone());
    let installed_dir = tempfile::tempdir().unwrap();
    let request = create_request(&catalog, "missing_source").await;
    sqlx::query(
        "CREATE TRIGGER reject_latest_update BEFORE UPDATE ON search_requests
         BEGIN SELECT RAISE(ABORT, 'latest update rejected'); END",
    )
    .execute(&pool)
    .await
    .unwrap();
    let runner = Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_dir.path()),
        Arc::new(ScriptedProfileHttpClient::new([])),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );

    let error = runner
        .run(
            catalog.begin_execution(request.id).await.unwrap(),
            Context {
                cancellation: None,
                geo: None,
                source_admission: SourceAdmission::ActiveOnly,
            },
        )
        .await
        .unwrap_err();

    assert!(
        matches!(error, search_runs::Error::Storage(message) if message.contains("latest update rejected"))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    catalog.delete(request.id).await.unwrap();
}

async fn create_request(catalog: &Catalog, source_key: &str) -> search_requests::Record {
    create_request_for_sources(catalog, &[source_key]).await
}

async fn create_request_for_sources(
    catalog: &Catalog,
    source_keys: &[&str],
) -> search_requests::Record {
    catalog
        .create(Input {
            status: Status::Active,
            include_rules: vec![SearchRule {
                target: SearchRuleTarget::Title,
                kind: SearchRuleKind::Text,
                value: "engineer".into(),
            }],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: source_keys.iter().map(|key| (*key).to_string()).collect(),
        })
        .await
        .unwrap()
}

fn runner_with_responses(
    pool: &sqlx::SqlitePool,
    installed_root: &std::path::Path,
    responses: impl IntoIterator<Item = ScriptedHttpEvent>,
) -> Runner {
    Runner::new(
        pool.clone(),
        sources::installed::Store::new(installed_root),
        Arc::new(ScriptedProfileHttpClient::new(responses)),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    )
}

fn jobs_response(url: &str) -> ScriptedHttpEvent {
    jobs_response_values(serde_json::json!([{
        "title": "Platform Engineer",
        "company": "ACME",
        "locations": ["Mainz"],
        "url": url
    }]))
}

fn jobs_response_values(jobs: serde_json::Value) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs.json".into(),
        headers: vec![("content-type".into(), b"application/json".to_vec())],
        body: vec![ScriptedHttpBodyEvent::Chunk(
            serde_json::json!({ "jobs": jobs }).to_string().into_bytes(),
        )],
        content_length: None,
    }
}

fn write_invalid_source(root: &std::path::Path, key: &str) {
    write_source(root, key, "active");
    let path = root.join(format!("sources/{key}.json"));
    let mut source: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    source.as_object_mut().unwrap().remove("sourceSupport");
    std::fs::write(path, source.to_string()).unwrap();
}

fn write_source(root: &std::path::Path, key: &str, status: &str) {
    let directory = root.join("sources");
    std::fs::create_dir_all(&directory).unwrap();
    let source = serde_json::json!({
        "schemaVersion": 3,
        "key": key,
        "name": "Fixture Source",
        "status": status,
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "fixture_discovery",
            "name": "Fixture Discovery",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "https://example.test/jobs.json",
                        "timeoutMs": 1000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "reference": { "url": {
                            "type": "json_path", "jsonPath": "$.url", "cardinality": "one"
                        }},
                        "providerValues": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
                            "locations": { "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" }
                        }
                    }
                }]
            }
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Search Run fixture."
        }
    });
    std::fs::write(directory.join(format!("{key}.json")), source.to_string()).unwrap();
}

async fn row_count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn migrated_pool() -> sqlx::SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    pool
}
