use super::support::*;

#[test]
fn dedupes_with_overlapping_locations_or_missing_locations_and_preserves_sources() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![
                    candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://source-one.test/laser",
                        &["Mainz"],
                    ),
                    candidate(
                        "Remote Engineer",
                        "ACME",
                        "https://source-one.test/remote",
                        &[],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-one.test/optics-berlin",
                        &["Berlin"],
                    ),
                ]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![
                    candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://source-two.test/laser",
                        &[" mainz ", "Wiesbaden"],
                    ),
                    candidate(
                        "Remote Engineer",
                        "ACME",
                        "https://source-two.test/remote",
                        &["Berlin"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-two.test/optics-hamburg",
                        &["Hamburg"],
                    ),
                ]),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 4);

        let laser = result
            .postings
            .iter()
            .find(|posting| posting.title == "Laser Engineer")
            .unwrap();
        assert_eq!(laser.locations, vec!["Mainz", "Wiesbaden"]);
        assert_eq!(laser.sources.len(), 2);
        assert_eq!(
            laser
                .sources
                .iter()
                .map(|source| source.source_key.as_str())
                .collect::<Vec<_>>(),
            vec!["source_one", "source_two"]
        );

        let remote = result
            .postings
            .iter()
            .find(|posting| posting.title == "Remote Engineer")
            .unwrap();
        assert_eq!(remote.locations, vec!["Berlin"]);
        assert_eq!(remote.sources.len(), 2);

        let optics_postings = result
            .postings
            .iter()
            .filter(|posting| posting.title == "Optics Engineer")
            .collect::<Vec<_>>();
        assert_eq!(optics_postings.len(), 2);
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Berlin"]));
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Hamburg"]));
    });
}

#[test]
fn keyed_posting_meta_never_crosses_finalization() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![candidate_with_meta(
                    "Laser Engineer",
                    "ACME",
                    "https://source-one.test/laser",
                    &["Mainz"],
                    [("jobId", "source-one-42")],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate_with_meta(
                    "Laser Engineer",
                    "ACME",
                    "https://source-two.test/laser",
                    &["Mainz"],
                    [("jobId", "source-two-99")],
                )]),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].sources.len(), 2);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("source-one-42"));
        assert!(!serialized.contains("source-two-99"));

        let rows = sqlx::query(
            "SELECT source_key, posting_meta_json
             FROM job_posting_sources
             ORDER BY source_key",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("source_key"), "source_one");
        assert_eq!(rows[0].get::<String, _>("posting_meta_json"), "{}");
        assert_eq!(rows[1].get::<String, _>("source_key"), "source_two");
        assert_eq!(rows[1].get::<String, _>("posting_meta_json"), "{}");
    });
}

#[test]
fn fuzzy_dedupes_equivalent_titles_and_preserves_representative_posting() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Head Of Laser & Post Processing Development (mwd) RP/1205240901",
                    "SCHOTT AG",
                    "https://source-one.test/schott-laser",
                    &["Mainz"],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate(
                    "Head of Laser & Post Processing Development (m/w/d)*",
                    "SCHOTT AG",
                    "https://source-two.test/schott-laser",
                    &["Mainz"],
                )]),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(
            posting.title,
            "Head Of Laser & Post Processing Development (mwd) RP/1205240901"
        );
        assert_eq!(posting.company, "SCHOTT AG");
        assert_eq!(posting.url, "https://source-one.test/schott-laser");
        assert_eq!(posting.locations, vec!["Mainz"]);
        assert_eq!(posting.sources.len(), 2);
        assert_eq!(
            posting
                .sources
                .iter()
                .map(|source| source.source_key.as_str())
                .collect::<Vec<_>>(),
            vec!["source_one", "source_two"]
        );
    });
}

#[test]
fn fuzzy_dedupe_keeps_different_roles_at_same_company_and_location_separate() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Laser Engineer",
                    "ACME",
                    "https://source-one.test/laser-engineer",
                    &["Mainz"],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate(
                    "Senior Laser Engineer Frontend",
                    "ACME",
                    "https://source-two.test/senior-laser-engineer-frontend",
                    &["Mainz"],
                )]),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 2);
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Laser Engineer"));
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Senior Laser Engineer Frontend"));
    });
}

#[test]
fn location_compatibility_allows_whole_phrase_overlap_but_blocks_contradictions() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![
                    candidate(
                        "Platform Engineer",
                        "ACME",
                        "https://source-one.test/platform-berlin",
                        &["Berlin"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-one.test/optics-mainz",
                        &["Mainz"],
                    ),
                ]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![
                    candidate(
                        "Platform Engineer",
                        "ACME",
                        "https://source-two.test/platform-berlin-germany",
                        &["Berlin, Germany"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-two.test/optics-frankfurt",
                        &["Frankfurt am Main"],
                    ),
                ]),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 3);

        let platform = result
            .postings
            .iter()
            .find(|posting| posting.title == "Platform Engineer")
            .unwrap();
        assert_eq!(platform.locations, vec!["Berlin", "Berlin, Germany"]);
        assert_eq!(platform.sources.len(), 2);

        let optics_postings = result
            .postings
            .iter()
            .filter(|posting| posting.title == "Optics Engineer")
            .collect::<Vec<_>>();
        assert_eq!(optics_postings.len(), 2);
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Mainz"]));
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Frankfurt am Main"]));
    });
}
