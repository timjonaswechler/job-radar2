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
        let extractor =
            crate::declarative::posting_detail::PostingDetailExtractor::new(client.clone());

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
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
                text: "Persisted description".to_string()
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
        let extractor =
            crate::declarative::posting_detail::PostingDetailExtractor::new(client.clone());

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
            .await
            .unwrap();

        assert_eq!(detail.posting.read_state, ReadState::Read);
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Stored description".to_string()
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
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("postingDetail is required")));
        let client = FixturePostingDetailHttpClient::new([]);
        let extractor =
            crate::declarative::posting_detail::PostingDetailExtractor::new(client.clone());

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
            .await
            .unwrap();

        assert_eq!(detail.posting.read_state, ReadState::Read);
        assert!(matches!(
            detail.description_state,
            PostingDescriptionState::Unsupported { .. }
        ));
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
        let extractor =
            crate::declarative::posting_detail::PostingDetailExtractor::new(client.clone());

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
            .await
            .unwrap();

        assert_eq!(
            client.requested_urls(),
            vec![
                "https://primary.example.test/jobs/laser",
                "https://fallback.example.test/jobs/laser"
            ]
        );
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Fallback description".to_string()
            }
        );
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
        let extractor = crate::declarative::posting_detail::PostingDetailExtractor::new(client);

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
            .await
            .unwrap();

        match detail.description_state {
            PostingDescriptionState::Failed { message } => {
                assert!(message.contains("detail_source"));
                assert!(message.contains("HTTP 500"));
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
        let extractor =
            crate::declarative::posting_detail::PostingDetailExtractor::new(client.clone());

        let detail = JobPostingService::new(&pool)
            .get_posting_detail_with_extractor(posting_id, &snapshot, &extractor)
            .await
            .unwrap();

        assert_eq!(
            client.requested_urls(),
            vec!["https://primary.example.test/jobs/laser?token=primary-token&job=primary-42"]
        );
        assert_eq!(
            detail.description_state,
            PostingDescriptionState::Loaded {
                text: "Primary aligned description".to_string()
            }
        );
    });
}
