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

use reqwest::Url;
use std::{future::Future, path::PathBuf, pin::Pin};

use crate::browser_runtime::BrowserRuntimePageWait;

use super::*;

pub(super) type BoxedBrowserInventoryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait BrowserInventoryClient {
    fn render_html(
        &self,
        url: Url,
        wait_for: Option<BrowserInventoryWait>,
    ) -> BoxedBrowserInventoryFuture<'_>;
}

pub(crate) struct ManagedBrowserInventoryClient {
    pub(super) runtime_dir: PathBuf,
}

impl BrowserInventoryClient for ManagedBrowserInventoryClient {
    fn render_html(
        &self,
        url: Url,
        wait_for: Option<BrowserInventoryWait>,
    ) -> BoxedBrowserInventoryFuture<'_> {
        Box::pin(async move {
            let spec = crate::browser_runtime::current_runtime_spec();
            let status = crate::browser_runtime::status_for_runtime_dir(
                &self.runtime_dir,
                spec.as_ref(),
                false,
            );
            if status.status != crate::browser_runtime::BrowserRuntimeState::Installed {
                let status_detail = status
                    .error
                    .as_deref()
                    .unwrap_or("managed browser runtime is not installed and ready");
                return Err(format!(
                    "browser runtime unavailable: status {:?}: {status_detail}",
                    status.status
                ));
            }

            let executable_path = status.executable_path.as_deref().ok_or_else(|| {
                "browser runtime unavailable: installed managed browser runtime has no executable path".to_string()
            })?;
            let executable_path = PathBuf::from(executable_path);
            let runtime_wait = wait_for.as_ref().map(|wait_for| BrowserRuntimePageWait {
                selector: wait_for.selector.clone(),
                timeout_ms: wait_for.timeout_ms,
            });

            crate::browser_runtime::render_page_html_with_wait(
                &executable_path,
                &self.runtime_dir,
                url.as_str(),
                runtime_wait.as_ref(),
            )
            .await
        })
    }
}
