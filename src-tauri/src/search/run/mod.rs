mod atomic_persistence;
mod execution;
mod service;
#[cfg(test)]
mod tests;
mod types;

pub(crate) use atomic_persistence::{persist_atomic_search_run, AtomicSearchRunInput};
#[cfg(test)]
pub(crate) use execution::ScriptedResolutionSource;
pub use execution::SearchRunResolutionRuntime;
pub(crate) use execution::{
    cancellation_or_default, production_resolution_ceilings, NeverCancelled,
};
pub use service::{
    default_search_run_result_artifact, default_search_run_result_path, SearchRunResultArtifact,
    SearchRunService, SourceExecutionError,
};
pub use types::{
    NormalizedPosting, PostingSource, SearchRunResult, SearchRunStatus, SourceResolutionSummary,
    SourceRunResult, SourceRunStatus,
};
