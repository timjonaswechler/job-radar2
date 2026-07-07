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
    FixtureManifest, FixtureManifestChecks, FixtureManifestDiscoveryExpect,
    FixtureManifestExpectedCandidate, FixtureManifestPostingDetailCase,
    FixtureManifestPostingDetailCheck, FixtureManifestPostingDetailExpect,
    FixtureManifestPostingDiscoveryCheck, FixtureManifestPostingField, FixtureManifestPostingInput,
    FixtureManifestRequestMapping, FixtureManifestRequestMatch, FixtureManifestRequestMethod,
    FixtureManifestResponse, FIXTURE_MANIFEST_SCHEMA_VERSION,
};
pub use report::{
    CheckReport, CheckReportKind, CheckReportResult, CheckReportSubject, CheckReportSubjectType,
    CHECK_REPORT_SCHEMA_VERSION,
};
