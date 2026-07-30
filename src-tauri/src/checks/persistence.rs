use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::report::CheckReport;

const SOURCE_LIVE_CHECKS_DIR: &str = "source-live-checks";

#[derive(Debug)]
pub(crate) enum CheckReportPersistenceError {
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

pub(crate) fn source_live_check_report_path(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> PathBuf {
    app_data_dir
        .as_ref()
        .join(SOURCE_LIVE_CHECKS_DIR)
        .join(format!("{}.json", source_key.as_ref()))
}

pub(crate) fn latest_check_report_path(
    app_data_dir: impl AsRef<Path>,
    report: &CheckReport,
) -> Result<PathBuf, CheckReportPersistenceError> {
    validate_source_live_check_report_key(&report.subject.key)?;
    Ok(source_live_check_report_path(
        app_data_dir,
        &report.subject.key,
    ))
}

pub(crate) fn persist_latest_check_report(
    app_data_dir: impl AsRef<Path>,
    report: &CheckReport,
) -> Result<PathBuf, CheckReportPersistenceError> {
    let path = latest_check_report_path(app_data_dir, report)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(report)?;
    crate::atomic_file::replace(&path, &bytes)?;
    Ok(path)
}

pub(crate) fn read_latest_check_report(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckReportKind, CheckReportResult, CheckReportSubject};

    fn report(key: &str, checked_at: &str, result: CheckReportResult) -> CheckReport {
        CheckReport::new(
            CheckReportKind::SourceLiveCheck,
            CheckReportSubject::source(key),
            checked_at,
            "source-live-check/v1",
            result,
        )
    }

    #[test]
    fn latest_report_paths_use_overwriteable_source_live_check_location() {
        let app_data_dir = PathBuf::from("/tmp/job-radar-check-report-test");
        assert_eq!(
            source_live_check_report_path(&app_data_dir, "acme_jobs"),
            app_data_dir.join("source-live-checks/acme_jobs.json")
        );
        assert_eq!(
            latest_check_report_path(
                &app_data_dir,
                &report(
                    "acme_jobs",
                    "2026-07-07T12:00:00Z",
                    CheckReportResult::Failed,
                ),
            )
            .unwrap(),
            app_data_dir.join("source-live-checks/acme_jobs.json")
        );
    }

    #[test]
    fn latest_report_path_rejects_invalid_source_key() {
        let error = latest_check_report_path(
            "/tmp/job-radar-check-report-test",
            &report(
                "../outside",
                "2026-07-07T12:00:00Z",
                CheckReportResult::Failed,
            ),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("invalid Source key `../outside`"));
    }

    #[test]
    fn persistence_overwrites_latest_source_live_check_report() {
        let directory = tempfile::tempdir().unwrap();
        let first = report(
            "acme_jobs",
            "2026-07-07T12:00:00Z",
            CheckReportResult::Failed,
        );
        let path = persist_latest_check_report(directory.path(), &first).unwrap();
        assert_eq!(
            read_latest_check_report(&path).unwrap().result,
            CheckReportResult::Failed
        );

        let second = report(
            "acme_jobs",
            "2026-07-07T12:05:00Z",
            CheckReportResult::Passed,
        );
        let overwritten_path = persist_latest_check_report(directory.path(), &second).unwrap();

        assert_eq!(overwritten_path, path);
        assert_eq!(read_latest_check_report(&path).unwrap(), second);
    }
}
