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

pub(super) fn parse_http_url(input: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string());
    }
    let with_protocol = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let url = Url::parse(&with_protocol)
        .map_err(|_| "Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())
    }
}

pub(super) fn built_in_source_message(url: &Url) -> Option<String> {
    let host = normalized_host(url);
    if host.contains("stepstone") || host.contains("indeed") {
        Some("StepStone und Indeed sind bereits als eingebaute Quellen vorhanden.".to_string())
    } else {
        None
    }
}

pub(super) fn origin(url: &Url) -> String {
    match url.port() {
        Some(port) => format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            port
        ),
        None => format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default()),
    }
}

pub(super) fn is_generic_host_label(label: &str) -> bool {
    matches!(
        label,
        "www"
            | "app"
            | "api"
            | "jobs"
            | "job"
            | "careers"
            | "career"
            | "join"
            | "boards"
            | "job-boards"
    )
}

pub(super) fn normalized_host(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}
