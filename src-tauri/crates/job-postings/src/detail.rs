mod acquisition;
mod diagnostics;

use crate::catalog::{self, Catalog, Id, Posting};
use serde::{Deserialize, Serialize};
use source_engine::{
    definition::Diagnostics,
    execution::{
        BoxedBrowserAcquisitionFuture, BrowserAcquisition, BrowserAcquisitionRequest,
        ProfileHttpClient, SourceBehaviorDetailExecution,
    },
};
use sources::installed::Store;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Description {
    Loaded {
        text: String,
        diagnostics: Diagnostics,
    },
    Unsupported {
        message: String,
        diagnostics: Diagnostics,
    },
    Failed {
        message: String,
        diagnostics: Diagnostics,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    #[serde(flatten)]
    pub posting: Posting,
    pub description_state: Description,
}

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error(transparent)]
    Catalog(#[from] catalog::Error),
    #[error("installed Source snapshot failed: {0}")]
    InstalledState(String),
    #[error("installed Source worker failed: {0}")]
    Worker(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("opening failed before mark-read: {0}")]
    BeforeRead(#[source] Failure),
    #[error("opening job posting {posting} failed after mark-read: {failure}")]
    AfterRead {
        posting: Id,
        #[source]
        failure: Failure,
    },
}

#[derive(Clone)]
struct SharedBrowser(Arc<dyn BrowserAcquisition>);

impl BrowserAcquisition for SharedBrowser {
    fn acquire<'a>(
        &'a self,
        request: BrowserAcquisitionRequest<'a>,
    ) -> BoxedBrowserAcquisitionFuture<'a> {
        self.0.acquire(request)
    }
}

/// Opens one Posting, owns its mark-read partial effect and lazy description cache.
#[derive(Clone)]
pub struct Detail {
    catalog: Catalog,
    installed: Store,
    http: Arc<dyn ProfileHttpClient + Send + Sync>,
    browser: SharedBrowser,
}

impl Detail {
    pub fn new(
        catalog: Catalog,
        installed: Store,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        Self {
            catalog,
            installed,
            http,
            browser: SharedBrowser(browser),
        }
    }

    pub async fn open(&self, id: Id) -> Result<Opened, Error> {
        let posting = self
            .catalog
            .mark_read(id)
            .await
            .map_err(|error| match error {
                catalog::MarkReadError::Before(error) => Error::BeforeRead(Failure::Catalog(error)),
                catalog::MarkReadError::After(error) => Error::AfterRead {
                    posting: id,
                    failure: Failure::Catalog(error),
                },
            })?;

        if let Some(text) = posting.description_text.clone() {
            return Ok(Opened {
                posting,
                description_state: Description::Loaded {
                    text,
                    diagnostics: Vec::new(),
                },
            });
        }

        let installed = self.installed.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .map_err(|error| Error::AfterRead {
                posting: id,
                failure: Failure::Worker(error.to_string()),
            })?
            .map_err(|error| Error::AfterRead {
                posting: id,
                failure: Failure::InstalledState(error.to_string()),
            })?;
        let execution = SourceBehaviorDetailExecution::new(self.http.as_ref(), &self.browser);
        let acquisition = acquisition::load(&posting, &snapshot, &execution).await;
        let (description, diagnostics) = match acquisition {
            Description::Loaded { text, diagnostics } => (text, diagnostics),
            Description::Unsupported {
                message,
                diagnostics,
            } => {
                return Ok(Opened {
                    posting,
                    description_state: Description::Unsupported {
                        message,
                        diagnostics,
                    },
                });
            }
            Description::Failed {
                message,
                diagnostics,
            } => {
                return Ok(Opened {
                    posting,
                    description_state: Description::Failed {
                        message,
                        diagnostics,
                    },
                });
            }
        };

        let posting = self
            .catalog
            .cache_description(id, &description)
            .await
            .map_err(|error| Error::AfterRead {
                posting: id,
                failure: Failure::Catalog(error),
            })?;
        let text = posting
            .description_text
            .clone()
            .expect("successful compare-and-set or concurrent winner leaves description");
        Ok(Opened {
            posting,
            description_state: Description::Loaded { text, diagnostics },
        })
    }
}
