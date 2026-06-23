use std::{collections::HashMap, path::PathBuf};

use sqlx::SqlitePool;

use crate::{
    search::normalization::normalize_source_candidate,
    search::request::{RunningSearchRuns, SearchRequestService},
};

use super::super::{SearchRunResult, SourceExecutionInput, SourceExecutor};
use super::{
    compile_rules, generated_at_timestamp, matches_any_rule, merge_postings, overall_status,
    posting_source, resolve_selected_sources, source_run_completed, source_run_failed,
    source_run_failed_for_key, validate_executable_search_request, write_search_run_result,
    SelectedSearchRunSource, Treffer,
};

pub struct SearchRunService<'a> {
    pool: &'a SqlitePool,
    running_search_runs: &'a RunningSearchRuns,
    source_executor: &'a dyn SourceExecutor,
    result_path: PathBuf,
    source_registry_app_data_dir: PathBuf,
}

impl<'a> SearchRunService<'a> {
    pub fn new(
        pool: &'a SqlitePool,
        running_search_runs: &'a RunningSearchRuns,
        source_executor: &'a dyn SourceExecutor,
        result_path: impl Into<PathBuf>,
        source_registry_app_data_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            pool,
            running_search_runs,
            source_executor,
            result_path: result_path.into(),
            source_registry_app_data_dir: source_registry_app_data_dir.into(),
        }
    }

    pub async fn run(&self, search_request_id: i64) -> Result<SearchRunResult, String> {
        let _running_run = self.running_search_runs.begin(search_request_id)?;
        let search_request = SearchRequestService::new(self.pool, self.running_search_runs)
            .get(search_request_id)
            .await?;
        validate_executable_search_request(&search_request)?;

        let include_rules = compile_rules(&search_request.include_rules, "includeRules", false)?;
        let exclude_rules = compile_rules(&search_request.exclude_rules, "excludeRules", true)?;
        let registry_snapshot =
            crate::source::registry::load_snapshot(&self.source_registry_app_data_dir);
        let selected_sources =
            resolve_selected_sources(&registry_snapshot, &search_request.source_keys);

        let mut source_runs = Vec::with_capacity(selected_sources.len());
        let mut candidates = Vec::new();

        for selected_source in &selected_sources {
            let source = match selected_source {
                SelectedSearchRunSource::Resolved(source) => source.as_ref(),
                SelectedSearchRunSource::Missing { source_key, error } => {
                    source_runs.push(source_run_failed_for_key(source_key, error.clone()));
                    continue;
                }
            };
            let input = SourceExecutionInput {
                search_request: &search_request,
                source,
            };

            match self.source_executor.execute(input).await {
                Ok(source_candidates) => {
                    let candidate_count = source_candidates.len();
                    candidates.extend(source_candidates.into_iter().filter_map(|candidate| {
                        normalize_source_candidate(candidate).map(|candidate| Treffer {
                            candidate,
                            source: posting_source(source, None),
                        })
                    }));
                    source_runs.push(source_run_completed(source, candidate_count));
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

        write_search_run_result(&self.result_path, &result).await?;

        Ok(result)
    }
}
