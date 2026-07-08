use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::report::CheckReport;

const SOURCE_LIVE_CHECKS_DIR: &str = "source-live-checks";

#[derive(Debug)]
pub enum CheckReportPersistenceError {
    Io(io::Error),
    InvalidSourceKey(String),
    Json(serde_json::Error),
}

impl fmt::Display for CheckReportPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "check report I/O error: {error}"),
            Self::InvalidSourceKey(key) => write!(
                formatter,
                "invalid Source key `{key}` for Source Live Check report path"
            ),
            Self::Json(error) => write!(formatter, "check report JSON error: {error}"),
        }
    }
}

impl std::error::Error for CheckReportPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidSourceKey(_) => None,
            Self::Json(error) => Some(error),
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
    validate_source_live_check_report_key(&report.subject.key)?;
    Ok(source_live_check_report_path(
        app_data_dir,
        &report.subject.key,
    ))
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

pub(crate) fn validate_source_live_check_report_key(
    source_key: &str,
) -> Result<(), CheckReportPersistenceError> {
    if is_technical_key(source_key) {
        Ok(())
    } else {
        Err(CheckReportPersistenceError::InvalidSourceKey(
            source_key.to_string(),
        ))
    }
}

fn is_technical_key(key: &str) -> bool {
    !key.is_empty()
        && key.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}
