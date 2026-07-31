#[path = "primitives/source_behavior/acceptance.rs"]
mod acceptance;
#[path = "primitives/browser.rs"]
mod browser;
#[path = "primitives/source_behavior/capture.rs"]
mod capture;
#[path = "primitives/source_behavior/cardinality.rs"]
mod cardinality;
#[path = "primitives/completeness/compiled_registration.rs"]
mod compiled_registration;
#[path = "definition/compiler/support.rs"]
mod compiler_support;
#[path = "definition/compiler/effective_profile.rs"]
mod effective_profile;
#[path = "definition/compiler/phase_naming.rs"]
mod phase_naming;
#[path = "definition/profile_compilation.rs"]
mod profile_compilation;
#[path = "definition/compiler/provenance.rs"]
mod provenance;
#[path = "definition/compiler/resolution.rs"]
mod resolution;
#[path = "definition/compiler/schema_v3.rs"]
mod schema_v3;
#[path = "definition/compiler/security_boundedness.rs"]
mod security_boundedness;
#[path = "definition/compiler/semantic_validation.rs"]
mod semantic_validation;

#[path = "primitives/completeness/completeness_failures.rs"]
mod completeness_failures;
#[path = "primitives/source_behavior/http_fetch.rs"]
mod http_fetch;
#[path = "primitives/pagination.rs"]
mod pagination;
#[path = "primitives/source_behavior/parse.rs"]
mod parse;
#[path = "primitives/source_behavior/predicate.rs"]
mod predicate;
#[path = "primitives/completeness/registry_structure.rs"]
mod registry_structure;
#[path = "primitives/completeness/schema_inventory.rs"]
mod schema_inventory;
#[path = "primitives/source_behavior/select.rs"]
mod select;
#[path = "primitives/completeness/serde_inventory.rs"]
mod serde_inventory;
#[path = "primitives/source_behavior/template.rs"]
mod template;
#[path = "primitives/source_behavior/transform.rs"]
mod transform;
#[path = "primitives/source_behavior/value.rs"]
mod value;
