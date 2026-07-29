//! Executable contracts for the Source Profile documents shipped as product
//! resources. Generic DSL behavior belongs to `crates/source-profile-dsl/tests`.

#[path = "bundled_source_profiles/greenhouse.rs"]
mod greenhouse;
#[path = "bundled_source_profiles/successfactors.rs"]
mod successfactors;
#[path = "support/mod.rs"]
mod support;
#[path = "bundled_source_profiles/workday.rs"]
mod workday;
