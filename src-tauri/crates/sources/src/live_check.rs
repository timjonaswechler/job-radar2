//! Bounded Source Live Check execution, evidence, freshness, and admission.

use std::{
    fmt,
    io::ErrorKind as IoErrorKind,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use source_profile_dsl::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use source_profile_dsl::execution::{
    BoxedBrowserAcquisitionFuture, BrowserAcquisition, BrowserAcquisitionRequest,
    ProfileHttpClient, ProfileHttpError, ProfileHttpRequest, ProfileHttpResponse,
    RuntimeCancellation, RuntimeExecutionContext,
};

use crate::installed::{self, Store};

mod execution;
mod fingerprint;
mod fingerprints;
mod freshness;
mod persistence;
mod report;

#[cfg(test)]
mod fingerprint_tests;
#[cfg(test)]
mod freshness_tests;

pub use fingerprint::CheckFingerprint as Fingerprint;
pub use freshness::{
    CheckReportFreshness as Freshness, CheckReportFreshnessState as FreshnessState,
    CheckReportStaleDetail as StaleDetail, CheckReportStaleReason as StaleReason,
};
pub use report::{
    CheckReport as Report, CheckReportKind as ReportKind, CheckReportResult as ReportResult,
    CheckReportSubject as ReportSubject, CheckReportSubjectType as ReportSubjectType,
    CHECK_REPORT_SCHEMA_VERSION,
};

pub const LOGIC_VERSION: &str = "source-live-check/v2";

pub trait Clock: Send + Sync {
    fn now(&self) -> String;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> String {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let days = seconds.div_euclid(86_400);
        let seconds_of_day = seconds.rem_euclid(86_400);
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_parameter = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_parameter + 2) / 5 + 1;
        let month = month_parameter + if month_parameter < 10 { 3 } else { -9 };
        year += i64::from(month <= 2);
        let hour = seconds_of_day / 3_600;
        let minute = (seconds_of_day % 3_600) / 60;
        let second = seconds_of_day % 60;
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportState {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: ReportState,
    pub report: Option<Report>,
    pub freshness: Option<Freshness>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutcome {
    pub report: Report,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdmissionOutcome {
    Checked {
        report: Report,
    },
    Activated {
        report: Report,
        source: installed::SourceView,
    },
}

#[derive(Clone, Default)]
pub struct Context {
    pub cancellation: Option<Arc<dyn RuntimeCancellation>>,
}

#[derive(Clone)]
pub struct Operation {
    app_data_dir: PathBuf,
    installed: Store,
    http: Arc<dyn ProfileHttpClient + Send + Sync>,
    browser: Arc<dyn BrowserAcquisition>,
    clock: Arc<dyn Clock>,
}

impl Operation {
    pub fn new(
        installed: Store,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            app_data_dir: installed.app_data_dir().to_path_buf(),
            installed,
            http,
            browser,
            clock,
        }
    }

    pub async fn status(&self, source_key: &str) -> Result<Status, Error> {
        validate_key(source_key)?;
        let installed = self.installed.clone();
        let source_key = source_key.to_string();
        let app_data_dir = self.app_data_dir.clone();
        tokio::task::spawn_blocking(move || {
            let snapshot = installed.snapshot().map_err(Error::installed)?;
            if snapshot.source(&source_key).is_none() {
                return Err(Error::not_found(&source_key));
            }
            let path = persistence::source_live_check_report_path(app_data_dir, &source_key);
            match persistence::read_latest_check_report(path) {
                Ok(report) => {
                    let current =
                        execution::prepare(&snapshot, &source_key).map_err(Error::check)?;
                    let freshness = freshness::evaluate_check_report_freshness(
                        &report,
                        LOGIC_VERSION,
                        &current.fingerprints,
                    );
                    let state = match freshness.state {
                        freshness::CheckReportFreshnessState::Fresh => ReportState::Fresh,
                        freshness::CheckReportFreshnessState::Stale => ReportState::Stale,
                    };
                    Ok(Status {
                        state,
                        report: Some(report),
                        freshness: Some(freshness),
                    })
                }
                Err(persistence::CheckReportPersistenceError::Io(error))
                    if error.kind() == IoErrorKind::NotFound =>
                {
                    Ok(Status {
                        state: ReportState::Unknown,
                        report: None,
                        freshness: None,
                    })
                }
                Err(error) => Err(Error::storage(error.to_string())),
            }
        })
        .await
        .map_err(|error| Error::storage(error.to_string()))?
    }

    pub async fn run(&self, source_key: &str, context: Context) -> Result<RunOutcome, Error> {
        validate_key(source_key)?;
        let installed = self.installed.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .map_err(|error| Error::storage(error.to_string()))?
            .map_err(Error::installed)?;
        if snapshot.source(source_key).is_none() {
            return Err(Error::not_found(source_key));
        }
        let execution_context = execution::ExecutionContext {
            checked_at: self.clock.now(),
            cancellation: context.cancellation.as_deref(),
        };
        let http = SharedHttp(Arc::clone(&self.http));
        let browser = SharedBrowser(Arc::clone(&self.browser));
        let mut report = execution::build_report(
            &snapshot,
            source_key,
            &http,
            &http,
            &browser,
            &execution_context,
        )
        .await
        .map_err(Error::check)?;
        mark_cancelled_if_observed(&mut report, context.cancellation.as_deref());
        self.persist(&report).await?;
        Ok(RunOutcome { report })
    }

    pub async fn check_and_activate(
        &self,
        source_key: &str,
        context: Context,
    ) -> Result<AdmissionOutcome, Error> {
        validate_key(source_key)?;
        let installed = self.installed.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .map_err(|error| Error::storage(error.to_string()))?
            .map_err(Error::installed)?;
        let prepared = snapshot
            .source(source_key)
            .ok_or_else(|| Error::not_found(source_key))?;
        if prepared.origin() != installed::Origin::Custom {
            return Err(Error::new(
                ErrorKind::BuiltIn,
                format!("built-in Source `{source_key}` cannot be mutated"),
            ));
        }
        if prepared.document().status == installed::SourceStatus::Active {
            return Err(Error::new(
                ErrorKind::InvalidLifecycle,
                "an active Source cannot be check-and-activated",
            ));
        }
        let generation = prepared.generation().clone();
        let execution_context = execution::ExecutionContext {
            checked_at: self.clock.now(),
            cancellation: context.cancellation.as_deref(),
        };
        let http = SharedHttp(Arc::clone(&self.http));
        let browser = SharedBrowser(Arc::clone(&self.browser));
        let mut report = execution::build_report(
            &snapshot,
            source_key,
            &http,
            &http,
            &browser,
            &execution_context,
        )
        .await
        .map_err(Error::check)?;
        mark_cancelled_if_observed(&mut report, context.cancellation.as_deref());
        self.persist(&report).await?;
        if report.result != ReportResult::Passed {
            return Ok(AdmissionOutcome::Checked { report });
        }
        let installed = self.installed.clone();
        let source_key = source_key.to_string();
        let cancellation = context.cancellation.clone();
        let source = tokio::task::spawn_blocking(move || {
            installed.admit_checked_if(&source_key, &generation, || {
                !cancellation
                    .as_deref()
                    .is_some_and(RuntimeCancellation::is_cancelled)
            })
        })
        .await
        .map_err(|error| Error::storage(error.to_string()))?
        .map_err(Error::installed)?;
        let Some(source) = source else {
            mark_cancelled(&mut report);
            self.persist(&report).await?;
            return Ok(AdmissionOutcome::Checked { report });
        };
        Ok(AdmissionOutcome::Activated { report, source })
    }

    async fn persist(&self, report: &Report) -> Result<(), Error> {
        let app_data_dir = self.app_data_dir.clone();
        let report = report.clone();
        tokio::task::spawn_blocking(move || {
            persistence::persist_latest_check_report(app_data_dir, &report)
        })
        .await
        .map_err(|error| Error::storage(error.to_string()))?
        .map(|_| ())
        .map_err(|error| Error::storage(error.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidKey,
    NotFound,
    BuiltIn,
    InvalidLifecycle,
    StaleGeneration,
    LimitExceeded,
    Storage,
    Check,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

impl Error {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn not_found(key: &str) -> Self {
        Self::new(ErrorKind::NotFound, format!("Source `{key}` was not found"))
    }
    fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, message)
    }
    fn check(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Check, message)
    }
    fn installed(error: installed::Error) -> Self {
        let kind = match error.kind {
            installed::ErrorKind::InvalidKey => ErrorKind::InvalidKey,
            installed::ErrorKind::NotFound => ErrorKind::NotFound,
            installed::ErrorKind::BuiltIn => ErrorKind::BuiltIn,
            installed::ErrorKind::InvalidLifecycle => ErrorKind::InvalidLifecycle,
            installed::ErrorKind::GenerationMismatch => ErrorKind::StaleGeneration,
            installed::ErrorKind::LimitExceeded => ErrorKind::LimitExceeded,
            installed::ErrorKind::Storage | installed::ErrorKind::Load => ErrorKind::Storage,
            installed::ErrorKind::Duplicate | installed::ErrorKind::InvalidInput => {
                unreachable!("Source Live Check does not perform authored Source mutations")
            }
        };
        Self::new(kind, error.message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn mark_cancelled_if_observed(report: &mut Report, cancellation: Option<&dyn RuntimeCancellation>) {
    if cancellation.is_some_and(RuntimeCancellation::is_cancelled) {
        mark_cancelled(report);
    }
}

fn mark_cancelled(report: &mut Report) {
    if report.result == ReportResult::Failed
        && report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_execution_cancelled")
    {
        return;
    }
    report.result = ReportResult::Failed;
    report.details.insert(
        "liveCheckState".to_string(),
        serde_json::Value::String("live_check_failed".to_string()),
    );
    report.diagnostics.push(Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "runtime_execution_cancelled".to_string(),
        message: "Source Live Check was cancelled before admission".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: None,
    });
}

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

fn validate_key(key: &str) -> Result<(), Error> {
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
        Err(Error::new(
            ErrorKind::InvalidKey,
            format!("invalid Source key `{key}`"),
        ))
    }
}
