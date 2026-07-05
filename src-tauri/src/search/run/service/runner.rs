use std::{collections::HashMap, path::PathBuf};

use sqlx::SqlitePool;

use crate::search::{
    normalization::normalize_source_candidate,
    posting::import_search_run_result_in_transaction,
    request::{RunningSearchRuns, SearchRequestService},
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
        }
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
                    let candidate_count = output.candidates.len();
                    candidates.extend(output.candidates.into_iter().filter_map(|candidate| {
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
        let treffers = positive_matches
            .into_iter()
            .filter(|candidate| !matches_any_rule(&exclude_rules, &candidate.candidate))
            .collect::<Vec<_>>();

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
