pub(crate) mod fingerprints;
pub(crate) mod freshness;
pub(crate) mod persistence;
pub(crate) mod report;
pub(crate) mod source_behavior_fingerprints;
pub(crate) mod source_live;

pub use fingerprints::CheckFingerprint;
pub use freshness::{
    evaluate_check_report_freshness, CheckReportFreshness, CheckReportFreshnessState,
    CheckReportStaleDetail, CheckReportStaleReason,
};
pub use persistence::{
    latest_check_report_path, persist_latest_check_report, read_latest_check_report,
    source_live_check_report_path, CheckReportPersistenceError,
};
pub use report::{
    CheckReport, CheckReportKind, CheckReportResult, CheckReportSubject, CheckReportSubjectType,
    CHECK_REPORT_SCHEMA_VERSION,
};
pub use source_behavior_fingerprints::{
    prepare_source_behavior_fingerprints, SourceBehaviorFingerprintPreparationError,
    SourceBehaviorFingerprintPreparationErrorKind,
};
pub use source_live::{
    check_and_activate_source, check_and_activate_source_with_clients,
    check_and_activate_source_with_fetcher, check_and_reactivate_source,
    check_and_reactivate_source_with_clients, check_and_reactivate_source_with_fetcher,
    check_source, check_source_with_clients, check_source_with_fetcher,
    source_live_check_report_status, SourceLiveCheckReportState, SourceLiveCheckReportStatus,
    SOURCE_LIVE_CHECK_LOGIC_VERSION,
};
