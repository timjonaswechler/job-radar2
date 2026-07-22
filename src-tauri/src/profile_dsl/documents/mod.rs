#![allow(unused_imports)]

pub mod access_path;
pub mod detail;
pub mod detection;
pub mod discovery;
pub mod extract;
pub mod fetch;
pub mod fragments;
pub mod limits;
pub mod pagination;
pub mod parse;
pub mod select;
pub mod strategy;
pub mod support;

#[cfg(test)]
mod serde_tests;

pub use access_path::ReusableAccessPathDocument;
pub use detail::{DetailExtraction, DetailFields, DetailStep, DetailStrategy};
pub use detection::{
    DetectionBrowserInteraction, DetectionBrowserProbe, DetectionDocument, DetectionEvidence,
    DetectionEvidenceKind, DetectionHttpCheck, InputUrlPattern,
};
pub use discovery::{
    DiscoveryExtraction, DiscoveryHintExpression, DiscoveryProviderValues, DiscoveryReference,
    DiscoveryStep, DiscoveryStrategy,
};
pub use extract::{AuthoredScalar, CombinePart, FieldExpression, ListFieldExpression};
pub use fetch::{BrowserInteraction, BrowserWait, Fetch, HttpMethod, RequestBody};
pub use fragments::{
    AccessPathFragment, DetailStepFragment, DetailStrategyFragment, DiscoveryStepFragment,
    DiscoveryStrategyFragment, PaginationFragment, PaginationTypeFragment, ParseFragment,
    ParseTypeFragment, SelectTypeFragment,
};
pub use limits::{PhaseLimits, PhaseLimitsFragment};
pub use pagination::{Pagination, PaginationLimits, PaginationParameterLocation};
pub use parse::{Parse, ParseType};
pub use select::{CaptureRule, Captures, Select};
pub use strategy::Acceptance;
pub use support::{
    JsonObject, JsonSchemaObject, SupportEvidence, SupportEvidenceKind, SupportLevel,
    SupportMetadata, SupportNote,
};
