//! Tauri-free, Source-local Candidate Resolution.
//!
//! This crate owns compilation of Search Request matching requirements, resolution of
//! Posting Occurrences into finalized candidates.

mod normalization;
mod requirements;
mod resolver;
mod rules;

pub use requirements::{Requirements, RequirementsCompilationFailure};
pub use resolver::{
    Candidate, CandidateDiagnosticSummary, Resolution, ResolutionCompletion, ResolutionCounts,
    ResolutionError, ResolutionFailure, ResolutionLimitDimension, Resolver,
};
pub use rules::{SearchRule, SearchRuleKind, SearchRuleTarget};
