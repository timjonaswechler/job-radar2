use super::support::*;

#[test]
fn matching_uses_or_semantics_and_excludes_after_positive_matching() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser"), regex_rule("Optics\\s+Engineer")],
            vec![text_rule("praktikum"), regex_rule("Werkstudent")],
        )
        .await;
        let result_path = temp_dir.path().join("search-run-result.json");
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "LASER Physicist",
                    "SCHOTT",
                    "https://example.test/1",
                    &["Mainz"],
                ),
                candidate(
                    "Senior Optics Engineer",
                    "SCHOTT",
                    "https://example.test/2",
                    &["Mainz"],
                ),
                candidate(
                    "Laser Praktikum",
                    "SCHOTT",
                    "https://example.test/3",
                    &["Mainz"],
                ),
                candidate(
                    "Werkstudent Optics Engineer",
                    "SCHOTT",
                    "https://example.test/4",
                    &["Mainz"],
                ),
                candidate("Chemist", "SCHOTT", "https://example.test/5", &["Mainz"]),
            ]),
        )]);
        let running_search_runs = RunningSearchRuns::default();

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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 5);
        assert_eq!(result.source_runs[0].matched_count, 2);
        assert_eq!(
            result
                .postings
                .iter()
                .map(|posting| posting.title.as_str())
                .collect::<Vec<_>>(),
            vec!["LASER Physicist", "Senior Optics Engineer"]
        );
        let result_json: Value = serde_json::from_str(
            &std::fs::read_to_string(result_path).expect("result JSON should be written"),
        )
        .unwrap();
        assert_eq!(result_json["status"], "completed");
        assert_eq!(result_json["sourceRuns"][0]["matchedCount"], 2);
    });
}

#[test]
fn exclude_regex_matching_is_case_insensitive_without_changing_include_regex() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![regex_rule("Laser")],
            vec![regex_rule("praktik(um|ant)")],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "Laser Engineer",
                    "SCHOTT",
                    "https://example.test/1",
                    &["Mainz"],
                ),
                candidate(
                    "Laser PraktikantIn",
                    "SCHOTT",
                    "https://example.test/2",
                    &["Mainz"],
                ),
                candidate(
                    "laser Engineer",
                    "SCHOTT",
                    "https://example.test/3",
                    &["Mainz"],
                ),
            ]),
        )]);
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
        assert_eq!(result.source_runs[0].candidate_count, 3);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(
            result
                .postings
                .iter()
                .map(|posting| posting.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Laser Engineer"]
        );
    });
}

#[test]
fn normalizes_source_candidates_before_matching_and_merging() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("Senior Laser Engineer")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "  Senior\n Laser   Engineer  ",
                    "\tACME   GmbH\n",
                    " https://example.test/laser ",
                    &[" Mainz ", "", "mainz", "  München\nNord  "],
                ),
                candidate(
                    "Senior Laser Engineer",
                    " ",
                    "https://example.test/empty-company",
                    &["Remote"],
                ),
            ]),
        )]);
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
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Senior Laser Engineer");
        assert_eq!(posting.company, "ACME GmbH");
        assert_eq!(posting.url, "https://example.test/laser");
        assert_eq!(posting.locations, vec!["Mainz", "München Nord"]);
        assert_eq!(posting.sources[0].url, "https://example.test/laser");
    });
}

#[test]
fn filters_search_run_matches_by_request_location_radius() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("Laser")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "Laser Engineer Wiesbaden",
                    "ACME",
                    "https://example.test/wiesbaden",
                    &["Wiesbaden"],
                ),
                candidate(
                    "Laser Engineer Köln",
                    "ACME",
                    "https://example.test/koeln",
                    &["Köln"],
                ),
            ]),
        )]);
        let running_search_runs = RunningSearchRuns::default();
        let geo_db_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path)
            .await
            .unwrap();

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer Wiesbaden");
        assert_eq!(result.postings[0].locations, vec!["Wiesbaden"]);
    });
}

#[test]
fn reports_when_request_location_filter_is_not_applied_without_radius() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let running_search_runs = RunningSearchRuns::default();
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Laser")],
                exclude_rules: vec![],
                locations: vec!["Mainz".to_string()],
                radius_km: None,
                source_keys: source_keys.clone(),
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer Köln",
                "ACME",
                "https://example.test/koeln",
                &["Köln"],
            )]),
        )]);
        let geo_db_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path)
            .await
            .unwrap();

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer Köln");
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "location_filter_not_applied_missing_radius_km"
                && diagnostic.severity
                    == crate::profile_dsl::diagnostics::DiagnosticSeverity::Warning
        }));
    });
}

#[test]
fn leaves_matching_unchanged_without_request_locations() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let running_search_runs = RunningSearchRuns::default();
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: source_keys.clone(),
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer Köln",
                "ACME",
                "https://example.test/koeln",
                &["Köln"],
            )]),
        )]);
        let geo_db_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path)
            .await
            .unwrap();

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer Köln");
        assert!(result.diagnostics.is_empty());
    });
}

#[test]
fn reports_unresolved_and_ambiguous_candidate_location_diagnostics() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("Laser")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "Laser Engineer Wiesbaden",
                    "ACME",
                    "https://example.test/wiesbaden",
                    &["Wiesbaden"],
                ),
                candidate(
                    "Laser Engineer Atlantis",
                    "ACME",
                    "https://example.test/atlantis",
                    &["Atlantis"],
                ),
                candidate(
                    "Laser Engineer Twin City",
                    "ACME",
                    "https://example.test/twin-city",
                    &["Twin City"],
                ),
            ]),
        )]);
        let running_search_runs = RunningSearchRuns::default();
        let geo_resolver = FixtureGeoResolver::new([
            (
                "Mainz",
                vec![
                    resolved_location("Mainz", "Mainz", 49.99, 8.24),
                    resolved_location("Mainz", "Mainz-Bretzenheim", 49.98, 8.23),
                ],
            ),
            (
                "Wiesbaden",
                vec![resolved_location("Wiesbaden", "Wiesbaden", 50.08, 8.24)],
            ),
            ("Atlantis", vec![]),
            (
                "Twin City",
                vec![
                    resolved_location("Twin City", "Twin City North", 50.0, 8.25),
                    resolved_location("Twin City", "Twin City South", 60.0, 9.25),
                ],
            ),
        ]);

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].matched_count, 2);
        assert_eq!(result.postings.len(), 2);
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Laser Engineer Wiesbaden"));
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Laser Engineer Twin City"));
        assert!(!result
            .postings
            .iter()
            .any(|posting| posting.title == "Laser Engineer Atlantis"));

        let unresolved = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "location_filter_candidate_locations_unresolved")
            .expect("unresolved candidate location diagnostic");
        assert_eq!(
            unresolved.severity,
            crate::profile_dsl::diagnostics::DiagnosticSeverity::Warning
        );
        assert_eq!(
            unresolved.details.as_ref().unwrap()["unresolvedLocationCount"],
            json!(1)
        );
        assert_eq!(
            unresolved.details.as_ref().unwrap()["affectedCandidateCount"],
            json!(1)
        );
        assert_eq!(
            unresolved.details.as_ref().unwrap()["samples"],
            json!(["Atlantis"])
        );

        let ambiguous = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "location_filter_ambiguous_locations")
            .expect("ambiguous location diagnostic");
        assert_eq!(
            ambiguous.severity,
            crate::profile_dsl::diagnostics::DiagnosticSeverity::Info
        );
        assert_eq!(
            ambiguous.details.as_ref().unwrap()["requestLocationAmbiguityCount"],
            json!(1)
        );
        assert_eq!(
            ambiguous.details.as_ref().unwrap()["candidateLocationAmbiguityCount"],
            json!(1)
        );
    });
}

#[test]
fn fails_search_run_when_request_location_cannot_be_resolved() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let running_search_runs = RunningSearchRuns::default();
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Laser")],
                exclude_rules: vec![],
                locations: vec!["Gibtsnichtstadt".to_string()],
                radius_km: Some(30),
                source_keys: source_keys.clone(),
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer Wiesbaden",
                "ACME",
                "https://example.test/wiesbaden",
                &["Wiesbaden"],
            )]),
        )]);
        let geo_db_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
        let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path)
            .await
            .unwrap();

        let error = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(search_request.id)
        .await
        .unwrap_err();

        assert_eq!(
            error,
            "Search Request location could not be resolved: Gibtsnichtstadt"
        );
    });
}

struct FixtureGeoResolver {
    locations: BTreeMap<String, Vec<crate::geo::ResolvedLocation>>,
}

impl FixtureGeoResolver {
    fn new(
        locations: impl IntoIterator<Item = (&'static str, Vec<crate::geo::ResolvedLocation>)>,
    ) -> Self {
        Self {
            locations: locations
                .into_iter()
                .map(|(input, locations)| (input.to_string(), locations))
                .collect(),
        }
    }
}

impl crate::geo::GeoResolver for FixtureGeoResolver {
    fn resolve<'a>(&'a self, input: &'a str) -> crate::geo::GeoResolveFuture<'a> {
        Box::pin(async move { Ok(self.locations.get(input).cloned().unwrap_or_default()) })
    }
}

fn resolved_location(
    input: &str,
    label: &str,
    latitude: f64,
    longitude: f64,
) -> crate::geo::ResolvedLocation {
    crate::geo::ResolvedLocation {
        input: input.to_string(),
        label: label.to_string(),
        point: crate::geo::GeoPoint {
            latitude,
            longitude,
        },
    }
}
