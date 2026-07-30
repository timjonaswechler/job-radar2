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
pub(crate) use persistence::{
    persist_latest_check_report, read_latest_check_report, source_live_check_report_path,
    CheckReportPersistenceError,
};
pub use report::{
    CheckReport, CheckReportKind, CheckReportResult, CheckReportSubject, CheckReportSubjectType,
    CHECK_REPORT_SCHEMA_VERSION,
};
pub use source_behavior_fingerprints::{
    prepare_source_behavior_fingerprints, SourceBehaviorFingerprintPreparationError,
    SourceBehaviorFingerprintPreparationErrorKind,
};
pub(crate) use source_live::{
    build_source_live_check_report, source_live_check_report_status,
    SourceLiveCheckExecutionContext,
};
pub use source_live::{
    SourceLiveCheckReportState, SourceLiveCheckReportStatus, SOURCE_LIVE_CHECK_LOGIC_VERSION,
};
