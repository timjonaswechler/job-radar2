use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
};

use sqlx::SqlitePool;

use crate::{
    geo::{
        prepare_location_filter, GeoResolver, LocationFilterMatchReport,
        LocationFilterNotAppliedReason, LocationMatchOutcome, LocationResolutionAmbiguity,
    },
    profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    search::{
        normalization::normalize_source_candidate,
        posting::import_search_run_result_in_transaction,
        request::{RunningSearchRuns, SearchRequestService},
    },
};

use super::super::{SearchRunResult, SearchRunStatus, SourceExecutionInput, SourceExecutor};
use super::{
    compile_rules, db_error, generated_at_timestamp, matches_any_rule, merge_postings,
    overall_status, posting_source, resolve_selected_sources_with_options,
    source_run_cancelled_for_key, source_run_cancelled_for_source, source_run_completed,
    source_run_failed, source_run_failed_for_key, source_run_failed_for_source,
    source_run_skipped_for_source, update_search_request_last_run,
    validate_executable_search_request, write_search_run_result, SearchRunResultArtifact,
    SelectedSearchRunSource, SourceSelectionOptions, Treffer,
};

pub struct SearchRunService<'a> {
    pool: &'a SqlitePool,
    running_search_runs: &'a RunningSearchRuns,
    source_executor: &'a dyn SourceExecutor,
    result_artifact: SearchRunResultArtifact,
    source_registry_app_data_dir: PathBuf,
    selection_options: SourceSelectionOptions,
    geo_resolver: Option<&'a dyn GeoResolver>,
}

impl<'a> SearchRunService<'a> {
    pub fn new(
        pool: &'a SqlitePool,
        running_search_runs: &'a RunningSearchRuns,
        source_executor: &'a dyn SourceExecutor,
        result_path: impl Into<PathBuf>,
        source_registry_app_data_dir: impl Into<PathBuf>,
    ) -> Self {
        Self::new_with_result_artifact(
            pool,
            running_search_runs,
            source_executor,
            SearchRunResultArtifact::WriteTo(result_path.into()),
            source_registry_app_data_dir,
        )
    }

    pub fn new_with_result_artifact(
        pool: &'a SqlitePool,
        running_search_runs: &'a RunningSearchRuns,
        source_executor: &'a dyn SourceExecutor,
        result_artifact: SearchRunResultArtifact,
        source_registry_app_data_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            pool,
            running_search_runs,
            source_executor,
            result_artifact,
            source_registry_app_data_dir: source_registry_app_data_dir.into(),
            selection_options: SourceSelectionOptions::default(),
            geo_resolver: None,
        }
    }

    pub fn with_geo_resolver(mut self, geo_resolver: &'a dyn GeoResolver) -> Self {
        self.geo_resolver = Some(geo_resolver);
        self
    }

    pub fn allowing_draft_sources(mut self, allow_draft_sources: bool) -> Self {
        self.selection_options.allow_draft_sources = allow_draft_sources;
        self
    }

    pub async fn run(&self, search_request_id: i64) -> Result<SearchRunResult, String> {
        self.run_with_cancellation(search_request_id, None).await
    }

    pub async fn run_with_cancellation(
        &self,
        search_request_id: i64,
        cancellation_token: Option<&crate::background_tasks::CancellationToken>,
    ) -> Result<SearchRunResult, String> {
        let _running_run = self.running_search_runs.begin(search_request_id)?;
        let search_request = SearchRequestService::new(self.pool, self.running_search_runs)
            .get(search_request_id)
            .await?;
        validate_executable_search_request(&search_request)?;

        let include_rules = compile_rules(&search_request.include_rules, "includeRules", false)?;
        let exclude_rules = compile_rules(&search_request.exclude_rules, "excludeRules", true)?;
        let registry_snapshot =
            crate::source_profile::registry::load_snapshot(&self.source_registry_app_data_dir);
        let selected_sources = resolve_selected_sources_with_options(
            &registry_snapshot,
            &search_request.source_keys,
            self.selection_options,
        );

        let mut source_runs = Vec::with_capacity(selected_sources.len());
        let mut candidates = Vec::new();

        for selected_source in &selected_sources {
            if cancellation_token.is_some_and(|token| token.is_cancelled()) {
                source_runs.push(cancelled_source_run_for_selected(selected_source));
                continue;
            }

            let source = match selected_source {
                SelectedSearchRunSource::Resolved(source) => source.as_ref(),
                SelectedSearchRunSource::Missing { source_key, error } => {
                    source_runs.push(source_run_failed_for_key(source_key, error.clone()));
                    continue;
                }
                SelectedSearchRunSource::Failed {
                    source_key,
                    source_name,
                    error,
                } => {
                    source_runs.push(source_run_failed_for_source(
                        source_key,
                        source_name,
                        error.clone(),
                    ));
                    continue;
                }
                SelectedSearchRunSource::Skipped {
                    source_key,
                    source_name,
                    diagnostics,
                    summary,
                } => {
                    source_runs.push(source_run_skipped_for_source(
                        source_key,
                        source_name,
                        diagnostics.clone(),
                        summary.clone(),
                    ));
                    continue;
                }
            };
            let input = SourceExecutionInput {
                source,
                cancellation_token,
            };

            match self.source_executor.execute(input).await {
                Ok(_output) if cancellation_token.is_some_and(|token| token.is_cancelled()) => {
                    source_runs.push(source_run_cancelled_for_source(&source.key, &source.name));
                }
                Ok(output) => {
                    let admitted = output
                        .occurrences
                        .into_iter()
                        .filter_map(super::super::execution::source_candidate)
                        .collect::<Vec<_>>();
                    let candidate_count = admitted.len();
                    candidates.extend(admitted.into_iter().filter_map(|candidate| {
                        normalize_source_candidate(candidate).map(|candidate| Treffer {
                            candidate,
                            source: posting_source(source, None),
                        })
                    }));
                    source_runs.push(source_run_completed(
                        source,
                        candidate_count,
                        output.diagnostics,
                    ));
                }
                Err(error) => source_runs.push(source_run_failed(source, error)),
            }
        }

        let positive_matches = candidates
            .into_iter()
            .filter(|candidate| matches_any_rule(&include_rules, &candidate.candidate))
            .collect::<Vec<_>>();
        let rule_matched_treffers = positive_matches
            .into_iter()
            .filter(|candidate| !matches_any_rule(&exclude_rules, &candidate.candidate))
            .collect::<Vec<_>>();
        let (treffers, diagnostics) = self
            .filter_treffers_by_location(
                &search_request.locations,
                search_request.radius_km,
                rule_matched_treffers,
            )
            .await?;

        let mut matched_counts = HashMap::<String, usize>::new();
        for treffer in &treffers {
            *matched_counts
                .entry(treffer.source.source_key.clone())
                .or_default() += 1;
        }
        for source_run in &mut source_runs {
            source_run.matched_count = matched_counts
                .get(&source_run.source_key)
                .copied()
                .unwrap_or_default();
        }

        let result = SearchRunResult {
            search_request_id,
            status: overall_status(&source_runs),
            generated_at: generated_at_timestamp(self.pool).await?,
            diagnostics,
            source_runs,
            postings: merge_postings(treffers),
        };

        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        if matches!(
            result.status,
            SearchRunStatus::Completed | SearchRunStatus::CompletedWithErrors
        ) {
            import_search_run_result_in_transaction(&mut transaction, &result).await?;
        }
        update_search_request_last_run(&mut transaction, &result).await?;
        transaction.commit().await.map_err(db_error)?;

        match &self.result_artifact {
            SearchRunResultArtifact::Disabled => {}
            SearchRunResultArtifact::WriteTo(path) => {
                write_search_run_result(path, &result).await?
            }
        }

        Ok(result)
    }

    async fn filter_treffers_by_location(
        &self,
        request_locations: &[String],
        radius_km: Option<i64>,
        treffers: Vec<Treffer>,
    ) -> Result<(Vec<Treffer>, Diagnostics), String> {
        let Some(geo_resolver) = self.geo_resolver else {
            return Ok((treffers, Vec::new()));
        };

        let location_filter =
            prepare_location_filter(geo_resolver, request_locations, radius_km).await?;
        let mut diagnostics = Vec::new();
        if location_filter.not_applied_reason()
            == Some(LocationFilterNotAppliedReason::MissingRadiusKm)
        {
            diagnostics.push(location_filter_missing_radius_diagnostic());
        }

        let mut location_diagnostic_summary = LocationFilterDiagnosticSummary::default();
        location_diagnostic_summary
            .observe_request_ambiguities(location_filter.request_ambiguities());

        let mut filtered_treffers = Vec::new();
        for treffer in treffers {
            let report = location_filter
                .matches_candidate_with_report(geo_resolver, &treffer.candidate.locations)
                .await?;
            location_diagnostic_summary.observe_match_report(&report);

            match report.outcome {
                LocationMatchOutcome::Applied { matched: true } => filtered_treffers.push(treffer),
                LocationMatchOutcome::NotApplied { .. } => filtered_treffers.push(treffer),
                LocationMatchOutcome::Applied { matched: false } => {}
            }
        }

        diagnostics.extend(location_diagnostic_summary.into_diagnostics());

        Ok((filtered_treffers, diagnostics))
    }
}

fn location_filter_missing_radius_diagnostic() -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "location_filter_not_applied_missing_radius_km".to_string(),
        message: "Search Request locations were configured, but radiusKm is missing; location filtering was not applied.".to_string(),
        severity: DiagnosticSeverity::Warning,
        path: "/radiusKm".to_string(),
        strategy_key: None,
        details: None,
    }
}

#[derive(Default)]
struct LocationFilterDiagnosticSummary {
    unresolved_candidate_location_count: usize,
    unresolved_candidate_affected_count: usize,
    unresolved_candidate_samples: BTreeSet<String>,
    request_ambiguities: Vec<LocationResolutionAmbiguity>,
    candidate_ambiguity_count: usize,
    candidate_ambiguity_samples: Vec<LocationResolutionAmbiguity>,
}

impl LocationFilterDiagnosticSummary {
    fn observe_request_ambiguities(&mut self, ambiguities: &[LocationResolutionAmbiguity]) {
        self.request_ambiguities.extend_from_slice(ambiguities);
    }

    fn observe_match_report(&mut self, report: &LocationFilterMatchReport) {
        if !report.unresolved_candidate_locations.is_empty() {
            self.unresolved_candidate_affected_count += 1;
            self.unresolved_candidate_location_count += report.unresolved_candidate_locations.len();
            for location in &report.unresolved_candidate_locations {
                self.unresolved_candidate_samples.insert(location.clone());
            }
        }

        self.candidate_ambiguity_count += report.candidate_ambiguities.len();
        for ambiguity in &report.candidate_ambiguities {
            if self.candidate_ambiguity_samples.len() < 5 {
                self.candidate_ambiguity_samples.push(ambiguity.clone());
            }
        }
    }

    fn into_diagnostics(self) -> Diagnostics {
        let mut diagnostics = Vec::new();
        if self.unresolved_candidate_location_count > 0 {
            diagnostics.push(unresolved_candidate_locations_diagnostic(&self));
        }
        if !self.request_ambiguities.is_empty() || self.candidate_ambiguity_count > 0 {
            diagnostics.push(ambiguous_locations_diagnostic(&self));
        }
        diagnostics
    }
}

fn unresolved_candidate_locations_diagnostic(
    summary: &LocationFilterDiagnosticSummary,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "location_filter_candidate_locations_unresolved".to_string(),
        message: "Some candidate location values could not be resolved and did not contribute to active location filter matches.".to_string(),
        severity: DiagnosticSeverity::Warning,
        path: "/postings/*/locations".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "unresolvedLocationCount": summary.unresolved_candidate_location_count,
            "affectedCandidateCount": summary.unresolved_candidate_affected_count,
            "samples": summary.unresolved_candidate_samples.iter().take(5).collect::<Vec<_>>()
        })),
    }
}

fn ambiguous_locations_diagnostic(summary: &LocationFilterDiagnosticSummary) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "location_filter_ambiguous_locations".to_string(),
        message: "Some locations resolved to multiple geo points; location filtering considered all resolved locations.".to_string(),
        severity: DiagnosticSeverity::Info,
        path: "/locations".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "requestLocationAmbiguityCount": summary.request_ambiguities.len(),
            "candidateLocationAmbiguityCount": summary.candidate_ambiguity_count,
            "requestSamples": ambiguity_samples(&summary.request_ambiguities),
            "candidateSamples": ambiguity_samples(&summary.candidate_ambiguity_samples)
        })),
    }
}

fn ambiguity_samples(ambiguities: &[LocationResolutionAmbiguity]) -> Vec<serde_json::Value> {
    ambiguities
        .iter()
        .take(5)
        .map(|ambiguity| {
            serde_json::json!({
                "input": &ambiguity.input,
                "resolvedLabels": &ambiguity.resolved_labels
            })
        })
        .collect()
}

fn cancelled_source_run_for_selected(
    selected_source: &SelectedSearchRunSource,
) -> super::super::SourceRunResult {
    match selected_source {
        SelectedSearchRunSource::Resolved(source) => {
            source_run_cancelled_for_source(&source.key, &source.name)
        }
        SelectedSearchRunSource::Missing { source_key, .. } => {
            source_run_cancelled_for_key(source_key)
        }
        SelectedSearchRunSource::Failed {
            source_key,
            source_name,
            ..
        }
        | SelectedSearchRunSource::Skipped {
            source_key,
            source_name,
            ..
        } => source_run_cancelled_for_source(source_key, source_name),
    }
}
