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
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.discovered as usize),
            5
        );
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            2
        );
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
        assert_eq!(
            result_json["sourceRuns"][0]["resolution"]["counts"]["finalized"],
            2
        );
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
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.discovered as usize),
            3
        );
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
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
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.discovered as usize),
            2
        );
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
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
        sqlx::query("UPDATE search_requests SET locations_json = '[\"Mainz\"]', radius_km = 30 WHERE id = ?1")
            .bind(search_request.id).execute(&pool).await.unwrap();
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.discovered as usize),
            2
        );
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer Wiesbaden");
        assert_eq!(result.postings[0].locations, vec!["Wiesbaden"]);
    });
}

#[test]
fn request_locations_without_radius_do_not_apply_filter_and_emit_warning() {
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
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
        assert_eq!(result.postings.len(), 1);
        assert!(result.source_runs[0].diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "location_filter_not_applied_missing_radius_km"
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
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            1
        );
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer Köln");
        assert!(result.diagnostics.is_empty());
    });
}

#[test]
fn radius_without_request_locations_does_not_require_candidate_locations_or_detail() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let running = RunningSearchRuns::default();
        let request = SearchRequestService::new(&pool, &running)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("Laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: Some(30),
                source_keys: source_keys.clone(),
            })
            .await
            .unwrap();
        let runtime = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/no-location",
                &[],
            )]),
        )]);
        let geo_resolver = FixtureGeoResolver::new(std::iter::empty::<(
            &'static str,
            Vec<crate::geo::ResolvedLocation>,
        )>());

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &running,
            &runtime,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .with_geo_resolver(&geo_resolver)
        .run(request.id)
        .await
        .unwrap();

        let resolution = result.source_runs[0].resolution.as_ref().unwrap();
        assert_eq!(resolution.counts.finalized, 1);
        assert_eq!(resolution.counts.unresolved, 0);
        assert_eq!(result.postings[0].locations, Vec::<String>::new());
    });
}

#[test]
fn geo_radius_matching_handles_unresolved_and_ambiguous_candidate_locations() {
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
        sqlx::query("UPDATE search_requests SET locations_json = '[\"Mainz\"]', radius_km = 30 WHERE id = ?1")
            .bind(search_request.id).execute(&pool).await.unwrap();
        let executor = fixture_resolution_runtime([(
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
        assert_eq!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .map_or(0, |r| r.counts.finalized as usize),
            2
        );
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

        assert!(result.diagnostics.is_empty());
        assert!(result.source_runs[0].diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "location_filter_candidate_locations_unresolved"
        }));
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "location_filter_ambiguous_locations" }));
    });
}

#[test]
fn candidate_geo_resolver_failure_is_a_truthful_source_runtime_failure_without_resolution() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("Laser")],
            vec![],
        )
        .await;
        sqlx::query("UPDATE search_requests SET locations_json = '[\"Mainz\"]', radius_km = 30 WHERE id = ?1")
            .bind(request.id).execute(&pool).await.unwrap();
        let runtime = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/broken",
                &["Broken"],
            )]),
        )]);
        let resolver = FailingCandidateGeoResolver;

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &RunningSearchRuns::default(),
            &runtime,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .with_geo_resolver(&resolver)
        .run(request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Failed);
        assert!(result.source_runs[0].resolution.is_none());
        assert!(result.source_runs[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("GeoResolution")));
        assert_eq!(
            result.source_runs[0].diagnostics[0].code,
            "location_filter_geo_resolution_failed"
        );
    });
}

struct FailingCandidateGeoResolver;
impl crate::geo::GeoResolver for FailingCandidateGeoResolver {
    fn resolve<'a>(&'a self, input: &'a str) -> crate::geo::GeoResolveFuture<'a> {
        Box::pin(async move {
            if input == "Mainz" {
                Ok(vec![resolved_location("Mainz", "Mainz", 49.99, 8.24)])
            } else {
                Err("fixture database unavailable".to_string())
            }
        })
    }
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
        let executor = fixture_resolution_runtime([(
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
