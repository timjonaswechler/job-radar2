use search_requests::{Catalog, Error, Input, Status, ValidationIssueCode};
use search_resolution::{SearchRule, SearchRuleKind, SearchRuleTarget};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[tokio::test]
async fn create_list_get_update_delete_round_trip_preserves_order() {
    let pool = migrated_pool(5).await;
    let catalog = Catalog::new(pool.clone());
    let authored = Input {
        status: Status::Active,
        include_rules: vec![text_rule(" Physik "), regex_rule("Laser|Optik")],
        exclude_rules: vec![text_rule(" Praktikum "), regex_rule("Werkstudent|Student")],
        locations: vec![" Mainz ".into(), " Berlin ".into()],
        radius_km: Some(42),
        source_keys: vec!["first_source".into(), "second_source".into()],
    };
    let first = catalog.create(authored).await.unwrap();
    let second = catalog.create(valid_input(Status::Disabled)).await.unwrap();

    assert_eq!(first.status, Status::Active);
    assert_eq!(
        first.include_rules,
        vec![text_rule("Physik"), regex_rule("Laser|Optik")]
    );
    assert_eq!(
        first.exclude_rules,
        vec![text_rule("Praktikum"), regex_rule("Werkstudent|Student")]
    );
    assert_eq!(first.locations, vec!["Mainz", "Berlin"]);
    assert_eq!(first.radius_km, Some(42));
    assert_eq!(first.source_keys, vec!["first_source", "second_source"]);
    assert!(first.validation.is_valid());
    assert!(!first.created_at.is_empty());
    assert!(!first.updated_at.is_empty());
    assert_eq!(catalog.list().await.unwrap(), vec![first.clone(), second]);
    assert_eq!(catalog.get(first.id).await.unwrap(), first);

    let mut changed = valid_input(Status::Draft);
    changed.include_rules[0].value = " Laser ".into();
    changed.locations = vec![" Berlin ".into(), " ".into()];
    changed.source_keys.clear();
    let updated = catalog.update(first.id, changed).await.unwrap();
    assert_eq!(updated.include_rules, vec![text_rule("Laser")]);
    assert!(updated.exclude_rules.is_empty());
    assert_eq!(updated.locations, vec!["Berlin"]);
    assert_eq!(updated.radius_km, Some(30));
    assert!(updated.source_keys.is_empty());
    assert_eq!(updated.status, Status::Draft);
    assert_eq!(
        updated.validation.issues[0].code,
        ValidationIssueCode::SourceKeyRequired
    );

    catalog.delete(first.id).await.unwrap();
    assert_eq!(
        catalog.get(first.id).await,
        Err(Error::NotFound { id: first.id })
    );
}

#[tokio::test]
async fn lifecycle_and_validity_are_derived_while_invalid_active_is_rejected() {
    let catalog = Catalog::new(migrated_pool(5).await);
    let mut invalid = valid_input(Status::Draft);
    invalid.include_rules[0].kind = SearchRuleKind::Regex;
    invalid.include_rules[0].value = "[".into();
    invalid.source_keys.clear();
    let draft = catalog.create(invalid.clone()).await.unwrap();
    assert_eq!(
        draft
            .validation
            .issues
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>(),
        vec![
            ValidationIssueCode::InvalidRegex,
            ValidationIssueCode::SourceKeyRequired
        ]
    );

    invalid.status = Status::Disabled;
    assert_eq!(
        catalog.create(invalid.clone()).await.unwrap().status,
        Status::Disabled
    );
    invalid.status = Status::Active;
    assert!(
        matches!(catalog.create(invalid).await, Err(Error::InvalidInput { message }) if message.contains("invalid_regex at /includeRules/0/value"))
    );
}

#[tokio::test]
async fn ordered_duplicate_source_keys_are_preserved_and_derived_issues_are_bounded() {
    let catalog = Catalog::new(migrated_pool(5).await);
    let mut input = valid_input(Status::Draft);
    input.include_rules = (0..70)
        .map(|_| SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Regex,
            value: "[".into(),
        })
        .collect();
    input.source_keys = vec!["first".into(), "second".into(), "first".into()];
    let record = catalog.create(input).await.unwrap();
    assert_eq!(record.source_keys, vec!["first", "second", "first"]);
    assert_eq!(record.validation.issues.len(), 64);
    assert_eq!(
        record.validation.issues.last().unwrap().code,
        ValidationIssueCode::IssuesTruncated
    );

    let mut duplicate_only = valid_input(Status::Draft);
    duplicate_only.source_keys = vec!["first".into(), "first".into()];
    let record = catalog.update(record.id, duplicate_only).await.unwrap();
    assert_eq!(
        record.validation.issues[0].code,
        ValidationIssueCode::DuplicateSourceKey
    );
    assert_eq!(record.validation.issues[0].path, "/sourceKeys/1");
}

#[test]
fn id_json_deserialization_enforces_the_positive_identity_invariant() {
    assert_eq!(
        serde_json::from_str::<search_requests::Id>("42").unwrap(),
        search_requests::Id::new(42).unwrap()
    );
    assert!(serde_json::from_str::<search_requests::Id>("0").is_err());
    assert!(serde_json::from_str::<search_requests::Id>("-1").is_err());
}

#[tokio::test]
async fn input_normalization_rejects_empty_values_invalid_keys_unsafe_radius_and_ids() {
    let catalog = Catalog::new(migrated_pool(5).await);
    assert!(matches!(
        search_requests::Id::new(0),
        Err(Error::InvalidInput { message }) if message.contains("greater than 0")
    ));
    assert!(matches!(
        search_requests::Id::new(-1),
        Err(Error::InvalidInput { .. })
    ));
    let mut input = valid_input(Status::Draft);
    input.include_rules[0].value = " ".into();
    assert!(
        matches!(catalog.create(input).await, Err(Error::InvalidInput { message }) if message.contains("includeRules[0].value"))
    );
    let mut input = valid_input(Status::Draft);
    input.source_keys = vec!["Invalid-Key".into()];
    assert!(
        matches!(catalog.create(input).await, Err(Error::InvalidInput { message }) if message.contains("^[a-z0-9_]+$"))
    );
    let mut input = valid_input(Status::Draft);
    input.radius_km = Some(9_007_199_254_740_992);
    assert!(matches!(
        catalog.create(input).await,
        Err(Error::InvalidInput { .. })
    ));
}

#[tokio::test]
async fn errors_distinguish_not_found_corrupt_rows_and_unavailable_storage() {
    let pool = migrated_pool(5).await;
    let catalog = Catalog::new(pool.clone());
    let missing = search_requests::Id::new(404).unwrap();
    assert_eq!(
        catalog.get(missing).await,
        Err(Error::NotFound { id: missing })
    );
    let record = catalog.create(valid_input(Status::Draft)).await.unwrap();
    sqlx::query("UPDATE search_requests SET include_rules_json='{}' WHERE id=?1")
        .bind(record.id.get())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        matches!(catalog.get(record.id).await, Err(Error::CorruptStoredRow { id: Some(id), .. }) if id == record.id)
    );

    let corrupt_status = catalog.create(valid_input(Status::Draft)).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("UPDATE search_requests SET status='unknown' WHERE id=?1")
        .bind(corrupt_status.id.get())
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    assert!(
        matches!(catalog.get(corrupt_status.id).await, Err(Error::CorruptStoredRow { id: Some(id), .. }) if id == corrupt_status.id)
    );

    pool.close().await;
    assert!(matches!(
        catalog.list().await,
        Err(Error::StorageUnavailable { .. })
    ));
}

#[tokio::test]
async fn execution_is_an_immutable_snapshot_and_exclusively_leased_until_drop() {
    let pool = migrated_pool(5).await;
    let catalog = Catalog::new(pool.clone());
    let record = catalog.create(valid_input(Status::Active)).await.unwrap();
    let execution = catalog.begin_execution(record.id).await.unwrap();
    assert_eq!(execution.snapshot(), &record);
    assert!(
        matches!(catalog.begin_execution(record.id).await, Err(Error::Busy { id }) if id == record.id)
    );
    assert_eq!(
        catalog
            .update(record.id, valid_input(Status::Active))
            .await
            .unwrap_err(),
        Error::Busy { id: record.id }
    );
    assert_eq!(
        catalog.delete(record.id).await.unwrap_err(),
        Error::Busy { id: record.id }
    );
    sqlx::query("UPDATE search_requests SET locations_json='[\"Elsewhere\"]' WHERE id=?1")
        .bind(record.id.get())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(execution.snapshot().locations, vec!["Mainz"]);
    drop(execution);
    catalog.delete(record.id).await.unwrap();
}

#[tokio::test]
async fn failed_admission_and_failed_mutation_release_the_activity_reservation() {
    let catalog = Catalog::new(migrated_pool(5).await);
    let mut draft_input = valid_input(Status::Draft);
    draft_input.source_keys.clear();
    let draft = catalog.create(draft_input).await.unwrap();
    assert!(matches!(
        catalog.begin_execution(draft.id).await,
        Err(Error::InvalidInput { .. })
    ));
    catalog.delete(draft.id).await.unwrap();

    let active = catalog.create(valid_input(Status::Active)).await.unwrap();
    let mut invalid = valid_input(Status::Active);
    invalid.include_rules.clear();
    assert!(matches!(
        catalog.update(active.id, invalid).await,
        Err(Error::InvalidInput { .. })
    ));
    assert!(catalog.begin_execution(active.id).await.is_ok());
}

#[tokio::test]
async fn mutation_reservation_wins_a_concurrent_execution_race_before_sql_await() {
    let pool = migrated_pool(1).await;
    let catalog = Catalog::new(pool.clone());
    let record = catalog.create(valid_input(Status::Active)).await.unwrap();
    let transaction = pool.begin().await.unwrap();
    let updating = {
        let catalog = catalog.clone();
        tokio::spawn(async move { catalog.update(record.id, valid_input(Status::Active)).await })
    };
    tokio::task::yield_now().await;
    assert!(
        matches!(catalog.begin_execution(record.id).await, Err(Error::Busy { id }) if id == record.id)
    );
    transaction.rollback().await.unwrap();
    updating.await.unwrap().unwrap();
    assert!(catalog.begin_execution(record.id).await.is_ok());
}

#[tokio::test]
async fn delete_cascades_runs_and_matches_but_preserves_job_postings() {
    let pool = migrated_pool(5).await;
    let catalog = Catalog::new(pool.clone());
    let request = catalog.create(valid_input(Status::Active)).await.unwrap();
    let posting_id =
        sqlx::query("INSERT INTO job_postings (title, company) VALUES ('Engineer','ACME')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
    let run_id = sqlx::query("INSERT INTO search_runs (search_request_id,status,generated_at) VALUES (?1,'completed','now')").bind(request.id.get()).execute(&pool).await.unwrap().last_insert_rowid();
    sqlx::query("INSERT INTO matches (search_run_id,job_posting_id) VALUES (?1,?2)")
        .bind(run_id)
        .bind(posting_id)
        .execute(&pool)
        .await
        .unwrap();
    catalog.delete(request.id).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matches")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn catalog_uses_the_real_migrated_search_request_schema_without_latest_projection() {
    let pool = migrated_pool(5).await;
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('search_requests') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(columns.contains(&"last_run_at".into()));
    let catalog = Catalog::new(pool.clone());
    let record = catalog.create(valid_input(Status::Active)).await.unwrap();
    sqlx::query("UPDATE search_requests SET last_run_at='preserved', last_run_status='completed' WHERE id=?1").bind(record.id.get()).execute(&pool).await.unwrap();
    catalog
        .update(record.id, valid_input(Status::Disabled))
        .await
        .unwrap();
    let latest: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT last_run_at,last_run_status FROM search_requests WHERE id=?1")
            .bind(record.id.get())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(latest, (Some("preserved".into()), Some("completed".into())));
    let json = serde_json::to_value(catalog.get(record.id).await.unwrap()).unwrap();
    assert!(json.get("lastRunAt").is_none());
    assert!(json.get("lastRunStatus").is_none());
}

fn text_rule(value: &str) -> SearchRule {
    SearchRule {
        target: SearchRuleTarget::Title,
        kind: SearchRuleKind::Text,
        value: value.into(),
    }
}

fn regex_rule(value: &str) -> SearchRule {
    SearchRule {
        target: SearchRuleTarget::Title,
        kind: SearchRuleKind::Regex,
        value: value.into(),
    }
}

fn valid_input(status: Status) -> Input {
    Input {
        status,
        include_rules: vec![text_rule(" Physik ")],
        exclude_rules: vec![],
        locations: vec![" Mainz ".into()],
        radius_km: Some(30),
        source_keys: vec!["fixture_source".into()],
    }
}

async fn migrated_pool(max_connections: u32) -> sqlx::SqlitePool {
    let (_, database_path) = tempfile::NamedTempFile::new().unwrap().keep().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    pool
}
