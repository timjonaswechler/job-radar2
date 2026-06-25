use super::*;
use crate::search::run::{
    NormalizedPosting, PostingSource, SearchRunResult, SearchRunStatus, SourceRunResult,
};
use serde_json::from_str;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};

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
                    first_seen_at, last_seen_at
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

#[test]
fn lists_persisted_postings_with_locations_primary_source_and_sources() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz", "Remote"],
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
            "schott_ag",
            "SCHOTT AG",
            "https://example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        let second_source_id = insert_existing_source(
            &pool,
            posting_id,
            "stepstone_de",
            "StepStone Deutschland",
            "https://stepstone.example.test/jobs/laser",
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;

        let postings = JobPostingService::new(&pool).list().await.unwrap();

        assert_eq!(postings.len(), 1);
        let posting = &postings[0];
        assert_eq!(posting.id, posting_id);
        assert_eq!(posting.title, "Laser Engineer");
        assert_eq!(posting.company, "ACME GmbH");
        assert_eq!(posting.locations, vec!["Mainz", "Remote"]);
        assert_eq!(posting.read_state, ReadState::Unread);
        assert_eq!(posting.interest_state, InterestState::Undecided);
        assert_eq!(posting.preparation_state, PreparationState::NotStarted);
        assert_eq!(posting.application_state, ApplicationState::NotApplied);
        assert_eq!(posting.first_seen_at, "2026-06-01T00:00:00.000Z");
        assert_eq!(posting.last_seen_at, "2026-06-23T21:41:36.000Z");
        assert!(!posting.created_at.is_empty());
        assert!(!posting.updated_at.is_empty());

        assert_eq!(
            posting.primary_source.as_ref().unwrap().id,
            primary_source_id
        );
        assert_eq!(posting.sources.len(), 2);
        assert_eq!(posting.sources[0].id, primary_source_id);
        assert_eq!(posting.sources[0].source_key, "schott_ag");
        assert_eq!(posting.sources[0].source_name_snapshot, "SCHOTT AG");
        assert_eq!(posting.sources[0].url, "https://example.test/jobs/laser");
        assert_eq!(posting.sources[1].id, second_source_id);
        assert_eq!(posting.sources[1].source_key, "stepstone_de");
    });
}

#[test]
fn lists_persisted_postings_by_last_seen_desc_then_id_desc() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let oldest_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Oldest Posting",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
        let newer_lower_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Newer Lower ID",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        let newer_higher_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Newer Higher ID",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;

        let ids = JobPostingService::new(&pool)
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|posting| posting.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec![newer_higher_id, newer_lower_id, oldest_id]);
    });
}

#[test]
fn queue_counts_use_mailbox_workflow_mapping() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "New Inbox",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Review Inbox",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Interesting",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Preparing",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "in_progress",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Ready",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "ready",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Submitted",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "not_started",
                application_state: "submitted",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "In Process Undecided",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "in_process",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Dismissed",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "dismissed",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Rejected",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "interested",
                preparation_state: "not_started",
                application_state: "rejected_by_company",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;

        let counts = JobPostingService::new(&pool).queue_counts().await.unwrap();

        assert_eq!(counts.all, 9);
        assert_eq!(counts.inbox, 2);
        assert_eq!(counts.new_inbox, 1);
        assert_eq!(counts.review_inbox, 1);
        assert_eq!(counts.interested, 1);
        assert_eq!(counts.preparation, 2);
        assert_eq!(counts.applied, 2);
        assert_eq!(counts.archive, 2);
    });
}

#[test]
fn lists_postings_for_queue_with_same_mailbox_workflow_mapping() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Inbox",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Applied",
                company: "ACME GmbH",
                locations: &[],
                read_state: "unread",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "submitted",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Archive",
                company: "ACME GmbH",
                locations: &[],
                read_state: "read",
                interest_state: "dismissed",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;

        let inbox_titles = JobPostingService::new(&pool)
            .list_for_queue(JobPostingQueueId::Inbox)
            .await
            .unwrap()
            .into_iter()
            .map(|posting| posting.title)
            .collect::<Vec<_>>();
        let applied_titles = JobPostingService::new(&pool)
            .list_for_queue(JobPostingQueueId::Applied)
            .await
            .unwrap()
            .into_iter()
            .map(|posting| posting.title)
            .collect::<Vec<_>>();
        let archive_titles = JobPostingService::new(&pool)
            .list_for_queue(JobPostingQueueId::Archive)
            .await
            .unwrap()
            .into_iter()
            .map(|posting| posting.title)
            .collect::<Vec<_>>();

        assert_eq!(inbox_titles, vec!["Inbox"]);
        assert_eq!(applied_titles, vec!["Applied"]);
        assert_eq!(archive_titles, vec!["Archive"]);
    });
}

#[test]
fn partial_state_update_changes_only_supplied_state_fields() {
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

        let updated = JobPostingService::new(&pool)
            .update_state(
                posting_id,
                UpdateJobPostingStateInput {
                    read_state: Some(ReadState::Read),
                    application_state: Some(ApplicationState::Submitted),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.read_state, ReadState::Read);
        assert_eq!(updated.interest_state, InterestState::Undecided);
        assert_eq!(updated.preparation_state, PreparationState::NotStarted);
        assert_eq!(updated.application_state, ApplicationState::Submitted);

        let persisted = JobPostingService::new(&pool)
            .list()
            .await
            .unwrap()
            .remove(0);
        assert_eq!(persisted.read_state, ReadState::Read);
        assert_eq!(persisted.interest_state, InterestState::Undecided);
        assert_eq!(persisted.preparation_state, PreparationState::NotStarted);
        assert_eq!(persisted.application_state, ApplicationState::Submitted);
    });
}

#[test]
fn state_update_changes_updated_at() {
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
        let old_updated_at = "2026-06-01T00:00:00.000Z";
        sqlx::query("UPDATE job_postings SET updated_at = ?1 WHERE id = ?2")
            .bind(old_updated_at)
            .bind(posting_id)
            .execute(&pool)
            .await
            .unwrap();

        let updated = JobPostingService::new(&pool)
            .update_state(
                posting_id,
                UpdateJobPostingStateInput {
                    interest_state: Some(InterestState::Interested),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_ne!(updated.updated_at, old_updated_at);
    });
}

#[test]
fn state_update_does_not_change_sources_or_seen_fields() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let posting_id = insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz", "Remote"],
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
            "schott_ag",
            "SCHOTT AG",
            "https://example.test/jobs/laser",
            "2026-06-01T00:00:00.000Z",
        )
        .await;
        insert_existing_source(
            &pool,
            posting_id,
            "stepstone_de",
            "StepStone Deutschland",
            "https://stepstone.example.test/jobs/laser",
            "2026-06-02T00:00:00.000Z",
        )
        .await;
        set_primary_source(&pool, posting_id, primary_source_id).await;
        let before = JobPostingService::new(&pool)
            .list()
            .await
            .unwrap()
            .remove(0);

        let after = JobPostingService::new(&pool)
            .update_state(
                posting_id,
                UpdateJobPostingStateInput {
                    preparation_state: Some(PreparationState::Ready),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(after.title, before.title);
        assert_eq!(after.company, before.company);
        assert_eq!(after.locations, before.locations);
        assert_eq!(after.first_seen_at, before.first_seen_at);
        assert_eq!(after.last_seen_at, before.last_seen_at);
        assert_eq!(after.created_at, before.created_at);
        assert_eq!(after.primary_source, before.primary_source);
        assert_eq!(after.sources, before.sources);
    });
}

#[test]
fn state_update_for_missing_posting_returns_clear_error() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;

        let error = JobPostingService::new(&pool)
            .update_state(
                42,
                UpdateJobPostingStateInput {
                    read_state: Some(ReadState::Read),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();

        assert_eq!(error, "job posting 42 not found");
    });
}

#[test]
fn invalid_persisted_state_values_fail_instead_of_deserializing_silently() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&pool)
            .await
            .unwrap();
        insert_existing_posting(
            &pool,
            ExistingPosting {
                title: "Laser Engineer",
                company: "ACME GmbH",
                locations: &["Mainz"],
                read_state: "archived",
                interest_state: "undecided",
                preparation_state: "not_started",
                application_state: "not_applied",
                first_seen_at: "2026-06-01T00:00:00.000Z",
                last_seen_at: "2026-06-23T21:41:36.000Z",
            },
        )
        .await;
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&pool)
            .await
            .unwrap();

        let error = JobPostingService::new(&pool).list().await.unwrap_err();

        assert_eq!(error, "unknown job posting read state: archived");
    });
}

#[test]
fn empty_state_update_returns_clear_error() {
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

        let error = JobPostingService::new(&pool)
            .update_state(posting_id, UpdateJobPostingStateInput::default())
            .await
            .unwrap_err();

        assert_eq!(error, "no state fields supplied");
    });
}

#[test]
fn invalid_persisted_locations_fail_with_posting_context() {
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
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE job_postings SET locations_json = 'not-json' WHERE id = ?1")
            .bind(posting_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&pool)
            .await
            .unwrap();

        let error = JobPostingService::new(&pool).list().await.unwrap_err();

        assert!(
            error.contains(&format!(
                "invalid locations_json for job posting {posting_id}"
            )),
            "unexpected error: {error}"
        );
    });
}

fn search_run_result(postings: Vec<NormalizedPosting>) -> SearchRunResult {
    SearchRunResult {
        search_request_id: 1,
        status: SearchRunStatus::Completed,
        generated_at: "2026-06-23T21:41:36.000Z".to_string(),
        source_runs: Vec::<SourceRunResult>::new(),
        postings,
    }
}

fn posting(
    title: &str,
    company: &str,
    locations: &[&str],
    sources: Vec<PostingSource>,
) -> NormalizedPosting {
    NormalizedPosting {
        title: title.to_string(),
        company: company.to_string(),
        url: sources
            .first()
            .map(|source| source.url.clone())
            .unwrap_or_default(),
        locations: locations
            .iter()
            .map(|location| (*location).to_string())
            .collect(),
        sources,
    }
}

fn source(source_key: &str, source_name: &str, url: &str) -> PostingSource {
    PostingSource {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        url: url.to_string(),
    }
}

fn locations_from_row(row: &sqlx::sqlite::SqliteRow) -> Vec<String> {
    from_str::<Vec<String>>(&row.get::<String, _>("locations_json")).unwrap()
}

struct ExistingPosting<'a> {
    title: &'a str,
    company: &'a str,
    locations: &'a [&'a str],
    read_state: &'a str,
    interest_state: &'a str,
    preparation_state: &'a str,
    application_state: &'a str,
    first_seen_at: &'a str,
    last_seen_at: &'a str,
}

async fn insert_existing_posting(pool: &SqlitePool, posting: ExistingPosting<'_>) -> i64 {
    let locations_json = serde_json::to_string(&posting.locations).unwrap();
    sqlx::query(
        "INSERT INTO job_postings (
           title, company, locations_json,
           read_state, interest_state, preparation_state, application_state,
           first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(posting.title)
    .bind(posting.company)
    .bind(locations_json)
    .bind(posting.read_state)
    .bind(posting.interest_state)
    .bind(posting.preparation_state)
    .bind(posting.application_state)
    .bind(posting.first_seen_at)
    .bind(posting.last_seen_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_existing_source(
    pool: &SqlitePool,
    posting_id: i64,
    source_key: &str,
    source_name_snapshot: &str,
    url: &str,
    seen_at: &str,
) -> i64 {
    sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, source_name_snapshot, url, first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
    )
    .bind(posting_id)
    .bind(source_key)
    .bind(source_name_snapshot)
    .bind(url)
    .bind(seen_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn set_primary_source(pool: &SqlitePool, posting_id: i64, source_id: i64) {
    sqlx::query("UPDATE job_postings SET primary_source_id = ?1 WHERE id = ?2")
        .bind(source_id)
        .bind(posting_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn table_count(pool: &SqlitePool, table_name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table_name}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn migrated_pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    pool
}
