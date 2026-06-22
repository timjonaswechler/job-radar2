use super::*;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};

#[test]
fn search_request_crud_round_trips_without_name() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);
        let source_key = "indeed_de".to_string();

        let created = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule(" Physik ")],
                exclude_rules: vec![text_rule("Praktikum")],
                locations: vec![" Mainz ".to_string(), "".to_string()],
                radius_km: Some(30),
                source_keys: vec![source_key.clone()],
            })
            .await
            .unwrap();

        assert_eq!(created.status, SearchRequestStatus::Active);
        assert_eq!(created.include_rules[0].value, "Physik");
        assert_eq!(created.exclude_rules[0].value, "Praktikum");
        assert_eq!(created.locations, vec!["Mainz"]);
        assert_eq!(created.radius_km, Some(30));
        assert_eq!(created.source_keys, vec![source_key]);
        assert!(created.validation_error.is_none());
        assert!(!created.created_at.is_empty());
        assert!(!created.updated_at.is_empty());
        let created_json = serde_json::to_value(&created).unwrap();
        assert!(created_json.get("name").is_none());
        assert_eq!(created_json["sourceKeys"], serde_json::json!(["indeed_de"]));
        assert!(created_json.get("sourceIds").is_none());

        let persisted_source_keys_json: String =
            sqlx::query_scalar("SELECT source_keys_json FROM search_requests WHERE id = ?1")
                .bind(created.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(persisted_source_keys_json, "[\"indeed_de\"]");

        let listed = service.list().await.unwrap();
        assert_eq!(listed, vec![created.clone()]);
        assert_eq!(service.get(created.id).await.unwrap(), created);

        let updated = service
            .update(
                created.id,
                UpdateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![regex_rule("Laser|Optik")],
                    exclude_rules: vec![],
                    locations: vec!["Berlin".to_string()],
                    radius_km: None,
                    source_keys: vec![],
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.status, SearchRequestStatus::Draft);
        assert_eq!(updated.include_rules[0].kind, SearchRuleKind::Regex);
        assert_eq!(updated.locations, vec!["Berlin"]);
        assert_eq!(updated.radius_km, None);
        assert!(updated.source_keys.is_empty());

        service.delete(created.id).await.unwrap();
        assert!(service.get(created.id).await.is_err());
    });
}

#[test]
fn invalid_regex_is_persisted_as_validation_error_only_when_not_active() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);
        let source_key = "indeed_de".to_string();

        let draft = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![regex_rule("[")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap();
        assert_eq!(draft.status, SearchRequestStatus::Draft);
        assert!(draft
            .validation_error
            .as_deref()
            .unwrap()
            .contains("includeRules[0].value is invalid regex"));

        let active_error = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![regex_rule("[")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![source_key.clone()],
            })
            .await
            .unwrap_err();
        assert!(active_error.contains("validationError"));
        assert!(active_error.contains("invalid regex"));
    });
}

#[test]
fn search_requests_do_not_query_removed_source_domain_table() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);

        let created = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Physik")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["missing_source".to_string()],
            })
            .await
            .unwrap();

        assert_eq!(created.source_keys, vec!["missing_source"]);
        assert_eq!(service.list().await.unwrap(), vec![created]);

        let invalid_source_key = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![text_rule("Physik")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["Invalid-Key".to_string()],
            })
            .await
            .unwrap_err();
        assert!(invalid_source_key.contains("sourceKeys[0] must match ^[a-z0-9_]+$"));
    });
}

#[test]
fn active_search_requests_require_include_rule_and_source_key() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);
        let source_key = "indeed_de".to_string();

        let missing_rule = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![source_key.clone()],
            })
            .await
            .unwrap_err();
        assert!(missing_rule.contains("at least one include rule"));

        let missing_source = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Physik")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap_err();
        assert!(missing_source.contains("at least one sourceKey"));
    });
}

#[test]
fn unsupported_rules_and_empty_rule_values_are_rejected() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);

        let unsupported_target = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![SearchRuleInput {
                    target: "company".to_string(),
                    kind: "text".to_string(),
                    value: "Acme".to_string(),
                }],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap_err();
        assert!(unsupported_target.contains("includeRules[0].target must be title"));

        let unsupported_kind = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![SearchRuleInput {
                    target: "title".to_string(),
                    kind: "glob".to_string(),
                    value: "Physik".to_string(),
                }],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap_err();
        assert!(unsupported_kind.contains("includeRules[0].kind must be text or regex"));

        let empty_value = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![text_rule("   ")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap_err();
        assert!(empty_value.contains("includeRules[0].value must not be empty"));
    });
}

#[test]
fn update_and_delete_are_rejected_while_search_request_has_running_run() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);

        let created = service
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Draft,
                include_rules: vec![text_rule("Physik")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![],
            })
            .await
            .unwrap();

        let running_run = running_search_runs.begin(created.id).unwrap();

        let update_error = service
            .update(
                created.id,
                UpdateSearchRequestInput {
                    status: SearchRequestStatus::Draft,
                    include_rules: vec![text_rule("Laser")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_keys: vec![],
                },
            )
            .await
            .unwrap_err();
        assert!(update_error.contains("currently running search run"));

        let delete_error = service.delete(created.id).await.unwrap_err();
        assert!(delete_error.contains("currently running search run"));

        drop(running_run);
        service.delete(created.id).await.unwrap();
    });
}

fn text_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "text".to_string(),
        value: value.to_string(),
    }
}

fn regex_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "regex".to_string(),
        value: value.to_string(),
    }
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
