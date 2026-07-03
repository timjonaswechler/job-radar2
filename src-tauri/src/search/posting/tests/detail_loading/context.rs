use super::support::*;

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
