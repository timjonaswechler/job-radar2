mod errors;
mod merging;
mod persistence;
mod runner;
mod selection;
mod source_runs;
mod validation;

use merging::{finalized_merge_input, merge_postings};
use persistence::{generated_at_timestamp, last_run_error_summary, write_search_run_result};
use selection::{
    resolve_selected_sources_with_options, SelectedSearchRunSource, SourceSelectionOptions,
};
use source_runs::{
    overall_status, source_run_cancelled_for_key, source_run_cancelled_for_source,
    source_run_completed, source_run_failed_for_key, source_run_failed_for_source,
    source_run_resolution_failed, source_run_skipped_for_source,
};
use validation::validate_executable_search_request;

pub use errors::SourceExecutionError;
pub use persistence::{
    default_search_run_result_artifact, default_search_run_result_path, SearchRunResultArtifact,
};
pub use runner::SearchRunService;
