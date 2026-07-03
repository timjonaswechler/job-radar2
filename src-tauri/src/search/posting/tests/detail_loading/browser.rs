use super::support::*;

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
