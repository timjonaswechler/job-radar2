//! Admitted Search Run execution and atomic terminal persistence.

mod runner;

pub use runner::{
    Context, Error, Outcome, ResolutionSummary, Runner, SourceAdmission, SourceOutcome,
    SourceStatus, Status,
};
