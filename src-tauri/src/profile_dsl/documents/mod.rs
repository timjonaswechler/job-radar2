#![allow(unused_imports)]

pub mod access_path;
pub mod extract;
pub mod fetch;
pub mod overrides;
pub mod pagination;
pub mod parse;
pub mod posting_detail;
pub mod posting_discovery;
pub mod select;
pub mod strategy;
pub mod support;
pub mod transform;

#[cfg(test)]
mod serde_tests;

pub use access_path::ReusableAccessPathDocument;
pub use extract::{Cardinality, CombinePart, FieldExpression, ListFieldExpression};
pub use fetch::{Fetch, HttpMethod, RequestBody};
pub use overrides::{OverridableStep, SourceOverrides, StrategyOverride};
pub use pagination::{Pagination, PaginationLimits};
pub use parse::{Parse, ParseType};
pub use posting_detail::{
    PostingDetailExtraction, PostingDetailFields, PostingDetailStep, PostingDetailStrategy,
};
pub use posting_discovery::{
    PostingDiscoveryExtraction, PostingDiscoveryFields, PostingDiscoveryStep,
    PostingDiscoveryStrategy,
};
pub use select::{CaptureRule, Captures, Filter, Select};
pub use strategy::{Acceptance, FieldMatch};
pub use support::{
    JsonObject, JsonSchemaObject, SupportEvidence, SupportEvidenceKind, SupportLevel,
    SupportMetadata, SupportNote,
};
pub use transform::Transform;
