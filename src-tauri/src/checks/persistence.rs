use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::report::{CheckReport, CheckReportKind, CheckReportSubjectType};

const SOURCE_PROFILE_VERIFICATIONS_DIR: &str = "source-profile-verifications";
const SOURCE_LIVE_CHECKS_DIR: &str = "source-live-checks";

#[derive(Debug)]
pub enum CheckReportPersistenceError {
    SubjectKindMismatch {
        kind: CheckReportKind,
        subject_type: CheckReportSubjectType,
    },
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for CheckReportPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubjectKindMismatch { kind, subject_type } => write!(
                formatter,
                "check report kind {kind:?} cannot be persisted for subject type {subject_type:?}"
            ),
            Self::Io(error) => write!(formatter, "check report I/O error: {error}"),
            Self::Json(error) => write!(formatter, "check report JSON error: {error}"),
        }
    }
}

impl std::error::Error for CheckReportPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::SubjectKindMismatch { .. } => None,
        }
    }
}

impl From<io::Error> for CheckReportPersistenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for CheckReportPersistenceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn source_profile_verification_report_path(
    app_data_dir: impl AsRef<Path>,
    profile_key: impl AsRef<str>,
) -> PathBuf {
    app_data_dir
        .as_ref()
        .join(SOURCE_PROFILE_VERIFICATIONS_DIR)
        .join(format!("{}.json", profile_key.as_ref()))
}

pub fn source_live_check_report_path(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> PathBuf {
    app_data_dir
        .as_ref()
        .join(SOURCE_LIVE_CHECKS_DIR)
        .join(format!("{}.json", source_key.as_ref()))
}

pub fn latest_check_report_path(
    app_data_dir: impl AsRef<Path>,
    report: &CheckReport,
) -> Result<PathBuf, CheckReportPersistenceError> {
    match (report.kind, report.subject.subject_type) {
        (CheckReportKind::SourceProfileVerification, CheckReportSubjectType::SourceProfile) => Ok(
            source_profile_verification_report_path(app_data_dir, &report.subject.key),
        ),
        (CheckReportKind::SourceLiveCheck, CheckReportSubjectType::Source) => Ok(
            source_live_check_report_path(app_data_dir, &report.subject.key),
        ),
        (kind, subject_type) => {
            Err(CheckReportPersistenceError::SubjectKindMismatch { kind, subject_type })
        }
    }
}

pub fn persist_latest_check_report(
    app_data_dir: impl AsRef<Path>,
    report: &CheckReport,
) -> Result<PathBuf, CheckReportPersistenceError> {
    let path = latest_check_report_path(app_data_dir, report)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(&path, bytes)?;
    Ok(path)
}

pub fn read_latest_check_report(
    path: impl AsRef<Path>,
) -> Result<CheckReport, CheckReportPersistenceError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}
