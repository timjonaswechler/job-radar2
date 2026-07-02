use super::*;

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
