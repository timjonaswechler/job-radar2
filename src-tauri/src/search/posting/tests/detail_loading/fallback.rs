use super::support::*;

#[test]
fn get_job_posting_falls_back_after_detail_capable_source_failure() {
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
        let client = FixtureDetailHttpClient::new([
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
            .get_job_posting_with_clients(
                posting_id,
                &snapshot,
                client.client(),
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
fn get_job_posting_falls_back_after_typed_source_mismatch_and_writes_only_final_description() {
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
        let primary_url = "https://primary.example.test/jobs/laser";
        let fallback_url = "https://fallback.example.test/jobs/laser";
        let primary_source_id = insert_existing_source(
            &pool,
            posting_id,
            "primary_detail_source",
            "Primary Detail Source",
            primary_url,
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "fallback_detail_source",
            "Fallback Detail Source",
            fallback_url,
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
        let expected = |source_key: &str, url: &str| {
            let (_, identity) =
                crate::profile_dsl::runtime::validate_posting_reference(source_key, url, None)
                    .unwrap();
            SourceDetailRequestSnapshot::new(
                source_key,
                identity,
                RequestedDetailFields::description_text(),
            )
        };
        let primary_call = expected("primary_detail_source", primary_url);
        let fallback_call = expected("fallback_detail_source", fallback_url);
        let execution = ScriptedSourceDetailExecution::new([
            (
                primary_call.clone(),
                Ok(SourceDetailOutcome::SourceMismatch),
            ),
            (
                fallback_call.clone(),
                Ok(SourceDetailOutcome::Completed {
                    fields: DetailPatch {
                        description_text: Some("Fallback scripted description".to_string()),
                        ..DetailPatch::default()
                    },
                    dispositions: vec![RequestedFieldDisposition::Produced {
                        field: DetailField::DescriptionText,
                    }],
                    phase_evidence: None,
                }),
            ),
        ]);

        let detail = JobPostingService::new(&pool)
            .get_job_posting_with_detail_execution(posting_id, &snapshot, &execution)
            .await
            .unwrap();

        assert_eq!(
            detail.posting.description_text.as_deref(),
            Some("Fallback scripted description")
        );
        assert_eq!(
            persisted_description_text(&pool, posting_id)
                .await
                .as_deref(),
            Some("Fallback scripted description")
        );
        assert_eq!(
            execution.recorded_calls(),
            vec![primary_call, fallback_call]
        );
        execution.assert_finished();
    });
}

#[test]
fn get_job_posting_reports_failed_after_all_detail_capable_sources_fail() {
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
        let client = FixtureDetailHttpClient::new([(
            "https://detail.example.test/jobs/laser".to_string(),
            Err("HTTP 500".to_string()),
        )]);
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
            PostingDescriptionState::Failed {
                message,
                diagnostics,
            } => {
                assert!(message.contains("HTTP fetch failed"));
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
