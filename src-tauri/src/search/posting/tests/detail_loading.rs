use super::*;

#[test]
fn get_posting_detail_loads_missing_description_marks_read_and_persists_text() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            posting_id,
            "detail_source",
            "Detail Source",
            "https://detail.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, source_id).await;
        let snapshot = test_snapshot(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}",
            )],
            vec![profile_source_json(
                "detail_source",
                "detail_profile",
                "detail_path",
                json!({}),
            )],
        );
        let client = FixturePostingDetailHttpClient::new([(
            "https://detail.example.test/jobs/laser".to_string(),
            Ok("<main><div class=\"description\">Persisted description</div></main>".to_string()),
        )]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        assert_eq!(detail.posting.read_state, ReadState::Read);
        assert_eq!(
            detail.posting.description_text.as_deref(),
            Some("Persisted description")
        );
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Persisted description".to_string(),
                diagnostics: Vec::new()
            }
        );
        assert_eq!(
            persisted_description_text(&pool, posting_id)
                .await
                .as_deref(),
            Some("Persisted description")
        );
        assert_eq!(
            client.requested_urls(),
            vec!["https://detail.example.test/jobs/laser"]
        );
    });
}

#[test]
fn get_posting_detail_returns_existing_description_without_fetching() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        persist_description_text(&pool, posting_id, "Stored description").await;
        let source_id = insert_existing_source(
            &pool,
            posting_id,
            "detail_source",
            "Detail Source",
            "https://detail.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, source_id).await;
        let snapshot = test_snapshot(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}",
            )],
            vec![profile_source_json(
                "detail_source",
                "detail_profile",
                "detail_path",
                json!({}),
            )],
        );
        let client = FixturePostingDetailHttpClient::new([]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        assert_eq!(detail.posting.read_state, ReadState::Read);
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Stored description".to_string(),
                diagnostics: Vec::new()
            }
        );
        assert!(client.requested_urls().is_empty());
    });
}

#[test]
fn get_posting_detail_returns_unsupported_when_no_concrete_source_supports_detail() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            posting_id,
            "list_only_source",
            "List Only Source",
            "https://list.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, source_id).await;
        let snapshot = test_snapshot_with_diagnostics(
            vec![profile_without_detail_json("list_profile", "list_path")],
            vec![profile_source_json(
                "list_only_source",
                "list_profile",
                "list_path",
                json!({}),
            )],
        );
        assert_eq!(snapshot.diagnostics, Vec::new());
        let client = FixturePostingDetailHttpClient::new([]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        assert_eq!(detail.posting.read_state, ReadState::Read);
        match detail.description_state {
            PostingDescriptionState::Unsupported { diagnostics, .. } => assert!(diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "posting_detail_missing")),
            other => panic!("expected unsupported state, got {other:?}"),
        }
        assert_eq!(persisted_description_text(&pool, posting_id).await, None);
        assert!(client.requested_urls().is_empty());
    });
}

#[test]
fn get_posting_detail_falls_back_after_detail_capable_source_failure() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source(
            &pool,
            posting_id,
            "primary_detail_source",
            "Primary Detail Source",
            "https://primary.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "fallback_detail_source",
            "Fallback Detail Source",
            "https://fallback.example.test/jobs/laser",
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let snapshot = test_snapshot(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}",
            )],
            vec![
                profile_source_json(
                    "primary_detail_source",
                    "detail_profile",
                    "detail_path",
                    json!({}),
                ),
                profile_source_json(
                    "fallback_detail_source",
                    "detail_profile",
                    "detail_path",
                    json!({}),
                ),
            ],
        );
        let client = FixturePostingDetailHttpClient::new([
            (
                "https://primary.example.test/jobs/laser".to_string(),
                Err("network unavailable".to_string()),
            ),
            (
                "https://fallback.example.test/jobs/laser".to_string(),
                Ok("<div class=\"description\">Fallback description</div>".to_string()),
            ),
        ]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        assert_eq!(
            client.requested_urls(),
            vec![
                "https://primary.example.test/jobs/laser",
                "https://fallback.example.test/jobs/laser"
            ]
        );
        match detail.description_state {
            PostingDescriptionState::Loaded { text, diagnostics } => {
                assert_eq!(text, "Fallback description");
                assert!(diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "fetch_failed"));
            }
            other => panic!("expected loaded state, got {other:?}"),
        }
        assert_eq!(
            persisted_description_text(&pool, posting_id)
                .await
                .as_deref(),
            Some("Fallback description")
        );
    });
}

#[test]
fn get_posting_detail_reports_failed_after_all_detail_capable_sources_fail() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            posting_id,
            "detail_source",
            "Detail Source",
            "https://detail.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, source_id).await;
        let snapshot = test_snapshot(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}",
            )],
            vec![profile_source_json(
                "detail_source",
                "detail_profile",
                "detail_path",
                json!({}),
            )],
        );
        let client = FixturePostingDetailHttpClient::new([(
            "https://detail.example.test/jobs/laser".to_string(),
            Err("HTTP 500".to_string()),
        )]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        match detail.description_state {
            PostingDescriptionState::Failed {
                message,
                diagnostics,
            } => {
                assert!(message.contains("HTTP 500"));
                assert!(diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "fetch_failed"
                        && diagnostic.details.as_ref().unwrap()["postingSourceKey"]
                            == "detail_source"
                }));
            }
            other => panic!("expected failed state, got {other:?}"),
        }
        assert_eq!(persisted_description_text(&pool, posting_id).await, None);
    });
}

#[test]
fn get_posting_detail_fetches_with_aligned_source_url_config_and_posting_meta() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source_with_meta(
            &pool,
            posting_id,
            "primary_detail_source",
            "Primary Detail Source",
            "https://primary.example.test/jobs/laser",
            [("jobId", "primary-42")],
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source_with_meta(
            &pool,
            posting_id,
            "fallback_detail_source",
            "Fallback Detail Source",
            "https://fallback.example.test/jobs/laser",
            [("jobId", "fallback-99")],
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let snapshot = test_snapshot(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}?token={{sourceConfig:token}}&job={{postingMeta:jobId}}",
            )],
            vec![
                profile_source_json(
                    "primary_detail_source",
                    "detail_profile",
                    "detail_path",
                    json!({ "token": "primary-token" }),
                ),
                profile_source_json(
                    "fallback_detail_source",
                    "detail_profile",
                    "detail_path",
                    json!({ "token": "fallback-token" }),
                ),
            ],
        );
        let client = FixturePostingDetailHttpClient::new([(
            "https://primary.example.test/jobs/laser?token=primary-token&job=primary-42"
                .to_string(),
            Ok("<div class=\"description\">Primary aligned description</div>".to_string()),
        )]);
        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        assert_eq!(
            client.requested_urls(),
            vec!["https://primary.example.test/jobs/laser?token=primary-token&job=primary-42"]
        );
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Primary aligned description".to_string(),
                diagnostics: Vec::new()
            }
        );
    });
}

#[test]
fn get_posting_detail_surfaces_missing_and_invalid_source_diagnostics_as_unsupported() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source(
            &pool,
            posting_id,
            "missing_detail_source",
            "Missing Detail Source",
            "https://missing.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "invalid_detail_source",
            "Invalid Detail Source",
            "https://invalid.example.test/jobs/laser",
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let snapshot = test_snapshot_with_diagnostics(
            vec![detail_profile_json(
                "detail_profile",
                "detail_path",
                "{{posting:url}}",
            )],
            vec![profile_source_json(
                "invalid_detail_source",
                "detail_profile",
                "detail_path",
                json!({ "token": 42 }),
            )],
        );
        let client = FixturePostingDetailHttpClient::new([]);

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        match detail.description_state {
            PostingDescriptionState::Unsupported { diagnostics, .. } => {
                assert!(diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "source_not_found"
                        && diagnostic.details.as_ref().unwrap()["postingSourceKey"]
                            == "missing_detail_source"
                }));
                assert!(diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "invalid_source_config_property_type"
                        && diagnostic.details.as_ref().unwrap()["postingSourceKey"]
                            == "invalid_detail_source"
                }));
            }
            other => panic!("expected unsupported state, got {other:?}"),
        }
        assert!(client.requested_urls().is_empty());
        assert_eq!(persisted_description_text(&pool, posting_id).await, None);
    });
}

#[test]
fn get_posting_detail_reports_parse_empty_too_short_and_missing_meta_diagnostics() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source(
            &pool,
            posting_id,
            "parse_failure_source",
            "Parse Failure Source",
            "https://detail.example.test/bad-json",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "empty_description_source",
            "Empty Description Source",
            "https://detail.example.test/empty",
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "short_description_source",
            "Short Description Source",
            "https://detail.example.test/short",
            "2026-06-03T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "missing_meta_source",
            "Missing Meta Source",
            "https://detail.example.test/collection",
            "2026-06-04T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let snapshot = test_snapshot(
            vec![
                profile_json_with_detail_step(
                    "json_detail_profile",
                    "detail_path",
                    Some(direct_json_detail_step("{{posting:url}}", None)),
                ),
                detail_profile_json("html_detail_profile", "detail_path", "{{posting:url}}"),
                profile_json_with_detail_step(
                    "short_detail_profile",
                    "detail_path",
                    Some(direct_json_detail_step(
                        "{{posting:url}}",
                        Some(json!({ "minDescriptionLength": 20 })),
                    )),
                ),
                profile_json_with_detail_step(
                    "collection_detail_profile",
                    "detail_path",
                    Some(collection_json_detail_step("{{posting:url}}")),
                ),
            ],
            vec![
                profile_source_json(
                    "parse_failure_source",
                    "json_detail_profile",
                    "detail_path",
                    json!({}),
                ),
                profile_source_json(
                    "empty_description_source",
                    "html_detail_profile",
                    "detail_path",
                    json!({}),
                ),
                profile_source_json(
                    "short_description_source",
                    "short_detail_profile",
                    "detail_path",
                    json!({}),
                ),
                profile_source_json(
                    "missing_meta_source",
                    "collection_detail_profile",
                    "detail_path",
                    json!({}),
                ),
            ],
        );
        let client = FixturePostingDetailHttpClient::new([
            (
                "https://detail.example.test/bad-json".to_string(),
                Ok("{not-json".to_string()),
            ),
            (
                "https://detail.example.test/empty".to_string(),
                Ok("<div class=\"description\"></div>".to_string()),
            ),
            (
                "https://detail.example.test/short".to_string(),
                Ok(json!({ "description": "Tiny" }).to_string()),
            ),
            (
                "https://detail.example.test/collection".to_string(),
                Ok(json!({ "jobs": [{ "id": "other", "description": "Other" }] }).to_string()),
            ),
        ]);

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        match detail.description_state {
            PostingDescriptionState::Failed { diagnostics, .. } => {
                for code in [
                    "json_parse_failed",
                    "description_empty",
                    "description_too_short",
                    "posting_meta_missing",
                ] {
                    assert!(
                        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
                        "missing diagnostic code {code}; diagnostics: {diagnostics:?}"
                    );
                }
            }
            other => panic!("expected failed state, got {other:?}"),
        }
        assert_eq!(persisted_description_text(&pool, posting_id).await, None);
    });
}

#[test]
fn get_posting_detail_reports_no_match_and_multiple_match_diagnostics() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source_with_meta(
            &pool,
            posting_id,
            "no_match_source",
            "No Match Source",
            "https://detail.example.test/no-match",
            [("jobId", "selected-42")],
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source_with_meta(
            &pool,
            posting_id,
            "multiple_match_source",
            "Multiple Match Source",
            "https://detail.example.test/multiple-match",
            [("jobId", "selected-42")],
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let snapshot = test_snapshot(
            vec![profile_json_with_detail_step(
                "collection_detail_profile",
                "detail_path",
                Some(collection_json_detail_step("{{posting:url}}")),
            )],
            vec![
                profile_source_json(
                    "no_match_source",
                    "collection_detail_profile",
                    "detail_path",
                    json!({}),
                ),
                profile_source_json(
                    "multiple_match_source",
                    "collection_detail_profile",
                    "detail_path",
                    json!({}),
                ),
            ],
        );
        let client = FixturePostingDetailHttpClient::new([
            (
                "https://detail.example.test/no-match".to_string(),
                Ok(json!({ "jobs": [{ "id": "other", "description": "Other" }] }).to_string()),
            ),
            (
                "https://detail.example.test/multiple-match".to_string(),
                Ok(json!({
                    "jobs": [
                        { "id": "selected-42", "description": "First" },
                        { "id": "selected-42", "description": "Second" }
                    ]
                })
                .to_string()),
            ),
        ]);

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(
                posting_id,
                &snapshot,
                &client,
                &UnavailableProfileBrowserClient,
            )
            .await
            .unwrap();

        match detail.description_state {
            PostingDescriptionState::Failed { diagnostics, .. } => {
                assert!(diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "detail_match_missing"));
                assert!(diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "detail_match_multiple"));
            }
            other => panic!("expected failed state, got {other:?}"),
        }
        assert_eq!(persisted_description_text(&pool, posting_id).await, None);
    });
}

#[test]
fn get_posting_detail_executes_compiled_browser_detail_plan_through_browser_client() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            posting_id,
            "browser_detail_source",
            "Browser Detail Source",
            "https://browser.example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, source_id).await;
        let snapshot = test_snapshot(
            vec![profile_json_with_detail_step(
                "browser_detail_profile",
                "detail_path",
                Some(browser_html_detail_step("{{posting:url}}")),
            )],
            vec![profile_source_json(
                "browser_detail_source",
                "browser_detail_profile",
                "detail_path",
                json!({}),
            )],
        );
        let fetcher = FixturePostingDetailHttpClient::new([]);
        let browser = FixtureProfileBrowserClient::new([(
            "https://browser.example.test/jobs/laser".to_string(),
            Ok(
                "<main><div class=\"description\">Rendered browser description</div></main>"
                    .to_string(),
            ),
        )]);

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_clients(posting_id, &snapshot, &fetcher, &browser)
            .await
            .unwrap();

        assert!(fetcher.requested_urls().is_empty());
        assert_eq!(
            browser.requested_urls(),
            vec!["https://browser.example.test/jobs/laser"]
        );
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Rendered browser description".to_string(),
                diagnostics: Vec::new()
            }
        );
    });
}

fn direct_json_detail_step(fetch_url: &str, accept_when: Option<Value>) -> Value {
    let mut strategy = json!({
        "key": "json_detail",
        "fetch": {
            "mode": "http",
            "method": "GET",
            "url": fetch_url,
            "timeoutMs": 1000
        },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
            "fields": {
                "descriptionText": {
                    "type": "json_path",
                    "jsonPath": "$.description",
                    "cardinality": "one"
                }
            }
        }
    });
    if let Some(accept_when) = accept_when {
        strategy["acceptWhen"] = accept_when;
    }
    json!({ "strategies": [strategy] })
}

fn collection_json_detail_step(fetch_url: &str) -> Value {
    json!({
        "strategies": [{
            "key": "collection_detail",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": fetch_url,
                "timeoutMs": 1000
            },
            "parse": { "type": "json" },
            "select": { "type": "json_path", "jsonPath": "$.jobs" },
            "match": {
                "left": {
                    "type": "json_path",
                    "jsonPath": "$.id",
                    "cardinality": "one"
                },
                "right": {
                    "type": "posting_meta",
                    "key": "jobId",
                    "cardinality": "one"
                }
            },
            "extract": {
                "fields": {
                    "descriptionText": {
                        "type": "json_path",
                        "jsonPath": "$.description",
                        "cardinality": "one"
                    }
                }
            }
        }]
    })
}

fn browser_html_detail_step(fetch_url: &str) -> Value {
    json!({
        "strategies": [{
            "key": "browser_detail",
            "fetch": {
                "mode": "browser",
                "url": fetch_url,
                "timeoutMs": 1000
            },
            "parse": { "type": "html" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "descriptionText": {
                        "type": "css_text",
                        "selector": ".description",
                        "cardinality": "first"
                    }
                }
            }
        }]
    })
}
