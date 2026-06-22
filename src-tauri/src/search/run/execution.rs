use std::{future::Future, path::PathBuf, pin::Pin};

use crate::{search::request::SearchRequest, source::registry::ResolvedSourceExecutionPlan};

use super::{SourceCandidate, SourceExecutionError};

pub type BoxedSourceExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<SourceCandidate>, SourceExecutionError>> + Send + 'a>>;

/// Public source-execution seam used by Suchläufe.
///
/// `SearchRunService` resolves selected source keys through one registry
/// snapshot at run start and passes immutable source execution plans here.
/// Adapters read access-path definitions from that plan instead of legacy
/// system/browser profile references.
pub type SourceExecutionSource = ResolvedSourceExecutionPlan;

pub struct SourceExecutionInput<'a> {
    pub search_request: &'a SearchRequest,
    pub source: &'a SourceExecutionSource,
}

pub trait SourceExecutor: Send + Sync {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a>;
}

pub struct DefaultSourceExecutor {
    browser_runtime_dir: PathBuf,
}

impl DefaultSourceExecutor {
    pub fn new(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser_runtime_dir: browser_runtime_dir.into(),
        }
    }
}

impl SourceExecutor for DefaultSourceExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            match input.source.adapter_key.as_str() {
                "declarative_endpoint_inventory" | "declarative_sitemap_inventory" => {
                    let executor =
                        crate::declarative::inventory::DeclarativeInventoryExecutor::new_reqwest();
                    executor.execute(input).await
                }
                "declarative_browser_inventory" => {
                    let executor = crate::declarative::browser_inventory::DeclarativeBrowserInventoryExecutor::new_managed(
                        self.browser_runtime_dir.clone(),
                    );
                    executor.execute(input).await
                }
                _ => Err(SourceExecutionError::Failed(format!(
                    "adapterKey {} has no search-run executor yet",
                    input.source.adapter_key
                ))),
            }
        })
    }
}
