mod atomic_persistence;
mod execution;
mod service;
#[cfg(test)]
mod tests;
mod types;

#[allow(unused_imports)]
pub(crate) use atomic_persistence::{persist_atomic_search_run, AtomicSearchRunInput};
#[allow(unused_imports)]
pub(crate) use execution::source_candidate;
#[allow(unused_imports)]
pub use execution::{
    BoxedSourceExecutionFuture, DefaultSourceExecutor, SourceExecutionInput, SourceExecutionOutput,
    SourceExecutionSource, SourceExecutor,
};
#[allow(unused_imports)]
pub use service::{
    default_search_run_result_artifact, default_search_run_result_path, SearchRunResultArtifact,
    SearchRunService, SourceExecutionError,
};
pub use types::{
    NormalizedPosting, PostingMeta, PostingSource, SearchRunResult, SearchRunStatus,
    SourceCandidate, SourceRunResult, SourceRunStatus,
};
