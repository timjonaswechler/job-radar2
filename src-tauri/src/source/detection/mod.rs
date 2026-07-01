#![allow(unused_imports)]

//! Legacy v1 Source detection. Kept temporarily until Source Proposal detection
//! is rebuilt on the declarative Source Profile DSL.

mod engine;
#[cfg(test)]
mod tests;
mod types;

pub use engine::detect_source_from_url;
pub use types::{SourceDetectionMatch, SourceDetectionResult, SourceDetectionStatus};

#[cfg(test)]
use self::engine::{
    detect_with_source_profiles, BoxedTextFuture, DetectionHttpClient, DetectionTemplateContext,
};
#[cfg(test)]
use crate::declarative::template::render_template;
#[cfg(test)]
use reqwest::Url;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use std::collections::HashMap;
