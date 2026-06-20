//! Declarative browser source-inventory adapter backed by registry execution plans.
//!
//! This adapter satisfies the `SourceExecutor` seam for Quellen with
//! `adapter_key = declarative_browser_inventory`. The external representation is
//! the resolved source registry access path: optional `query`, ordered
//! `interactions`, and `inventory` definitions. The module translates that JSON
//! shape into Job Radar `SourceCandidate` values and maps selector/browser
//! failures to `SourceExecutionError::Failed`.
//!
//! Minimal browser inventory language:
//!
//! - `executionPlan.query` is optional and can build a query-parameterized URL
//!   from `baseUrl`, `path`, and an ordered `params` array. When absent,
//!   `sourceConfig.startUrl` is used as the page URL.
//! - Query param templates may use `{{searchRequest:titleText}}`,
//!   `{{searchRequest:firstLocation}}`, and `{{searchRequest:radiusKm}}`.
//! - The first `waitFor` entry in `executionPlan.interactions` is passed to the
//!   managed browser runtime.
//! - `executionPlan.inventory.items.select` is a CSS selector for job cards.
//! - `executionPlan.inventory.fields.title`, `company`, and `url` use exactly
//!   one of `selectorText` or `selectorAttribute`.
//! - `executionPlan.inventory.fields.locations` is an array of the same field
//!   expressions and may yield zero or more locations.

use serde_json::Value;
use std::path::PathBuf;

use crate::search_run_model::{
    BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
    SourceExecutor,
};

use super::*;

pub(super) const ADAPTER_KEY: &str = "declarative_browser_inventory";

pub(crate) struct DeclarativeBrowserInventoryExecutor<B = ManagedBrowserInventoryClient> {
    pub(super) browser: B,
}

impl DeclarativeBrowserInventoryExecutor<ManagedBrowserInventoryClient> {
    pub(crate) fn new_managed(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser: ManagedBrowserInventoryClient {
                runtime_dir: browser_runtime_dir.into(),
            },
        }
    }
}

impl<B> DeclarativeBrowserInventoryExecutor<B> {
    #[cfg(test)]
    pub(super) fn new(browser: B) -> Self {
        Self { browser }
    }
}

impl<B> SourceExecutor for DeclarativeBrowserInventoryExecutor<B>
where
    B: BrowserInventoryClient + Send + Sync,
{
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(input).await })
    }
}

impl<B> DeclarativeBrowserInventoryExecutor<B>
where
    B: BrowserInventoryClient + Send + Sync,
{
    async fn execute_source(
        &self,
        input: SourceExecutionInput<'_>,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        let source = input.source;
        if source.adapter_key != ADAPTER_KEY {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by {ADAPTER_KEY}",
                source.adapter_key
            )));
        }

        let inventory = source
            .inventory()
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SourceExecutionError::Failed(format!(
                    "executionPlan.inventory must be a JSON object for source {}",
                    source.key
                ))
            })?;
        validate_allowed_keys(
            inventory,
            &["items", "fields"],
            &plan_path(source, "executionPlan.inventory"),
        )?;

        let query_url = render_query_url(&input)?;
        let navigate_url = match query_url {
            Some(query_url) => query_url,
            None => source_config_start_url(source)?,
        };
        let page_url = parse_http_url(
            &navigate_url,
            &plan_path(source, "executionPlan.navigate.url"),
        )?;

        let wait_for = parse_wait_for(source)?;
        let rendered_html = self
            .browser
            .render_html(page_url.clone(), wait_for.clone())
            .await
            .map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not render browser inventory {} for source {}: {error}",
                    page_url.as_str(),
                    source.key
                ))
            })?;

        extract_candidates(source, &rendered_html, &page_url)
    }
}
