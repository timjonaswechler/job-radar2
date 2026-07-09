use super::*;

#[test]
fn imports_new_posting_with_source_row_and_primary_source() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let result = search_run_result(vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![
                source("schott_ag", "SCHOTT AG", "https://example.test/jobs/laser"),
                source(
                    "stepstone_de",
                    "StepStone Deutschland",
                    "https://stepstone.example.test/jobs/laser",
                ),
            ],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        let posting_row = sqlx::query(
            "SELECT id, title, company, locations_json, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at
             FROM job_postings",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let posting_id: i64 = posting_row.get("id");
        let primary_source_id: i64 = posting_row.get("primary_source_id");

        assert_eq!(posting_row.get::<String, _>("title"), "Laser Engineer");
        assert_eq!(posting_row.get::<String, _>("company"), "ACME GmbH");
        assert_eq!(locations_from_row(&posting_row), vec!["Mainz"]);
        assert_eq!(posting_row.get::<String, _>("read_state"), "unread");
        assert_eq!(posting_row.get::<String, _>("interest_state"), "undecided");
        assert_eq!(
            posting_row.get::<String, _>("preparation_state"),
            "not_started"
        );
        assert_eq!(
            posting_row.get::<String, _>("application_state"),
            "not_applied"
        );
        assert_eq!(
            posting_row.get::<String, _>("first_seen_at"),
            result.generated_at
        );
        assert_eq!(
            posting_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );

        assert_eq!(table_count(&pool, "job_posting_sources").await, 2);
        let source_row = sqlx::query(
            "SELECT id, posting_id, source_key, source_name_snapshot, url,
                    posting_meta_json, first_seen_at, last_seen_at
             FROM job_posting_sources
             WHERE id = ?1",
        )
        .bind(primary_source_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(source_row.get::<i64, _>("id"), primary_source_id);
        assert_eq!(source_row.get::<i64, _>("posting_id"), posting_id);
        assert_eq!(source_row.get::<String, _>("source_key"), "schott_ag");
        assert_eq!(
            source_row.get::<String, _>("source_name_snapshot"),
            "SCHOTT AG"
        );
        assert_eq!(
            source_row.get::<String, _>("url"),
            "https://example.test/jobs/laser"
        );
        assert_eq!(source_row.get::<String, _>("posting_meta_json"), "{}");
        assert_eq!(
            source_row.get::<String, _>("first_seen_at"),
            result.generated_at
        );
        assert_eq!(
            source_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );
    });
}

#[test]
fn imports_posting_meta_per_source_and_updates_existing_source_metadata() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let first_result = search_run_result(vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source_with_meta(
                "schott_ag",
                "SCHOTT AG",
                "https://example.test/jobs/laser",
                [("jobId", "old-42")],
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&first_result)
            .await
            .unwrap();

        let initial_meta: String =
            sqlx::query_scalar("SELECT posting_meta_json FROM job_posting_sources")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(initial_meta, json!({ "jobId": "old-42" }).to_string());

        let second_result = search_run_result(vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source_with_meta(
                "schott_ag",
                "SCHOTT AG Careers",
                "https://example.test/jobs/laser",
                [("jobId", "new-99")],
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&second_result)
            .await
            .unwrap();

        assert_eq!(table_count(&pool, "job_posting_sources").await, 1);
        let updated_row = sqlx::query(
            "SELECT source_name_snapshot, posting_meta_json
             FROM job_posting_sources",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            updated_row.get::<String, _>("source_name_snapshot"),
            "SCHOTT AG Careers"
        );
        assert_eq!(
            updated_row.get::<String, _>("posting_meta_json"),
            json!({ "jobId": "new-99" }).to_string()
        );

        let listed = JobPostingService::new(&pool).list().await.unwrap();
        let listed_json = serde_json::to_value(&listed).unwrap();
        assert!(listed_json[0]["sources"][0].get("postingMeta").is_none());
        assert!(listed_json[0]["primarySource"].get("postingMeta").is_none());
    });
}

#[test]
fn distinct_findings_with_same_listing_url_but_different_posting_meta_stay_separate() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let result = search_run_result(vec![
            posting(
                "Laser Engineer",
                "ACME GmbH",
                &["Mainz"],
                vec![source_with_meta(
                    "career_page",
                    "ACME Careers",
                    "https://example.test/jobs",
                    [("jobId", "laser-42")],
                )],
            ),
            posting(
                "Data Engineer",
                "ACME GmbH",
                &["Berlin"],
                vec![source_with_meta(
                    "career_page",
                    "ACME Careers",
                    "https://example.test/jobs",
                    [("jobId", "data-99")],
                )],
            ),
        ]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        assert_eq!(table_count(&pool, "job_postings").await, 2);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 2);

        let rows = sqlx::query(
            "SELECT p.title, s.url, s.posting_meta_json
             FROM job_postings p
             JOIN job_posting_sources s ON s.posting_id = p.id
             ORDER BY p.title",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows[0].get::<String, _>("title"), "Data Engineer");
        assert_eq!(rows[0].get::<String, _>("url"), "https://example.test/jobs");
        assert_eq!(
            rows[0].get::<String, _>("posting_meta_json"),
            json!({ "jobId": "data-99" }).to_string()
        );
        assert_eq!(rows[1].get::<String, _>("title"), "Laser Engineer");
        assert_eq!(rows[1].get::<String, _>("url"), "https://example.test/jobs");
        assert_eq!(
            rows[1].get::<String, _>("posting_meta_json"),
            json!({ "jobId": "laser-42" }).to_string()
        );
    });
}

#[test]
fn exact_url_match_reuses_existing_posting_and_preserves_manual_states() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let existing_posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Original Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "in_progress",
                application_state: "submitted",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            existing_posting_id,
            "schott_ag",
            "Old SCHOTT Name",
            "https://example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, existing_posting_id, source_id).await;
        let result = search_run_result(vec![posting(
            "Updated Laser Engineer",
            "ACME GmbH Updated",
            &["Mainz"],
            vec![source(
                "schott_ag",
                "SCHOTT AG",
                "https://example.test/jobs/laser",
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        assert_eq!(table_count(&pool, "job_postings").await, 1);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 1);
        let posting_row = sqlx::query(
            "SELECT id, title, company, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at
             FROM job_postings",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(posting_row.get::<i64, _>("id"), existing_posting_id);
        assert_eq!(
            posting_row.get::<String, _>("title"),
            "Original Laser Engineer"
        );
        assert_eq!(posting_row.get::<String, _>("company"), "ACME GmbH");
        assert_eq!(posting_row.get::<i64, _>("primary_source_id"), source_id);
        assert_eq!(posting_row.get::<String, _>("read_state"), "read");
        assert_eq!(posting_row.get::<String, _>("interest_state"), "interested");
        assert_eq!(
            posting_row.get::<String, _>("preparation_state"),
            "in_progress"
        );
        assert_eq!(
            posting_row.get::<String, _>("application_state"),
            "submitted"
        );
        assert_eq!(
            posting_row.get::<String, _>("first_seen_at"),
            "2026-06-01T00:00:00.000Z"
        );
        assert_eq!(
            posting_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );
    });
}

#[test]
fn company_title_location_dedupe_reuses_existing_posting_and_adds_source_row() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let existing_posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Head Of Laser & Post Processing Development (mwd) RP/1205240901",
                company: "SCHOTT AG",
                locations: &["Mainz"],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
        let primary_source_id = insert_existing_source(
            &pool,
            existing_posting_id,
            "schott_ag",
            "SCHOTT AG",
            "https://join.schott.com/job/Mainz-Head-of-Laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, existing_posting_id, primary_source_id).await;
        let result = search_run_result(vec![posting(
            "Head of Laser & Post Processing Development (m/w/d)*",
            "SCHOTT AG",
            &["Mainz"],
            vec![source(
                "stepstone_de",
                "StepStone Deutschland",
                "https://www.stepstone.de/stellenangebote--head-of-laser.html",
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        assert_eq!(table_count(&pool, "job_postings").await, 1);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 2);
        let source_row = sqlx::query(
            "SELECT posting_id, source_key, source_name_snapshot, url, first_seen_at, last_seen_at
             FROM job_posting_sources
             WHERE source_key = 'stepstone_de'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(source_row.get::<i64, _>("posting_id"), existing_posting_id);
        assert_eq!(
            source_row.get::<String, _>("source_name_snapshot"),
            "StepStone Deutschland"
        );
        assert_eq!(
            source_row.get::<String, _>("url"),
            "https://www.stepstone.de/stellenangebote--head-of-laser.html"
        );
        assert_eq!(
            source_row.get::<String, _>("first_seen_at"),
            result.generated_at
        );
        assert_eq!(
            source_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );

        let posting_row = sqlx::query(
            "SELECT title, company, primary_source_id, last_seen_at
             FROM job_postings",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            posting_row.get::<String, _>("title"),
            "Head Of Laser & Post Processing Development (mwd) RP/1205240901"
        );
        assert_eq!(posting_row.get::<String, _>("company"), "SCHOTT AG");
        assert_eq!(
            posting_row.get::<i64, _>("primary_source_id"),
            primary_source_id
        );
        assert_eq!(
            posting_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );
    });
}

#[test]
fn existing_posting_locations_are_merged_additively() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let existing_posting_id = insert_existing_posting(
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
                last_seen_at: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            existing_posting_id,
            "schott_ag",
            "SCHOTT AG",
            "https://example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, existing_posting_id, source_id).await;
        let result = search_run_result(vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["mainz", "Wiesbaden"],
            vec![source(
                "schott_ag",
                "SCHOTT AG",
                "https://example.test/jobs/laser",
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        let posting_row = sqlx::query("SELECT locations_json FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(locations_from_row(&posting_row), vec!["Mainz", "Wiesbaden"]);
    });
}

#[test]
fn source_snapshot_and_last_seen_update_when_source_row_is_seen_again() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let existing_posting_id = insert_existing_posting(
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
                last_seen_at: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
        let source_id = insert_existing_source(
            &pool,
            existing_posting_id,
            "schott_ag",
            "Old Source Name",
            "https://example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, existing_posting_id, source_id).await;
        let result = search_run_result(vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source(
                "schott_ag",
                "New Source Name",
                "https://example.test/jobs/laser",
            )],
        )]);

        JobPostingImportService::new(&pool)
            .import_search_run_result(&result)
            .await
            .unwrap();

        let source_row = sqlx::query(
            "SELECT source_name_snapshot, first_seen_at, last_seen_at
             FROM job_posting_sources
             WHERE id = ?1",
        )
        .bind(source_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            source_row.get::<String, _>("source_name_snapshot"),
            "New Source Name"
        );
        assert_eq!(
            source_row.get::<String, _>("first_seen_at"),
            "2026-06-01T00:00:00.000Z"
        );
        assert_eq!(
            source_row.get::<String, _>("last_seen_at"),
            result.generated_at
        );
    });
}

#[test]
fn invariant_violations_fail_and_roll_back_the_import() {
    tauri::async_runtime::block_on(async {
        let cases = vec![
            (
                "missing source",
                posting("Laser Engineer", "ACME GmbH", &["Mainz"], vec![]),
                "no sources",
            ),
            (
                "empty title",
                posting(
                    " ",
                    "ACME GmbH",
                    &["Mainz"],
                    vec![source(
                        "schott_ag",
                        "SCHOTT AG",
                        "https://example.test/jobs/empty-title",
                    )],
                ),
                "title is empty",
            ),
            (
                "empty company",
                posting(
                    "Laser Engineer",
                    " ",
                    &["Mainz"],
                    vec![source(
                        "schott_ag",
                        "SCHOTT AG",
                        "https://example.test/jobs/empty-company",
                    )],
                ),
                "company is empty",
            ),
            (
                "empty source url",
                posting(
                    "Laser Engineer",
                    "ACME GmbH",
                    &["Mainz"],
                    vec![source("schott_ag", "SCHOTT AG", " ")],
                ),
                "source url is empty",
            ),
        ];

        for (case_name, invalid_posting, expected_error) in cases {
            let pool = migrated_pool().await;
            let result = search_run_result(vec![
                posting(
                    "Valid Posting",
                    "ACME GmbH",
                    &["Mainz"],
                    vec![source(
                        "schott_ag",
                        "SCHOTT AG",
                        "https://example.test/jobs/valid",
                    )],
                ),
                invalid_posting,
            ]);

            let error = JobPostingImportService::new(&pool)
                .import_search_run_result(&result)
                .await
                .unwrap_err();

            assert!(
                error.contains(expected_error),
                "{case_name}: expected error containing `{expected_error}`, got `{error}`"
            );
            assert_eq!(
                table_count(&pool, "job_postings").await,
                0,
                "{case_name}: job_postings should be rolled back"
            );
            assert_eq!(
                table_count(&pool, "job_posting_sources").await,
                0,
                "{case_name}: job_posting_sources should be rolled back"
            );
        }
    });
}
