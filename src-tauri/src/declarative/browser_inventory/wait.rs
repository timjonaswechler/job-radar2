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

use crate::{
    search::run::{SourceExecutionError, SourceExecutionSource},
    source::registry::BrowserInteraction,
};

use super::*;

pub(super) const DEFAULT_WAIT_TIMEOUT_MS: u64 = 15_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrowserInventoryWait {
    pub selector: String,
    pub timeout_ms: u64,
}

pub(super) fn parse_wait_for(
    source: &SourceExecutionSource,
) -> Result<Option<BrowserInventoryWait>, SourceExecutionError> {
    let Some(interactions) = source.interactions() else {
        return Ok(None);
    };

    for (index, interaction) in interactions.iter().enumerate() {
        match interaction {
            BrowserInteraction::WaitFor {
                selector,
                timeout_ms,
            } => {
                let path = plan_path(source, &format!("executionPlan.interactions[{index}]"));
                compile_selector(selector, &format!("{path}.selector"))?;
                let timeout_ms = timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
                if timeout_ms == 0 {
                    return Err(SourceExecutionError::Failed(format!(
                        "{path}.timeoutMs must be a positive integer"
                    )));
                }

                return Ok(Some(BrowserInventoryWait {
                    selector: selector.to_string(),
                    timeout_ms,
                }));
            }
            BrowserInteraction::ClickIfVisible { .. } | BrowserInteraction::ClickUpToN { .. } => {
                return Err(SourceExecutionError::Failed(format!(
                    "{} is not supported by the browser inventory executor yet",
                    plan_path(source, &format!("executionPlan.interactions[{index}]"))
                )));
            }
        }
    }

    Ok(None)
}
