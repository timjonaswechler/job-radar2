#![allow(unused_imports)]

mod checks;
mod client;
mod config;
mod detector;
mod evaluation;
mod identity;
mod templates;
mod url;

use self::checks::*;
use self::client::*;
use self::config::*;
use self::evaluation::*;
use self::identity::*;
use self::templates::*;
use self::url::*;
use super::*;

pub use self::detector::detect_source_from_url;

#[cfg(test)]
pub(in crate::source::detection) use self::client::{BoxedTextFuture, DetectionHttpClient};
#[cfg(test)]
pub(in crate::source::detection) use self::evaluation::detect_with_source_profiles;
#[cfg(test)]
pub(in crate::source::detection) use self::templates::DetectionTemplateContext;
