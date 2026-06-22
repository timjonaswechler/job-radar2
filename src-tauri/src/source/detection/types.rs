use std::{collections::HashMap, future::Future, path::Path, pin::Pin, time::Duration};

use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    declarative::template::{
        render_template, title_case, to_technical_key, TemplateContext, TemplateError,
    },
    simple_json_path::simple_json_path_exists,
    source::registry::{
        self, AvailabilityBlock, DetectionBlock, DetectionPhase, ProfileAccessPathDefinition,
        RegistrySourceProfile, SourceProfileIdentity, SourceRegistryDiagnostic,
        SourceRegistryDocumentKind,
    },
};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDetectionStatus {
    Detected,
    Ambiguous,
    Unsupported,
    BuiltInSource,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetectionResult {
    pub status: SourceDetectionStatus,
    pub adapter_key: Option<String>,
    pub profile_key: Option<String>,
    pub profile_name: Option<String>,
    pub path_key: Option<String>,
    pub path_name: Option<String>,
    pub key: Option<String>,
    pub name: Option<String>,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub source_config: Option<Value>,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
    pub matches: Vec<SourceDetectionMatch>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetectionMatch {
    pub adapter_key: String,
    pub profile_key: String,
    pub profile_name: String,
    pub path_key: String,
    pub path_name: Option<String>,
    pub key: String,
    pub name: String,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub source_config: Value,
    pub evidence: Vec<String>,
}
