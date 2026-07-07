use serde::{de, Deserialize, Deserializer, Serialize};

use crate::profile_dsl::{diagnostics::Diagnostics, documents::JsonObject};

use super::fingerprints::CheckFingerprint;

pub const CHECK_REPORT_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckReportKind {
    SourceProfileVerification,
    SourceLiveCheck,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckReportSubjectType {
    SourceProfile,
    Source,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckReportSubject {
    #[serde(rename = "type")]
    pub subject_type: CheckReportSubjectType,
    pub key: String,
}

impl CheckReportSubject {
    pub fn source_profile(key: impl Into<String>) -> Self {
        Self {
            subject_type: CheckReportSubjectType::SourceProfile,
            key: key.into(),
        }
    }

    pub fn source(key: impl Into<String>) -> Self {
        Self {
            subject_type: CheckReportSubjectType::Source,
            key: key.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckReportResult {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub schema_version: u64,
    pub kind: CheckReportKind,
    pub subject: CheckReportSubject,
    pub checked_at: String,
    pub logic_version: String,
    pub result: CheckReportResult,
    pub fingerprints: Vec<CheckFingerprint>,
    pub diagnostics: Diagnostics,
    pub details: JsonObject,
}

impl<'de> Deserialize<'de> for CheckReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CheckReportUnchecked {
            schema_version: u64,
            kind: CheckReportKind,
            subject: CheckReportSubject,
            checked_at: String,
            logic_version: String,
            result: CheckReportResult,
            fingerprints: Vec<CheckFingerprint>,
            diagnostics: Diagnostics,
            details: JsonObject,
        }

        let unchecked = CheckReportUnchecked::deserialize(deserializer)?;
        let report = CheckReport {
            schema_version: unchecked.schema_version,
            kind: unchecked.kind,
            subject: unchecked.subject,
            checked_at: unchecked.checked_at,
            logic_version: unchecked.logic_version,
            result: unchecked.result,
            fingerprints: unchecked.fingerprints,
            diagnostics: unchecked.diagnostics,
            details: unchecked.details,
        };

        validate_report_contract(&report).map_err(de::Error::custom)?;
        Ok(report)
    }
}

fn validate_report_contract(report: &CheckReport) -> Result<(), String> {
    if report.schema_version != CHECK_REPORT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported Check Report schemaVersion {}; expected {}",
            report.schema_version, CHECK_REPORT_SCHEMA_VERSION
        ));
    }

    match (report.kind, report.subject.subject_type) {
        (CheckReportKind::SourceProfileVerification, CheckReportSubjectType::SourceProfile)
        | (CheckReportKind::SourceLiveCheck, CheckReportSubjectType::Source) => Ok(()),
        (kind, subject_type) => Err(format!(
            "Check Report kind {kind:?} cannot use subject type {subject_type:?}"
        )),
    }
}

impl CheckReport {
    pub fn new(
        kind: CheckReportKind,
        subject: CheckReportSubject,
        checked_at: impl Into<String>,
        logic_version: impl Into<String>,
        result: CheckReportResult,
    ) -> Self {
        Self {
            schema_version: CHECK_REPORT_SCHEMA_VERSION,
            kind,
            subject,
            checked_at: checked_at.into(),
            logic_version: logic_version.into(),
            result,
            fingerprints: Vec::new(),
            diagnostics: Vec::new(),
            details: JsonObject::new(),
        }
    }
}
