use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::checks::{
    build_source_live_check_report, persist_latest_check_report, source_live_check_report_status,
    CheckReport, CheckReportResult, SourceLiveCheckExecutionContext, SourceLiveCheckReportStatus,
};
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{AccessPathFragment, JsonObject, SupportMetadata};
use crate::profile_dsl::runtime::{
    BoxedBrowserAcquisitionFuture, BrowserAcquisition, BrowserAcquisitionRequest, PhaseBrowser,
    PhaseCompletion, PhaseExecutionReport, PhaseUsage, ProfileHttpClient, ProfileHttpError,
    ProfileHttpRequest, ProfileHttpResponse, RuntimeCancellation, RuntimeExecutionContext,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
use crate::source::validation::SourceValidationState;
use crate::source_profile::detection::{
    compile_detection_plan, execute_detection_operation, DetectionRunStatus,
    ReconciledSourceProposal, UnsupportedReconciledDetection,
};
use crate::source_profile::registry::{RegistrySource, SourceProfileRegistrySnapshot};

#[derive(Clone)]
pub struct SourceOnboarding {
    app_data_dir: PathBuf,
    http: SharedHttp,
    browser: SharedBrowser,
    #[cfg(test)]
    snapshot_override: Option<SourceProfileRegistrySnapshot>,
}

impl SourceOnboarding {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        Self {
            app_data_dir: app_data_dir.into(),
            http: SharedHttp(http),
            browser: SharedBrowser(browser),
            #[cfg(test)]
            snapshot_override: None,
        }
    }

    pub async fn detect(
        &self,
        request: DetectSource,
        context: OperationContext<'_>,
    ) -> Result<DetectionOutcome, SourceOnboardingError> {
        let snapshot = self.snapshot();
        let mut plans = Vec::new();
        let mut diagnostics = Vec::new();
        for profile in snapshot
            .profiles
            .iter()
            .filter(|profile| profile.document.detection.is_some())
        {
            match compile_detection_plan(&profile.document) {
                Ok(plan) => plans.push(plan),
                Err(profile_diagnostics) => diagnostics.extend(profile_diagnostics),
            }
        }
        if !diagnostics.is_empty() {
            return Ok(DetectionOutcome {
                status: DetectionRunStatus::Failed,
                proposals: Vec::new(),
                unsupported_profiles: Vec::new(),
                diagnostics,
                report: PhaseExecutionReport {
                    usage: PhaseUsage::default(),
                    completion: PhaseCompletion::ExecutionFailed,
                },
            });
        }

        let uncancelled = NeverCancelled;
        let cancellation = context.cancellation.unwrap_or(&uncancelled);
        let result = execute_detection_operation(
            &request.url,
            &plans,
            &self.http,
            PhaseBrowser::Browser(&self.browser),
            cancellation,
        )
        .await;
        Ok(DetectionOutcome {
            status: result.run_result.status,
            proposals: result.run_result.proposals,
            unsupported_profiles: result.run_result.unsupported_profiles,
            diagnostics: result.diagnostics,
            report: result.report,
        })
    }

    pub async fn change(
        &self,
        change: SourceChange,
    ) -> Result<SourceChangeOutcome, SourceOnboardingError> {
        match change {
            SourceChange::CreateDraft(draft) => self.create_draft(draft),
            SourceChange::ReviseDefinition(revision) => self.revise_definition(revision),
            SourceChange::SetInactive { source_key, status } => {
                self.set_inactive(&source_key, status)
            }
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
                self.source(&source_key)?;
                let status = source_live_check_report_status(&self.app_data_dir, &source_key)
                    .map_err(SourceOnboardingError::check)?;
                Ok(SourceLiveCheckOutcome::LatestReportStatus(status))
            }
            SourceLiveCheckRequest::Run { source_key } => {
                let report = self.run_check(&source_key, context).await?;
                persist_latest_check_report(&self.app_data_dir, &report)
                    .map_err(|error| SourceOnboardingError::storage(error.to_string()))?;
                Ok(SourceLiveCheckOutcome::Checked {
                    report,
                    source: self.saved_source(&source_key)?,
                })
            }
            SourceLiveCheckRequest::CheckAndActivate { source_key } => {
                validate_key(&source_key)?;
                let snapshot = self.snapshot();
                let source = snapshot
                    .source(&source_key)
                    .cloned()
                    .ok_or_else(|| SourceOnboardingError::not_found(&source_key))?;
                self.ensure_custom(&source)?;
                if source.document.status == SourceStatus::Active {
                    return Err(SourceOnboardingError::invalid_lifecycle(
                        "an active Source cannot be check-and-activated",
                    ));
                }
                if !matches!(
                    source.document.status,
                    SourceStatus::Draft | SourceStatus::Disabled
                ) {
                    return Err(SourceOnboardingError::invalid_lifecycle(
                        "Source is not activatable",
                    ));
                }
                let execution_context = self.check_context(context);
                let report = build_source_live_check_report(
                    &snapshot,
                    &source_key,
                    &self.http,
                    &self.http,
                    &self.browser,
                    &execution_context,
                )
                .await
                .map_err(SourceOnboardingError::check)?;

                // The report is durable admission evidence before lifecycle mutation. If either
                // write fails, the persisted Source remains inactive.
                persist_latest_check_report(&self.app_data_dir, &report)
                    .map_err(|error| SourceOnboardingError::storage(error.to_string()))?;
                if report.result != CheckReportResult::Passed {
                    return Ok(SourceLiveCheckOutcome::Checked {
                        report,
                        source: SavedSource::from_registry(&source),
                    });
                }
                let source_path = PathBuf::from(&source.path);
                let mut activated = source.document;
                activated.status = SourceStatus::Active;
                self.write_source_at(&activated, &source_path, true)?;
                let source = self.saved_source(&source_key)?;
                Ok(SourceLiveCheckOutcome::Activated { report, source })
            }
        }
    }

    fn create_draft(
        &self,
        draft: CreateSourceDraft,
    ) -> Result<SourceChangeOutcome, SourceOnboardingError> {
        validate_key(&draft.key)?;
        if self.snapshot().source(&draft.key).is_some() {
            return Err(SourceOnboardingError::duplicate(&draft.key));
        }
        let document = draft.into_document();
        let path = self
            .app_data_dir
            .join("sources")
            .join(format!("{}.json", document.key));
        self.write_source_at(&document, &path, false)?;
        Ok(SourceChangeOutcome {
            source: self.saved_source(&document.key)?,
        })
    }

    fn revise_definition(
        &self,
        revision: ReviseSourceDefinition,
    ) -> Result<SourceChangeOutcome, SourceOnboardingError> {
        validate_key(&revision.key)?;
        let existing = self.source(&revision.key)?;
        self.ensure_custom(&existing)?;
        let path = PathBuf::from(&existing.path);
        let document = revision.into_document(existing.document.status);
        self.write_source_at(&document, &path, true)?;
        Ok(SourceChangeOutcome {
            source: self.saved_source(&document.key)?,
        })
    }

    fn set_inactive(
        &self,
        source_key: &str,
        status: InactiveSourceStatus,
    ) -> Result<SourceChangeOutcome, SourceOnboardingError> {
        validate_key(source_key)?;
        let existing = self.source(source_key)?;
        self.ensure_custom(&existing)?;
        let path = PathBuf::from(&existing.path);
        let mut document = existing.document;
        document.status = status.into();
        self.write_source_at(&document, &path, true)?;
        Ok(SourceChangeOutcome {
            source: self.saved_source(source_key)?,
        })
    }

    async fn run_check(
        &self,
        source_key: &str,
        context: OperationContext<'_>,
    ) -> Result<CheckReport, SourceOnboardingError> {
        validate_key(source_key)?;
        self.source(source_key)?;
        let snapshot = self.snapshot();
        let execution_context = self.check_context(context);
        build_source_live_check_report(
            &snapshot,
            source_key,
            &self.http,
            &self.http,
            &self.browser,
            &execution_context,
        )
        .await
        .map_err(SourceOnboardingError::check)
    }

    fn check_context<'a>(
        &self,
        context: OperationContext<'a>,
    ) -> SourceLiveCheckExecutionContext<'a> {
        let mut result = SourceLiveCheckExecutionContext::default();
        if let Some(checked_at) = context.checked_at {
            result = result.with_checked_at(checked_at);
        }
        if let Some(cancellation) = context.cancellation {
            result = result.with_cancellation(cancellation);
        }
        result
    }

    fn snapshot(&self) -> SourceProfileRegistrySnapshot {
        #[cfg(test)]
        if let Some(snapshot) = &self.snapshot_override {
            return snapshot.clone();
        }
        crate::source_profile::registry::load_snapshot(&self.app_data_dir)
    }

    #[cfg(test)]
    fn with_snapshot_for_test(mut self, snapshot: SourceProfileRegistrySnapshot) -> Self {
        self.snapshot_override = Some(snapshot);
        self
    }

    fn source(&self, key: &str) -> Result<RegistrySource, SourceOnboardingError> {
        self.snapshot()
            .source(key)
            .cloned()
            .ok_or_else(|| SourceOnboardingError::not_found(key))
    }

    fn saved_source(&self, key: &str) -> Result<SavedSource, SourceOnboardingError> {
        self.source(key)
            .map(|source| SavedSource::from_registry(&source))
    }

    fn ensure_custom(&self, source: &RegistrySource) -> Result<(), SourceOnboardingError> {
        if source.origin == "custom" {
            Ok(())
        } else {
            Err(SourceOnboardingError::built_in(&source.document.key))
        }
    }

    fn write_source_at(
        &self,
        document: &SourceDocument,
        path: &Path,
        replace: bool,
    ) -> Result<(), SourceOnboardingError> {
        let directory = path.parent().ok_or_else(|| {
            SourceOnboardingError::storage("Source document path has no parent directory")
        })?;
        std::fs::create_dir_all(directory)
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))?;
        if !replace && path.exists() {
            return Err(SourceOnboardingError::duplicate(&document.key));
        }
        if replace && !path.exists() {
            return Err(SourceOnboardingError::not_found(&document.key));
        }
        let mut bytes = serde_json::to_vec_pretty(document)
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))?;
        bytes.push(b'\n');
        crate::atomic_file::replace(path, &bytes)
            .map_err(|error| SourceOnboardingError::storage(error.to_string()))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectSource {
    pub url: String,
}

#[derive(Clone, Copy, Default)]
pub struct OperationContext<'a> {
    pub checked_at: Option<&'a str>,
    pub cancellation: Option<&'a dyn RuntimeCancellation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionOutcome {
    pub status: crate::source_profile::detection::DetectionRunStatus,
    pub proposals: Vec<ReconciledSourceProposal>,
    pub unsupported_profiles: Vec<UnsupportedReconciledDetection>,
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceChange {
    CreateDraft(CreateSourceDraft),
    ReviseDefinition(ReviseSourceDefinition),
    SetInactive {
        source_key: String,
        status: InactiveSourceStatus,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InactiveSourceStatus {
    Draft,
    Disabled,
}
impl From<InactiveSourceStatus> for SourceStatus {
    fn from(value: InactiveSourceStatus) -> Self {
        match value {
            InactiveSourceStatus::Draft => Self::Draft,
            InactiveSourceStatus::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSourceDraft {
    pub key: String,
    pub name: String,
    pub source_config: JsonObject,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default)]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(default)]
    pub source_support: Option<SupportMetadata>,
}
impl CreateSourceDraft {
    fn into_document(self) -> SourceDocument {
        SourceDocument {
            schema_version: 3,
            key: self.key,
            name: self.name,
            status: SourceStatus::Draft,
            source_config: self.source_config,
            selected_access_path: self.selected_access_path,
            access_paths: self.access_paths,
            source_support: self.source_support,
            diagnostics: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviseSourceDefinition {
    pub key: String,
    pub name: String,
    pub source_config: JsonObject,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default)]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(default)]
    pub source_support: Option<SupportMetadata>,
}
impl ReviseSourceDefinition {
    fn into_document(self, status: SourceStatus) -> SourceDocument {
        SourceDocument {
            schema_version: 3,
            key: self.key,
            name: self.name,
            status,
            source_config: self.source_config,
            selected_access_path: self.selected_access_path,
            access_paths: self.access_paths,
            source_support: self.source_support,
            diagnostics: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceChangeOutcome {
    pub source: SavedSource,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSource {
    pub origin: String,
    pub document: SourceDocument,
    pub validation_state: SourceValidationState,
}

impl SavedSource {
    fn from_registry(source: &RegistrySource) -> Self {
        Self {
            origin: source.origin.clone(),
            document: source.document.clone(),
            validation_state: source.validation_state.clone(),
        }
    }
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
        source: SavedSource,
    },
    Activated {
        report: CheckReport,
        source: SavedSource,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOnboardingErrorKind {
    InvalidKey,
    Duplicate,
    NotFound,
    BuiltIn,
    InvalidLifecycle,
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
    fn duplicate(key: &str) -> Self {
        Self::new(
            SourceOnboardingErrorKind::Duplicate,
            format!("Source `{key}` already exists"),
        )
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
}
impl fmt::Display for SourceOnboardingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for SourceOnboardingError {}

fn validate_key(key: &str) -> Result<(), SourceOnboardingError> {
    if !key.is_empty()
        && key.chars().all(|character| {
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

struct NeverCancelled;
impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_dsl::runtime::{ScriptedBrowserAcquisition, ScriptedProfileHttpClient};
    use crate::source::validation::ValidationStateKind;

    #[test]
    fn built_in_source_mutation_is_rejected() {
        let document: SourceDocument = serde_json::from_str(include_str!(
            "../tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json"
        ))
        .unwrap();
        let source = RegistrySource {
            origin: "built_in".to_string(),
            path: "embedded/source.json".to_string(),
            validation_state: SourceValidationState {
                source_key: document.key.clone(),
                state: ValidationStateKind::Valid,
                can_compile: true,
                can_execute: false,
                diagnostics: Vec::new(),
            },
            document,
            effective_profile: None,
            compile_outcome: None,
        };
        let onboarding = SourceOnboarding::new(
            tempfile::tempdir().unwrap().path(),
            Arc::new(ScriptedProfileHttpClient::new([])),
            Arc::new(ScriptedBrowserAcquisition::new([])),
        );

        let error = onboarding.ensure_custom(&source).unwrap_err();

        assert_eq!(error.kind, SourceOnboardingErrorKind::BuiltIn);
    }

    #[test]
    fn detect_exposes_authoritative_unsupported_status_for_isolated_test_registry() {
        let directory = tempfile::tempdir().unwrap();
        let mut profile: serde_json::Value =
            serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
        profile["support"]["level"] = serde_json::json!("unsupported");
        let profile_json = serde_json::to_string(&profile).unwrap();
        let snapshot = crate::source_profile::registry::load_snapshot_with_builtins(
            directory.path(),
            &[("source-profiles/builtin/greenhouse.json", &profile_json)],
            &[],
        );
        assert_eq!(
            snapshot.profiles.len(),
            1,
            "synthetic profile diagnostics: {:?}",
            snapshot.diagnostics
        );
        let onboarding = SourceOnboarding::new(
            directory.path(),
            Arc::new(ScriptedProfileHttpClient::new([])),
            Arc::new(ScriptedBrowserAcquisition::new([])),
        )
        .with_snapshot_for_test(snapshot);
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(onboarding.detect(
                DetectSource {
                    url: "https://boards.greenhouse.io/acme".to_string(),
                },
                OperationContext::default(),
            ))
            .unwrap();

        assert_eq!(result.status, DetectionRunStatus::Unsupported);
        assert_eq!(result.unsupported_profiles.len(), 1);
    }
}
