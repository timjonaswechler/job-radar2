mod errors;
mod merging;
mod persistence;
mod rules;
mod runner;
mod selection;
mod source_runs;
mod validation;

use merging::{merge_postings, Treffer};
use persistence::{
    db_error, generated_at_timestamp, update_search_request_last_run, write_search_run_result,
};
use rules::{compile_rules, matches_any_rule};
use selection::{resolve_selected_sources, SelectedSearchRunSource};
use source_runs::{
    overall_status, posting_source, source_run_completed, source_run_failed,
    source_run_failed_for_key, source_run_failed_for_source, source_run_skipped_for_source,
};
use validation::validate_executable_search_request;

pub use errors::SourceExecutionError;
pub use persistence::{
    default_search_run_result_artifact, default_search_run_result_path, SearchRunResultArtifact,
};
pub use runner::SearchRunService;
