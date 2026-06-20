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
    source_registry::{
        self, AvailabilityBlock, DetectionBlock, DetectionPhase, ProfileAccessPathDefinition,
        RegistrySourceProfile, SourceProfileIdentity, SourceRegistryDiagnostic,
        SourceRegistryDocumentKind,
    },
};

use super::*;

pub(in crate::source_detection) type BoxedTextFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(in crate::source_detection) trait DetectionHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_>;
}

pub(super) struct ReqwestDetectionHttpClient {
    client: reqwest::Client,
}

impl ReqwestDetectionHttpClient {
    pub(super) fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("JobRadarSourceDetection/0.1")
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

impl DetectionHttpClient for ReqwestDetectionHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            let mut last_error = None;
            for attempt in 0..3 {
                match self.client.get(url.clone()).send().await {
                    Ok(response) if response.status().is_success() => {
                        return response.text().await.map_err(|error| error.to_string());
                    }
                    Ok(response) => {
                        last_error = Some(format!(
                            "{} returned HTTP {}",
                            url.as_str(),
                            response.status()
                        ));
                    }
                    Err(error) => {
                        last_error =
                            Some(format!("{} could not be fetched: {error}", url.as_str()));
                    }
                }

                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }

            Err(last_error.unwrap_or_else(|| format!("{} could not be fetched", url.as_str())))
        })
    }
}
