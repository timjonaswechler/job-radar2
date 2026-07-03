use super::support::*;

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
