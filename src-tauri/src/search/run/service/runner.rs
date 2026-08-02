use std::path::PathBuf;

use geo::GeoResolver;
use search_resolution::CompiledSearchRequirements;
use source_engine::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use sqlx::SqlitePool;

use crate::search::run::{persist_atomic_search_run, AtomicSearchRunInput};
use search_requests::Execution;

use super::super::{
    cancellation_or_default, NeverCancelled, SearchRunResolutionRuntime, SearchRunResult,
    SearchRunStatus,
};
use super::{
    finalized_merge_input, generated_at_timestamp, last_run_error_summary, merge_postings,
    overall_status, resolve_selected_sources_with_options, source_run_cancelled_for_key,
    source_run_cancelled_for_source, source_run_completed, source_run_failed_for_key,
    source_run_failed_for_source, source_run_resolution_failed, source_run_skipped_for_source,
    write_search_run_result, SearchRunResultArtifact, SelectedSearchRunSource,
    SourceSelectionOptions,
};

pub struct SearchRunService<'a> {
    pool: &'a SqlitePool,
    resolution_runtime: &'a SearchRunResolutionRuntime,
    result_artifact: SearchRunResultArtifact,
    installed_sources: sources::installed::Store,
    selection_options: SourceSelectionOptions,
    geo_resolver: Option<&'a dyn GeoResolver>,
    #[cfg(test)]
    after_source_resolution: Option<&'a (dyn Fn() + Send + Sync)>,
    #[cfg(test)]
    before_persistence: Option<&'a (dyn Fn() + Send + Sync)>,
}

impl<'a> SearchRunService<'a> {
    pub fn new(
        pool: &'a SqlitePool,
        resolution_runtime: &'a SearchRunResolutionRuntime,
        result_path: impl Into<PathBuf>,
        installed_sources: sources::installed::Store,
    ) -> Self {
        Self::new_with_result_artifact(
            pool,
            resolution_runtime,
            SearchRunResultArtifact::WriteTo(result_path.into()),
            installed_sources,
        )
    }

    pub fn new_with_result_artifact(
        pool: &'a SqlitePool,
        resolution_runtime: &'a SearchRunResolutionRuntime,
        result_artifact: SearchRunResultArtifact,
        installed_sources: sources::installed::Store,
    ) -> Self {
        Self {
            pool,
            resolution_runtime,
            result_artifact,
            installed_sources,
            selection_options: SourceSelectionOptions::default(),
            geo_resolver: None,
            #[cfg(test)]
            after_source_resolution: None,
            #[cfg(test)]
            before_persistence: None,
        }
    }

    pub fn with_geo_resolver(mut self, resolver: &'a dyn GeoResolver) -> Self {
        self.geo_resolver = Some(resolver);
        self
    }

    pub fn allowing_draft_sources(mut self, allow: bool) -> Self {
        self.selection_options.allow_draft_sources = allow;
        self
    }

    #[cfg(test)]
    pub(crate) fn after_source_resolution(
        mut self,
        callback: &'a (dyn Fn() + Send + Sync),
    ) -> Self {
        self.after_source_resolution = Some(callback);
        self
    }

    #[cfg(test)]
    pub(crate) fn before_persistence(mut self, callback: &'a (dyn Fn() + Send + Sync)) -> Self {
        self.before_persistence = Some(callback);
        self
    }

    pub async fn run(&self, execution: Execution) -> Result<SearchRunResult, String> {
        self.run_with_cancellation(execution, None).await
    }

    pub async fn run_with_cancellation(
        &self,
        execution: Execution,
        cancellation_token: Option<&crate::background_tasks::CancellationToken>,
    ) -> Result<SearchRunResult, String> {
        let request = execution.snapshot();
        let search_request_id = execution.id().get();

        let requirements = match request.radius_km {
            Some(radius) => {
                let resolver = self.geo_resolver.ok_or_else(|| {
                    "Search Request radius requires an available GeoResolver".to_string()
                })?;
                CompiledSearchRequirements::compile_with_geo(
                    &request.include_rules,
                    &request.exclude_rules,
                    &request.locations,
                    Some(radius),
                    resolver,
                )
                .await?
            }
            None => CompiledSearchRequirements::compile(
                &request.include_rules,
                &request.exclude_rules,
                &request.locations,
                None,
            )
            .map_err(|failure| {
                format!("Search Request matching requirements are invalid: {failure:?}")
            })?,
        };

        let installed_sources = self.installed_sources.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed_sources.snapshot())
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let selected = resolve_selected_sources_with_options(
            &snapshot,
            &request.source_keys,
            self.selection_options,
        );
        let never_cancelled = NeverCancelled;
        let cancellation = cancellation_or_default(cancellation_token, &never_cancelled);
        let mut source_runs = Vec::with_capacity(selected.len());
        let mut finalized = Vec::new();

        for selected_source in &selected {
            if cancellation.is_cancelled() {
                source_runs.push(cancelled_source_run_for_selected(selected_source));
                continue;
            }
            let source = match selected_source {
                SelectedSearchRunSource::Resolved(source) => *source,
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

            match self
                .resolution_runtime
                .resolve(source, &requirements, cancellation)
                .await
            {
                Ok(resolution) => {
                    finalized.extend(
                        resolution.finalized.iter().map(|candidate| {
                            finalized_merge_input(candidate, source.source_name())
                        }),
                    );
                    source_runs.push(source_run_completed(source, &resolution));
                }
                Err(error) => source_runs.push(source_run_resolution_failed(source, error)),
            }
        }

        #[cfg(test)]
        if let Some(callback) = self.after_source_resolution {
            callback();
        }

        // Source resolution may finish concurrently with a task cancellation. Re-read the
        // authoritative token rather than relying only on the Source outcomes it produced.
        let cancelled_after_resolution =
            cancellation_token.is_some_and(|token| token.is_cancelled());
        let status = if cancelled_after_resolution {
            SearchRunStatus::Cancelled
        } else {
            overall_status(&source_runs)
        };
        let postings = if matches!(
            status,
            SearchRunStatus::Completed | SearchRunStatus::CompletedWithErrors
        ) {
            merge_postings(finalized)
        } else {
            Vec::new()
        };
        let mut result = SearchRunResult {
            search_request_id,
            status,
            generated_at: generated_at_timestamp(self.pool).await?,
            diagnostics: Vec::new(),
            source_runs,
            postings,
        };

        // This is the last cancellation boundary before DB01. It keeps the terminal Search Run
        // and posting persistence under one authority and preserves the single DB01 invocation.
        if cancellation_token.is_some_and(|token| token.is_cancelled()) {
            result.status = SearchRunStatus::Cancelled;
            result.postings.clear();
        }
        let last_run_error = last_run_error_summary(&result);
        #[cfg(test)]
        if let Some(callback) = self.before_persistence {
            callback();
        }
        persist_atomic_search_run(
            self.pool,
            AtomicSearchRunInput {
                search_request_id,
                status: result.status,
                generated_at: &result.generated_at,
                last_run_error: last_run_error.as_deref(),
                postings: &result.postings,
            },
        )
        .await?;

        if let SearchRunResultArtifact::WriteTo(path) = &self.result_artifact {
            if write_search_run_result(path, &result).await.is_err() {
                result.diagnostics.push(artifact_write_failed_diagnostic());
            }
        }
        Ok(result)
    }
}

fn artifact_write_failed_diagnostic() -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "search_run_result_artifact_write_failed".to_string(),
        message: "Search Run committed successfully, but its non-authoritative result artifact could not be written".to_string(),
        severity: DiagnosticSeverity::Warning,
        path: "/artifact".to_string(),
        strategy_key: None,
        details: None,
    }
}

fn cancelled_source_run_for_selected(
    selected: &SelectedSearchRunSource<'_>,
) -> super::super::SourceRunResult {
    match selected {
        SelectedSearchRunSource::Resolved(source) => {
            source_run_cancelled_for_source(source.source_key(), source.source_name())
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
