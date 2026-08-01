//! Host-facing installed Profile Detection.
//!
//! One [`Operation::run`] loads one immutable installed-state Snapshot, executes
//! only its admitted Profile Detection material, and returns an intentional
//! projection. It never persists a Source and owns no native adapter.

use std::{fmt, sync::Arc};

use serde::Serialize;
use source_engine::{
    definition::Diagnostics,
    detection::{
        execute_detection_operation, DetectionRunStatus, ReconciledSourceProposal,
        UnsupportedReconciledDetection,
    },
    execution::{
        BrowserAcquisition, PhaseBrowser, PhaseExecutionReport, ProfileHttpClient,
        RuntimeCancellation,
    },
};

use crate::installed::Store;

#[derive(Clone)]
pub struct Operation {
    installed: Store,
    http: Arc<dyn ProfileHttpClient + Send + Sync>,
    browser: Arc<dyn BrowserAcquisition>,
}

impl Operation {
    pub fn new(
        installed: Store,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        Self {
            installed,
            http,
            browser,
        }
    }

    pub async fn run(&self, request: Request, context: Context<'_>) -> Result<Outcome, Error> {
        let installed = self.installed.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .expect("installed Profile Detection snapshot task must run to completion")
            .map_err(Error::installed)?;
        let profile_diagnostics = snapshot.profiles.view.diagnostics.clone();
        let uncancelled = NeverCancelled;
        let cancellation = context.cancellation.unwrap_or(&uncancelled);
        let result = execute_detection_operation(
            &request.url,
            &snapshot.profiles.prepared_detection,
            self.http.as_ref(),
            PhaseBrowser::Browser(self.browser.as_ref()),
            cancellation,
        )
        .await;
        let status = if result.run_result.status == DetectionRunStatus::Failed
            && result.run_result.proposals.is_empty()
            && !result.run_result.unsupported_profiles.is_empty()
            && !result.has_profile_execution_failure()
        {
            // Admitted Profiles that simply do not match contribute no failure
            // Diagnostics. A positive unsupported match remains an intentional
            // unsupported application outcome rather than a probe failure.
            DetectionRunStatus::Unsupported
        } else {
            result.run_result.status
        };

        let diagnostics = if status == DetectionRunStatus::Failed && result.diagnostics.is_empty() {
            result.run_result.diagnostics.clone()
        } else {
            result.diagnostics
        };
        Ok(Outcome {
            status,
            proposals: result.run_result.proposals,
            unsupported_profiles: result.run_result.unsupported_profiles,
            profile_diagnostics,
            diagnostics,
            report: result.report,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,
}

#[derive(Clone, Copy, Default)]
pub struct Context<'a> {
    pub cancellation: Option<&'a dyn RuntimeCancellation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub status: DetectionRunStatus,
    pub proposals: Vec<ReconciledSourceProposal>,
    pub unsupported_profiles: Vec<UnsupportedReconciledDetection>,
    pub profile_diagnostics: Diagnostics,
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InstalledState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

impl Error {
    fn installed(error: crate::installed::Error) -> Self {
        Self {
            kind: ErrorKind::InstalledState,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

struct NeverCancelled;

impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}
