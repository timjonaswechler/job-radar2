use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{SupportEvidenceKind, SupportLevel, SupportMetadata};

use super::compiler_error;

pub(super) fn validate_support_metadata(
    support: &SupportMetadata,
    path: &str,
    details: serde_json::Value,
    diagnostics: &mut Diagnostics,
) {
    if support.level == SupportLevel::Verified {
        let has_fixture_evidence = support.evidence.as_ref().is_some_and(|evidence| {
            evidence
                .iter()
                .any(|entry| entry.kind == SupportEvidenceKind::Fixture)
        });
        if !has_fixture_evidence {
            diagnostics.push(compiler_error(
                "verified_support_missing_fixture_evidence",
                "Verified support metadata must include fixture evidence",
                format!("{path}/evidence"),
                details,
            ));
        }
    }
}
