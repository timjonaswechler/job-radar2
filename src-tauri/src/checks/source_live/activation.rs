use std::path::{Path, PathBuf};

use crate::profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::profile_dsl::runtime::{
    DetailFetcher, DiscoveryFetcher, ProfileBrowserClient, ReqwestDetailFetcher,
    ReqwestDiscoveryFetcher, UnavailableProfileBrowserClient,
};
use crate::source::documents::{SourceDocument, SourceStatus};
use crate::source_profile::registry::RegistrySource;

use super::build_source_live_check_report;
use crate::checks::persistence::validate_source_live_check_report_key;
use crate::checks::{persist_latest_check_report, CheckReport, CheckReportResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceActivationFlow {
    Activate,
    Reactivate,
}

impl SourceActivationFlow {
    fn expected_current_status(self) -> SourceStatus {
        match self {
            Self::Activate => SourceStatus::Draft,
            Self::Reactivate => SourceStatus::Disabled,
        }
    }

    fn requested_status(self) -> SourceStatus {
        SourceStatus::Active
    }
}

pub fn check_and_activate_source(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> Result<CheckReport, String> {
    check_and_activate_source_with_clients(
        app_data_dir,
        source_key,
        &ReqwestDiscoveryFetcher::new(),
        &ReqwestDetailFetcher::new(),
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_and_activate_source_with_fetcher<F>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    fetcher: &F,
) -> Result<CheckReport, String>
where
    F: DiscoveryFetcher + DetailFetcher + Sync + ?Sized,
{
    check_and_activate_source_with_clients(
        app_data_dir,
        source_key,
        fetcher,
        fetcher,
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_and_activate_source_with_clients<D, T, B>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    browser: &B,
) -> Result<CheckReport, String>
where
    D: DiscoveryFetcher + Sync + ?Sized,
    T: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    check_and_set_source_active(
        app_data_dir,
        source_key,
        SourceActivationFlow::Activate,
        discovery_fetcher,
        detail_fetcher,
        browser,
    )
}

pub fn check_and_reactivate_source(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> Result<CheckReport, String> {
    check_and_reactivate_source_with_clients(
        app_data_dir,
        source_key,
        &ReqwestDiscoveryFetcher::new(),
        &ReqwestDetailFetcher::new(),
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_and_reactivate_source_with_fetcher<F>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    fetcher: &F,
) -> Result<CheckReport, String>
where
    F: DiscoveryFetcher + DetailFetcher + Sync + ?Sized,
{
    check_and_reactivate_source_with_clients(
        app_data_dir,
        source_key,
        fetcher,
        fetcher,
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_and_reactivate_source_with_clients<D, T, B>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    browser: &B,
) -> Result<CheckReport, String>
where
    D: DiscoveryFetcher + Sync + ?Sized,
    T: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    check_and_set_source_active(
        app_data_dir,
        source_key,
        SourceActivationFlow::Reactivate,
        discovery_fetcher,
        detail_fetcher,
        browser,
    )
}

fn check_and_set_source_active<D, T, B>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    flow: SourceActivationFlow,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    browser: &B,
) -> Result<CheckReport, String>
where
    D: DiscoveryFetcher + Sync + ?Sized,
    T: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let app_data_dir = app_data_dir.as_ref();
    let source_key = source_key.as_ref();
    validate_source_live_check_report_key(source_key).map_err(|error| error.to_string())?;
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    let source = snapshot.source(source_key).cloned();
    let current_status = source.as_ref().map(|source| source.document.status);

    let mut report = build_source_live_check_report(
        &snapshot,
        source_key,
        discovery_fetcher,
        detail_fetcher,
        browser,
    )?;

    let live_check_passed = report.result == CheckReportResult::Passed;
    let transition_allowed = current_status == Some(flow.expected_current_status());

    if live_check_passed && transition_allowed {
        let source =
            source.ok_or_else(|| format!("Source `{source_key}` wurde nicht gefunden."))?;
        activate_custom_source(source)?;
        let updated_snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
        report.fingerprints = super::source_live_check_fingerprints(&updated_snapshot, source_key)?;
    } else {
        report.diagnostics.push(activation_blocked_diagnostic(
            source_key,
            current_status,
            flow.requested_status(),
            report.result,
        ));
        report.result = CheckReportResult::Failed;
        report.details.insert(
            "liveCheckState".to_string(),
            serde_json::Value::String("live_check_failed".to_string()),
        );
    }

    persist_latest_check_report(app_data_dir, &report).map_err(|error| error.to_string())?;
    Ok(report)
}

fn activate_custom_source(source: RegistrySource) -> Result<(), String> {
    if source.origin != "custom" {
        return Err(format!(
            "Source `{}` ist eingebaut und kann nicht überschrieben werden.",
            source.document.key
        ));
    }

    let mut document = source.document;
    document.status = SourceStatus::Active;
    write_source_document(PathBuf::from(source.path), &document)
}

fn write_source_document(path: PathBuf, document: &SourceDocument) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(document)
        .map_err(|error| format!("Source konnte nicht serialisiert werden: {error}"))?;
    std::fs::write(&path, format!("{contents}\n"))
        .map_err(|error| format!("Source konnte nicht geschrieben werden: {error}"))
}

fn activation_blocked_diagnostic(
    source_key: &str,
    current_status: Option<SourceStatus>,
    requested_status: SourceStatus,
    live_check_result: CheckReportResult,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_live_check.activation_blocked".to_string(),
        message: "Source activation was blocked because the live check failed or the status transition is not allowed".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "sourceKey": source_key,
            "currentStatus": current_status,
            "requestedStatus": requested_status,
            "liveCheckResult": live_check_result
        })),
    }
}
