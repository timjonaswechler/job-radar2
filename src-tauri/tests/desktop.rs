//! Desktop composition and adapter contracts for the `job-radar` package.

#[allow(unused_imports)]
mod job_radar_lib {
    pub use ::job_radar_lib::*;
    pub use source_engine::test_support::*;
}

#[path = "desktop/agent.rs"]
mod agent;
#[path = "desktop/browser_acquisition.rs"]
mod browser_acquisition;
#[path = "desktop/browser_runtime.rs"]
mod browser_runtime;
#[path = "desktop/geo_resolution.rs"]
mod geo_resolution;
