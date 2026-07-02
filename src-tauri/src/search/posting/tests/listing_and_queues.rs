use super::*;

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
