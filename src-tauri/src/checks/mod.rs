pub(crate) mod fingerprints;
pub(crate) mod freshness;
pub(crate) mod persistence;
pub(crate) mod profile_verification;
pub(crate) mod report;

pub use fingerprints::CheckFingerprint;
pub use freshness::{
    evaluate_check_report_freshness, CheckReportFreshness, CheckReportFreshnessState,
    CheckReportStaleDetail, CheckReportStaleReason,
};
pub use persistence::{
    latest_check_report_path, persist_latest_check_report, read_latest_check_report,
    source_live_check_report_path, source_profile_verification_report_path,
    CheckReportPersistenceError,
};
pub use profile_verification::{
    derive_effective_verification_state_for_source_profile, fixture_pack_root,
    resolve_fixture_file_reference, resolve_fixture_manifest_reference, verify_source_profile,
    EffectiveVerificationState, FixtureManifest, FixtureManifestChecks,
    FixtureManifestDiscoveryExpect, FixtureManifestExpectedCandidate,
    FixtureManifestPostingDetailCase, FixtureManifestPostingDetailCheck,
    FixtureManifestPostingDetailExpect, FixtureManifestPostingDiscoveryCheck,
    FixtureManifestPostingField, FixtureManifestPostingInput, FixtureManifestRequestMapping,
    FixtureManifestRequestMatch, FixtureManifestRequestMethod, FixtureManifestResponse,
    FixturePathResolution, DEFAULT_FIXTURE_MANIFEST_REFERENCE, FIXTURE_MANIFEST_SCHEMA_VERSION,
    PROFILE_VERIFICATION_LOGIC_VERSION, SOURCE_PROFILE_FIXTURES_DIR,
};
pub use report::{
    CheckReport, CheckReportKind, CheckReportResult, CheckReportSubject, CheckReportSubjectType,
    CHECK_REPORT_SCHEMA_VERSION,
};
