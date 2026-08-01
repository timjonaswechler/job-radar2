//! Executable contracts for the Source Profile documents shipped as product
//! resources. Generic Source Behavior Language behavior belongs to `crates/source-engine/tests`.

#[allow(unused_imports)]
mod job_radar_lib {
    pub use ::job_radar_lib::*;
    pub use source_engine::test_support::*;
}

#[path = "bundled_source_profiles/greenhouse.rs"]
mod greenhouse;
#[path = "bundled_source_profiles/successfactors.rs"]
mod successfactors;
#[path = "support/mod.rs"]
mod support;
#[path = "bundled_source_profiles/workday.rs"]
mod workday;
