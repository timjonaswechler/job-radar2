use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{fingerprints::CheckFingerprint, report::CheckReport};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckReportFreshnessState {
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckReportStaleReason {
    LogicVersionChanged,
    MissingReportFingerprint,
    ChangedFingerprintSha256,
    UnexpectedReportFingerprint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckReportStaleDetail {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    pub reason: CheckReportStaleReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_value: Option<String>,
}

impl CheckReportStaleDetail {
    fn logic_version_changed(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            kind: "logic_version".to_string(),
            reference: None,
            reason: CheckReportStaleReason::LogicVersionChanged,
            expected_sha256: None,
            actual_sha256: None,
            expected_value: Some(expected.into()),
            actual_value: Some(actual.into()),
        }
    }

    fn missing_report_fingerprint(expected: &CheckFingerprint) -> Self {
        Self {
            kind: expected.kind.clone(),
            reference: expected.reference.clone(),
            reason: CheckReportStaleReason::MissingReportFingerprint,
            expected_sha256: expected.sha256.clone(),
            actual_sha256: None,
            expected_value: None,
            actual_value: None,
        }
    }

    fn changed_fingerprint_sha256(expected: &CheckFingerprint, actual: &CheckFingerprint) -> Self {
        Self {
            kind: expected.kind.clone(),
            reference: expected.reference.clone(),
            reason: CheckReportStaleReason::ChangedFingerprintSha256,
            expected_sha256: expected.sha256.clone(),
            actual_sha256: actual.sha256.clone(),
            expected_value: None,
            actual_value: None,
        }
    }

    fn unexpected_report_fingerprint(actual: &CheckFingerprint) -> Self {
        Self {
            kind: actual.kind.clone(),
            reference: actual.reference.clone(),
            reason: CheckReportStaleReason::UnexpectedReportFingerprint,
            expected_sha256: None,
            actual_sha256: actual.sha256.clone(),
            expected_value: None,
            actual_value: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckReportFreshness {
    pub state: CheckReportFreshnessState,
    pub stale_fingerprints: Vec<CheckReportStaleDetail>,
}

impl CheckReportFreshness {
    pub fn is_fresh(&self) -> bool {
        self.state == CheckReportFreshnessState::Fresh
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FingerprintIdentity {
    kind: String,
    reference: Option<String>,
}

impl From<&CheckFingerprint> for FingerprintIdentity {
    fn from(fingerprint: &CheckFingerprint) -> Self {
        Self {
            kind: fingerprint.kind.clone(),
            reference: fingerprint.reference.clone(),
        }
    }
}

pub fn evaluate_check_report_freshness(
    report: &CheckReport,
    current_logic_version: impl AsRef<str>,
    current_fingerprints: &[CheckFingerprint],
) -> CheckReportFreshness {
    let current_logic_version = current_logic_version.as_ref();
    let mut stale_fingerprints = Vec::new();

    if report.logic_version != current_logic_version {
        stale_fingerprints.push(CheckReportStaleDetail::logic_version_changed(
            current_logic_version,
            &report.logic_version,
        ));
    }

    for current in current_fingerprints {
        match find_fingerprint(&report.fingerprints, current) {
            Some(report_fingerprint) if report_fingerprint.sha256 != current.sha256 => {
                stale_fingerprints.push(CheckReportStaleDetail::changed_fingerprint_sha256(
                    current,
                    report_fingerprint,
                ));
            }
            Some(_) => {}
            None => {
                stale_fingerprints
                    .push(CheckReportStaleDetail::missing_report_fingerprint(current));
            }
        }
    }

    let current_identities: HashSet<_> = current_fingerprints
        .iter()
        .map(FingerprintIdentity::from)
        .collect();
    for report_fingerprint in &report.fingerprints {
        if !current_identities.contains(&FingerprintIdentity::from(report_fingerprint)) {
            stale_fingerprints.push(CheckReportStaleDetail::unexpected_report_fingerprint(
                report_fingerprint,
            ));
        }
    }

    let state = if stale_fingerprints.is_empty() {
        CheckReportFreshnessState::Fresh
    } else {
        CheckReportFreshnessState::Stale
    };

    CheckReportFreshness {
        state,
        stale_fingerprints,
    }
}

fn find_fingerprint<'a>(
    fingerprints: &'a [CheckFingerprint],
    expected: &CheckFingerprint,
) -> Option<&'a CheckFingerprint> {
    let expected_identity = FingerprintIdentity::from(expected);
    fingerprints
        .iter()
        .find(|fingerprint| FingerprintIdentity::from(*fingerprint) == expected_identity)
}
