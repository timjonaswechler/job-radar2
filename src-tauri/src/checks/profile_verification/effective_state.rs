use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::SupportLevel;

use super::super::{CheckReport, CheckReportFreshness, CheckReportKind, CheckReportResult};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveVerificationState {
    Verified,
    Failed,
    NotApplicable,
    Unknown,
}

pub fn derive_effective_verification_state_for_source_profile(
    declared_support_level: SupportLevel,
    fixture_evidence_present: bool,
    verification_report: Option<&CheckReport>,
    freshness: Option<&CheckReportFreshness>,
) -> EffectiveVerificationState {
    let Some(report) = verification_report else {
        return EffectiveVerificationState::Unknown;
    };
    if report.kind != CheckReportKind::SourceProfileVerification {
        return EffectiveVerificationState::Unknown;
    }

    let Some(freshness) = freshness else {
        return EffectiveVerificationState::Unknown;
    };
    if !freshness.is_fresh() {
        return EffectiveVerificationState::Unknown;
    }

    derive_effective_verification_state_from_fresh_report(
        declared_support_level,
        fixture_evidence_present,
        report.result,
        report
            .details
            .get("fixtureChecks")
            .and_then(|fixture_checks| fixture_checks.as_array())
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    )
}

pub(crate) fn derive_effective_verification_state_from_fresh_report(
    declared_support_level: SupportLevel,
    fixture_evidence_present: bool,
    check_result: CheckReportResult,
    fixture_checks: &[serde_json::Value],
) -> EffectiveVerificationState {
    if declared_support_level != SupportLevel::Verified {
        return EffectiveVerificationState::NotApplicable;
    }

    if !fixture_evidence_present {
        return EffectiveVerificationState::Failed;
    }

    if check_result != CheckReportResult::Passed {
        return EffectiveVerificationState::Failed;
    }

    if !fixture_checks_have_sufficient_verified_coverage(fixture_checks) {
        return EffectiveVerificationState::Failed;
    }

    EffectiveVerificationState::Verified
}

pub(crate) fn fixture_checks_have_sufficient_verified_coverage(
    fixture_checks: &[serde_json::Value],
) -> bool {
    fixture_checks.iter().any(|fixture_check| {
        fixture_check
            .get("result")
            .and_then(|result| result.as_str())
            == Some("passed")
            && fixture_check
                .pointer("/coverage/postingDiscovery")
                .and_then(|coverage| coverage.as_bool())
                == Some(true)
            && fixture_check
                .pointer("/coverage/postingDetailDescriptionText")
                .and_then(|coverage| coverage.as_bool())
                == Some(true)
    })
}
