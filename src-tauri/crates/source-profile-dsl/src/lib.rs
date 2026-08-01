//! Declarative Source behavior engine.
//!
//! The package name remains `source-profile-dsl` temporarily until issue #320.

pub mod definition;
pub mod detection;
pub mod execution;

#[cfg(feature = "test-support")]
pub mod test_support;
