#![allow(unused_imports)]

//! Legacy v1 Source detection internals retained for pre-existing tests while
//! the app boundary uses Source Proposal detection.

mod engine;
#[cfg(test)]
mod tests;
mod types;

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
