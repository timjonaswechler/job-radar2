//! Search Run execution, atomic terminal persistence, and latest-run history projections.

mod history;
mod runner;

pub use history::{Error as HistoryError, History, Latest};
pub use runner::{
    Context, Error, Outcome, ResolutionSummary, Runner, SourceAdmission, SourceOutcome,
    SourceStatus, Status,
};
