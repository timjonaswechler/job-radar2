#![allow(unused_imports)]

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
