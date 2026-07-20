use super::support::*;

#[test]
fn get_job_posting_surfaces_missing_and_invalid_source_diagnostics_as_unsupported() {
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
        let client = FixtureDetailHttpClient::new([]);

        let detail = JobPostingService::new(&pool)
            .get_job_posting_with_clients(
                posting_id,
                &snapshot,
                client.client(),
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
fn get_job_posting_reports_parse_empty_too_short_and_missing_meta_diagnostics() {
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
        let client = FixtureDetailHttpClient::new([
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
            .get_job_posting_with_clients(
                posting_id,
                &snapshot,
                client.client(),
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
fn get_job_posting_reports_no_match_and_multiple_match_diagnostics() {
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
        let client = FixtureDetailHttpClient::new([
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
            .get_job_posting_with_clients(
                posting_id,
                &snapshot,
                client.client(),
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
