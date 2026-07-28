//! Tauri-free, Source-local Candidate Resolution.
//!
//! This crate owns compilation of Search Request matching requirements, resolution of
//! Posting Occurrences into finalized candidates, and canonical cross-Source posting comparison.

mod comparison;
mod normalization;
mod posting_matching;
mod resolution;
mod rules;

pub use posting_matching::{merge_unique_locations, same_job_posting, PostingComparison};
pub use resolution::*;
pub use rules::{SearchRule, SearchRuleKind, SearchRuleTarget};
