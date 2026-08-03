#[test]
fn smoke_cli_requires_at_least_one_explicit_source_key() {
    let error = super::cli::parse_smoke_cli_args([
        std::ffi::OsString::from("--app-data-dir"),
        std::ffi::OsString::from("/tmp/job-radar-smoke"),
    ])
    .err()
    .expect("missing explicit Source selection must be rejected");

    assert_eq!(error, "at least one --source-key is required");
}

#[test]
fn smoke_cli_does_not_consume_another_option_as_a_source_key() {
    let error = super::cli::parse_smoke_cli_args([
        std::ffi::OsString::from("--source-key"),
        std::ffi::OsString::from("--allow-draft"),
    ])
    .err()
    .expect("an option must not be accepted as a Source key");

    assert_eq!(error, "--source-key requires a non-empty source key value");
}

#[test]
fn smoke_reuses_request_and_overwrites_post_commit_artifact_through_runner() {
    struct UnusedGeo;
    impl geo::GeoResolver for UnusedGeo {
        fn resolve<'a>(&'a self, _input: &'a str) -> geo::GeoResolveFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    tauri::async_runtime::block_on(async {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::sync::Arc;

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
        let temp = tempfile::tempdir().unwrap();
        let catalog = search_requests::Catalog::new(pool.clone());
        let runner = search_runs::Runner::new(
            pool,
            sources::installed::Store::new(temp.path()),
            Arc::new(crate::adapters::ReqwestProfileHttpClient::new()),
            Arc::new(crate::browser_runtime::ManagedBrowserAcquisition::new(
                temp.path().join("browser"),
            )),
        );
        let artifact = temp.path().join("result.json");

        let first = super::runner::run_search_run_smoke_with_options(
            &catalog,
            &runner,
            &artifact,
            vec!["missing_source".into()],
            false,
            &UnusedGeo,
        )
        .await
        .unwrap();
        std::fs::write(&artifact, "stale artifact").unwrap();
        let second = super::runner::run_search_run_smoke_with_options(
            &catalog,
            &runner,
            &artifact,
            vec!["missing_source".into()],
            false,
            &UnusedGeo,
        )
        .await
        .unwrap();

        assert!(first.search_request_created);
        assert!(!second.search_request_created);
        assert_eq!(first.search_request_id, second.search_request_id);
        assert_eq!(second.result.status, search_runs::Status::Failed);
        assert_eq!(catalog.list().await.unwrap().len(), 1);
        let artifact_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
        assert_eq!(artifact_json["searchRequestId"], second.search_request_id);
        assert!(artifact_json.get("postings").is_none());
        catalog
            .delete(search_requests::Id::new(second.search_request_id).unwrap())
            .await
            .unwrap();
    });
}
