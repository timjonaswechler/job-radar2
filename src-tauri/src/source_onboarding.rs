use std::{fmt, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use source_profile_dsl::execution::{
    BoxedBrowserAcquisitionFuture, BrowserAcquisition, BrowserAcquisitionRequest,
    ProfileHttpClient, ProfileHttpError, ProfileHttpRequest, ProfileHttpResponse,
    RuntimeCancellation, RuntimeExecutionContext,
};
use sources::installed::{Snapshot, SourceStatus, SourceView, Store};

use crate::checks::{
    build_source_live_check_report, persist_latest_check_report, source_live_check_report_status,
    CheckReport, CheckReportResult, SourceLiveCheckExecutionContext, SourceLiveCheckReportStatus,
};

#[derive(Clone)]
pub struct SourceOnboarding {
    app_data_dir: PathBuf,
    installed: Store,
    http: SharedHttp,
    browser: SharedBrowser,
}

impl SourceOnboarding {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        let app_data_dir = app_data_dir.into();
        Self::with_store(
            app_data_dir.clone(),
            Store::new(&app_data_dir),
            http,
            browser,
        )
    }
    pub(crate) fn with_store(
        app_data_dir: impl Into<PathBuf>,
        installed: Store,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        Self {
            app_data_dir: app_data_dir.into(),
            installed,
            http: SharedHttp(http),
            browser: SharedBrowser(browser),
        }
    }

    pub async fn live_check(
        &self,
        request: SourceLiveCheckRequest,
        context: OperationContext<'_>,
    ) -> Result<SourceLiveCheckOutcome, SourceOnboardingError> {
        match request {
            SourceLiveCheckRequest::LatestReportStatus { source_key } => {
                validate_key(&source_key)?;
                let snapshot = self.snapshot().await?;
                source(&snapshot, &source_key)?;
                let status =
                    source_live_check_report_status(&self.app_data_dir, &snapshot, &source_key)
                        .await
                        .map_err(SourceOnboardingError::check)?;
                Ok(SourceLiveCheckOutcome::LatestReportStatus(status))
            }
            SourceLiveCheckRequest::Run { source_key } => {
                validate_key(&source_key)?;
                let snapshot = self.snapshot().await?;
                source(&snapshot, &source_key)?;
                let report = self.run_check(&snapshot, &source_key, context).await?;
                self.persist_report(&report).await?;
                Ok(SourceLiveCheckOutcome::Checked {
                    report,
                    source: view(&snapshot, &source_key)?,
                })
            }
            SourceLiveCheckRequest::CheckAndActivate { source_key } => {
                validate_key(&source_key)?;
                let snapshot = self.snapshot().await?;
                let prepared = source(&snapshot, &source_key)?;
                if prepared.origin() != sources::installed::Origin::Custom {
                    return Err(SourceOnboardingError::built_in(&source_key));
                }
                if prepared.document().status == SourceStatus::Active {
                    return Err(SourceOnboardingError::invalid_lifecycle(
                        "an active Source cannot be check-and-activated",
                    ));
                }
                let generation = prepared.generation().clone();
                let report = self.run_check(&snapshot, &source_key, context).await?;
                self.persist_report(&report).await?;
                if report.result != CheckReportResult::Passed {
                    return Ok(SourceLiveCheckOutcome::Checked {
                        report,
                        source: view(&snapshot, &source_key)?,
                    });
                }
                let installed = self.installed.clone();
                let checked_key = source_key.clone();
                let source = tokio::task::spawn_blocking(move || {
                    installed.admit_checked(&checked_key, &generation)
                })
                .await
                .map_err(|error| SourceOnboardingError::storage(error.to_string()))?
                .map_err(SourceOnboardingError::installed)?;
                Ok(SourceLiveCheckOutcome::Activated { report, source })
            }
        }
    }

    async fn run_check(
        &self,
        snapshot: &Snapshot,
        key: &str,
        context: OperationContext<'_>,
    ) -> Result<CheckReport, SourceOnboardingError> {
        build_source_live_check_report(
            snapshot,
            key,
            &self.http,
            &self.http,
            &self.browser,
            &check_context(context),
        )
        .await
        .map_err(SourceOnboardingError::check)
    }
    async fn persist_report(&self, report: &CheckReport) -> Result<(), SourceOnboardingError> {
        let app_data_dir = self.app_data_dir.clone();
        let report = report.clone();
        tokio::task::spawn_blocking(move || persist_latest_check_report(app_data_dir, &report))
            .await
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))?
            .map(|_| ())
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))
    }

    async fn snapshot(&self) -> Result<Snapshot, SourceOnboardingError> {
        let installed = self.installed.clone();
        tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))?
            .map_err(SourceOnboardingError::installed)
    }
}

fn validate_key(key: &str) -> Result<(), SourceOnboardingError> {
    let mut characters = key.chars();
    if characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        Ok(())
    } else {
        Err(SourceOnboardingError::new(
            SourceOnboardingErrorKind::InvalidKey,
            format!("invalid Source key `{key}`"),
        ))
    }
}
fn source<'a>(
    snapshot: &'a Snapshot,
    key: &str,
) -> Result<&'a sources::installed::PreparedSource, SourceOnboardingError> {
    snapshot
        .source(key)
        .ok_or_else(|| SourceOnboardingError::not_found(key))
}
fn view(snapshot: &Snapshot, key: &str) -> Result<SourceView, SourceOnboardingError> {
    snapshot
        .view()
        .sources
        .iter()
        .find(|item| item.document.key == key)
        .cloned()
        .ok_or_else(|| SourceOnboardingError::not_found(key))
}
fn check_context(context: OperationContext<'_>) -> SourceLiveCheckExecutionContext<'_> {
    let mut result = SourceLiveCheckExecutionContext::default();
    if let Some(checked_at) = context.checked_at {
        result = result.with_checked_at(checked_at);
    }
    if let Some(cancellation) = context.cancellation {
        result = result.with_cancellation(cancellation);
    }
    result
}

#[derive(Clone, Copy, Default)]
pub struct OperationContext<'a> {
    pub checked_at: Option<&'a str>,
    pub cancellation: Option<&'a dyn RuntimeCancellation>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceLiveCheckRequest {
    LatestReportStatus { source_key: String },
    Run { source_key: String },
    CheckAndActivate { source_key: String },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceLiveCheckOutcome {
    LatestReportStatus(SourceLiveCheckReportStatus),
    Checked {
        report: CheckReport,
        source: SourceView,
    },
    Activated {
        report: CheckReport,
        source: SourceView,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOnboardingErrorKind {
    InvalidKey,
    NotFound,
    BuiltIn,
    InvalidLifecycle,
    GenerationMismatch,
    LimitExceeded,
    Storage,
    Check,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOnboardingError {
    pub kind: SourceOnboardingErrorKind,
    pub message: String,
}
impl SourceOnboardingError {
    fn new(kind: SourceOnboardingErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn not_found(key: &str) -> Self {
        Self::new(
            SourceOnboardingErrorKind::NotFound,
            format!("Source `{key}` was not found"),
        )
    }
    fn built_in(key: &str) -> Self {
        Self::new(
            SourceOnboardingErrorKind::BuiltIn,
            format!("built-in Source `{key}` cannot be mutated"),
        )
    }
    fn invalid_lifecycle(message: impl Into<String>) -> Self {
        Self::new(SourceOnboardingErrorKind::InvalidLifecycle, message)
    }
    fn storage(message: impl Into<String>) -> Self {
        Self::new(SourceOnboardingErrorKind::Storage, message)
    }
    fn check(message: impl Into<String>) -> Self {
        Self::new(SourceOnboardingErrorKind::Check, message)
    }
    fn installed(error: sources::installed::Error) -> Self {
        let kind = match error.kind {
            sources::installed::ErrorKind::InvalidKey => SourceOnboardingErrorKind::InvalidKey,
            sources::installed::ErrorKind::NotFound => SourceOnboardingErrorKind::NotFound,
            sources::installed::ErrorKind::BuiltIn => SourceOnboardingErrorKind::BuiltIn,
            sources::installed::ErrorKind::InvalidLifecycle => {
                SourceOnboardingErrorKind::InvalidLifecycle
            }
            sources::installed::ErrorKind::GenerationMismatch => {
                SourceOnboardingErrorKind::GenerationMismatch
            }
            sources::installed::ErrorKind::LimitExceeded => {
                SourceOnboardingErrorKind::LimitExceeded
            }
            sources::installed::ErrorKind::Storage | sources::installed::ErrorKind::Load => {
                SourceOnboardingErrorKind::Storage
            }
            sources::installed::ErrorKind::Duplicate
            | sources::installed::ErrorKind::InvalidInput => {
                unreachable!("Source Onboarding does not perform authored Source mutations")
            }
        };
        Self::new(kind, error.message)
    }
}
impl fmt::Display for SourceOnboardingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SourceOnboardingError {}

#[derive(Clone)]
struct SharedHttp(Arc<dyn ProfileHttpClient + Send + Sync>);
impl ProfileHttpClient for SharedHttp {
    fn fetch<'a>(
        &'a self,
        request: ProfileHttpRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ProfileHttpResponse, ProfileHttpError>>
                + Send
                + 'a,
        >,
    > {
        self.0.fetch(request, context)
    }
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
