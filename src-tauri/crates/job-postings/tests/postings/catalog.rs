use job_postings::catalog::{ApplicationState, Error, InterestState, PreparationState, ReadState};
use job_postings::{Catalog, Change, Id, Queue};

use super::support::{insert_posting, insert_source, pool, Posting};

#[tokio::test]
async fn catalog_lists_all_in_order_with_batched_sources_and_primary_queue() {
    let pool = pool().await;
    let (older, primary) = insert_posting(
        &pool,
        Posting {
            title: "Older",
            read: "unread",
            interest: "undecided",
            preparation: "not_started",
            application: "not_applied",
            last_seen: "2026-06-01T00:00:00.000Z",
        },
    )
    .await;
    let secondary = insert_source(
        &pool,
        older,
        "secondary",
        "https://secondary.test/jobs/older",
        false,
        Default::default(),
    )
    .await;
    let (newer, _) = insert_posting(
        &pool,
        Posting {
            title: "Newer",
            read: "read",
            interest: "interested",
            preparation: "ready",
            application: "not_applied",
            last_seen: "2026-06-02T00:00:00.000Z",
        },
    )
    .await;
    let (newer_tie, _) = insert_posting(
        &pool,
        Posting {
            title: "NewerTie",
            read: "read",
            interest: "interested",
            preparation: "ready",
            application: "not_applied",
            last_seen: "2026-06-02T00:00:00.000Z",
        },
    )
    .await;

    let postings = Catalog::new(pool).list(Queue::All).await.unwrap();

    assert_eq!(
        postings
            .iter()
            .map(|posting| posting.id.get())
            .collect::<Vec<_>>(),
        vec![newer_tie, newer, older]
    );
    assert_eq!(postings[0].primary_queue, Queue::Preparation);
    assert_eq!(
        serde_json::to_value(&postings[0]).unwrap()["primaryQueue"],
        "preparation"
    );
    assert_eq!(postings[2].primary_queue, Queue::Inbox);
    assert_eq!(postings[2].primary_source.id, primary);
    assert_eq!(
        postings[2]
            .sources
            .iter()
            .map(|source| source.id)
            .collect::<Vec<_>>(),
        vec![primary, secondary]
    );
}

#[tokio::test]
async fn catalog_preserves_queue_membership_counts_and_subcategories() {
    let pool = pool().await;
    let cases = [
        (
            "New",
            "unread",
            "undecided",
            "not_started",
            "not_applied",
            Queue::Inbox,
        ),
        (
            "Review",
            "read",
            "undecided",
            "not_started",
            "not_applied",
            Queue::Inbox,
        ),
        (
            "Interested",
            "read",
            "interested",
            "not_started",
            "not_applied",
            Queue::Interested,
        ),
        (
            "Preparing",
            "read",
            "interested",
            "in_progress",
            "not_applied",
            Queue::Preparation,
        ),
        (
            "Ready",
            "read",
            "interested",
            "ready",
            "not_applied",
            Queue::Preparation,
        ),
        (
            "Submitted",
            "read",
            "interested",
            "not_started",
            "submitted",
            Queue::Applied,
        ),
        (
            "InProcess",
            "unread",
            "undecided",
            "not_started",
            "in_process",
            Queue::Applied,
        ),
        (
            "Dismissed",
            "read",
            "dismissed",
            "not_started",
            "not_applied",
            Queue::Archive,
        ),
        (
            "Rejected",
            "read",
            "interested",
            "not_started",
            "rejected_by_company",
            Queue::Archive,
        ),
    ];
    for (title, read, interest, preparation, application, _) in cases {
        insert_posting(
            &pool,
            Posting {
                title,
                read,
                interest,
                preparation,
                application,
                last_seen: "2026-06-01T00:00:00.000Z",
            },
        )
        .await;
    }
    let catalog = Catalog::new(pool);

    let counts = catalog.counts().await.unwrap();
    assert_eq!(
        (
            counts.all,
            counts.inbox,
            counts.new_inbox,
            counts.review_inbox
        ),
        (9, 2, 1, 1)
    );
    assert_eq!(
        (
            counts.interested,
            counts.preparation,
            counts.applied,
            counts.archive
        ),
        (1, 2, 2, 2)
    );
    for queue in [
        Queue::Inbox,
        Queue::Interested,
        Queue::Preparation,
        Queue::Applied,
        Queue::Archive,
    ] {
        let listed = catalog.list(queue).await.unwrap();
        assert!(listed.iter().all(|posting| posting.primary_queue == queue));
        assert_eq!(
            listed.len() as i64,
            match queue {
                Queue::Inbox | Queue::Preparation | Queue::Applied | Queue::Archive => 2,
                Queue::Interested => 1,
                Queue::All => unreachable!(),
            }
        );
    }
}

#[tokio::test]
async fn catalog_changes_only_supplied_workflow_axes() {
    let pool = pool().await;
    let (id, _) = insert_posting(
        &pool,
        Posting {
            title: "Workflow",
            read: "unread",
            interest: "undecided",
            preparation: "not_started",
            application: "not_applied",
            last_seen: "2026-06-01T00:00:00.000Z",
        },
    )
    .await;
    let catalog = Catalog::new(pool);
    let change = Change::new(
        Some(ReadState::Read),
        None,
        Some(PreparationState::Ready),
        None,
    )
    .unwrap();

    let posting = catalog.change(Id::new(id), change).await.unwrap();

    assert_eq!(posting.read_state, ReadState::Read);
    assert_eq!(posting.interest_state, InterestState::Undecided);
    assert_eq!(posting.preparation_state, PreparationState::Ready);
    assert_eq!(posting.application_state, ApplicationState::NotApplied);
    assert_eq!(posting.primary_queue, Queue::Inbox);
}

#[tokio::test]
async fn catalog_rejects_empty_change_and_reports_missing_posting() {
    assert!(matches!(
        Change::new(None, None, None, None),
        Err(Error::InvalidChange)
    ));
    let catalog = Catalog::new(pool().await);
    let error = catalog
        .change(
            Id::new(42),
            Change::new(None, Some(InterestState::Interested), None, None).unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::NotFound(id) if id == Id::new(42)));
}

#[tokio::test]
async fn catalog_rejects_missing_primary_and_corrupt_workflow_values() {
    let pool = pool().await;
    let (missing_primary, source) = insert_posting(
        &pool,
        Posting {
            title: "MissingPrimary",
            read: "unread",
            interest: "undecided",
            preparation: "not_started",
            application: "not_applied",
            last_seen: "2026-06-01T00:00:00.000Z",
        },
    )
    .await;
    sqlx::query("UPDATE job_posting_sources SET is_primary = 0 WHERE id = ?1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    let catalog = Catalog::new(pool.clone());
    let error = catalog.list(Queue::All).await.unwrap_err();
    assert!(matches!(error, Error::Corrupt { posting: id, .. } if id == Id::new(missing_primary)));
    let error = catalog.counts().await.unwrap_err();
    assert!(matches!(error, Error::Corrupt { posting: id, .. } if id == Id::new(missing_primary)));

    sqlx::query("UPDATE job_posting_sources SET is_primary = 1 WHERE id = ?1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE job_postings SET read_state = 'archived' WHERE id = ?1")
        .bind(missing_primary)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        Catalog::new(pool).list(Queue::All).await.unwrap_err(),
        Error::Corrupt { posting: id, .. } if id == Id::new(missing_primary)
    ));
}

#[tokio::test]
async fn schema_prevents_two_primary_occurrences_for_one_posting() {
    let pool = pool().await;
    let (posting_id, _) = insert_posting(
        &pool,
        Posting {
            title: "OnePrimary",
            read: "unread",
            interest: "undecided",
            preparation: "not_started",
            application: "not_applied",
            last_seen: "2026-06-01T00:00:00.000Z",
        },
    )
    .await;
    let result = sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, identity_kind, identity_value, provider_url,
           source_name_snapshot, posting_meta_json, is_primary
         ) VALUES (?1, 'other', 'normalized_url', 'https://other.test/jobs/1',
                   'https://other.test/jobs/1', 'Other', '{}', 1)",
    )
    .bind(posting_id)
    .execute(&pool)
    .await;
    assert!(result.is_err());

    sqlx::query("DROP INDEX idx_job_posting_sources_one_primary")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, identity_kind, identity_value, provider_url,
           source_name_snapshot, posting_meta_json, is_primary
         ) VALUES (?1, 'other', 'normalized_url', 'https://other.test/jobs/1',
                   'https://other.test/jobs/1', 'Other', '{}', 1)",
    )
    .bind(posting_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        Catalog::new(pool).list(Queue::All).await.unwrap_err(),
        Error::Corrupt { posting, .. } if posting == Id::new(posting_id)
    ));
}
