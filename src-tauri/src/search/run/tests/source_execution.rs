use super::support::*;

#[test]
fn missing_source_key_becomes_failed_source_run_and_valid_keys_continue() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["missing_source".to_string(), source_keys[0].clone()],
            })
            .await
            .unwrap();
        let executor = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
            )]),
        )]);
        let result_path = temp_dir.path().join("search-run-result.json");

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            result_path.clone(),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].source_key, "missing_source");
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert!(result.source_runs[0]
            .error
            .as_deref()
            .unwrap()
            .contains("Selected Source `missing_source` was not found"));
        assert_eq!(
            result.source_runs[0].diagnostics[0].code,
            "source_not_found"
        );
        assert_eq!(
            serde_json::to_value(result.source_runs[0].diagnostics[0].category).unwrap(),
            json!("source_validation")
        );
        assert_eq!(result.source_runs[1].source_key, source_keys[0]);
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
        assert_eq!(
            result.source_runs[1]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].sources[0].source_key, source_keys[0]);

        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(result_path).unwrap()).unwrap();
        assert!(result_json["sourceRuns"][0].get("sourceId").is_none());
        assert!(result_json["postings"][0]["sources"][0]
            .get("sourceId")
            .is_none());
    });
}

#[test]
fn invalid_derived_selected_source_fails_with_structured_diagnostics_and_valid_sources_continue() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let mut invalid_source: Value =
            serde_json::from_str(&source_json("invalid_source", "Invalid Source")).unwrap();
        invalid_source
            .as_object_mut()
            .unwrap()
            .remove("sourceSupport");
        write_json(
            temp_dir.path().join("sources/invalid_source.json"),
            &invalid_source.to_string(),
        );
        write_json(
            temp_dir.path().join("sources/valid_source.json"),
            &source_json("valid_source", "Valid Source"),
        );
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["invalid_source".to_string(), "valid_source".to_string()],
            })
            .await
            .unwrap();
        let executor = fixture_resolution_runtime([(
            "valid_source",
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
            )]),
        )]);

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_source_support"));
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "source_validation_failed"));
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
    });
}

#[test]
fn draft_and_disabled_selected_sources_are_skipped_without_execution() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("sources/draft_source.json"),
            &source_json_with_status("draft_source", "Draft Source", "draft"),
        );
        write_json(
            temp_dir.path().join("sources/disabled_source.json"),
            &source_json_with_status("disabled_source", "Disabled Source", "disabled"),
        );
        write_json(
            temp_dir.path().join("sources/active_source.json"),
            &source_json("active_source", "Active Source"),
        );
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![
                    "draft_source".to_string(),
                    "disabled_source".to_string(),
                    "active_source".to_string(),
                ],
            })
            .await
            .unwrap();
        let executor = fixture_resolution_runtime([(
            "active_source",
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
            )]),
        )]);

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Skipped);
        assert_eq!(
            result.source_runs[0].diagnostics[0].code,
            "source_not_active"
        );
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Skipped);
        assert_eq!(
            result.source_runs[1].diagnostics[0].code,
            "source_not_active"
        );
        assert_eq!(result.source_runs[2].status, SourceRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
    });
}
