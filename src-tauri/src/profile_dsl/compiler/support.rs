use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::SupportMetadata;

pub(super) fn validate_support_metadata(
    _support: &SupportMetadata,
    _path: &str,
    _details: serde_json::Value,
    _diagnostics: &mut Diagnostics,
) {
    // Support metadata is declarative product guidance. Concrete operational
    // confidence is derived from Source Live Checks, not profile-level evidence.
}
