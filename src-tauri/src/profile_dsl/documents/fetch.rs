use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::JsonObject;

pub(crate) const MAX_BROWSER_FETCH_TIMEOUT_MS: u64 = 120_000;
pub(crate) const MAX_BROWSER_WAIT_TIMEOUT_MS: u64 = 60_000;
pub(crate) const MAX_BROWSER_INTERACTION_COUNT: u64 = 50;
pub(crate) const MAX_BROWSER_WAIT_AFTER_MS: u64 = 60_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fetch {
    Http {
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<HttpMethod>,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<BTreeMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<RequestBody>,
        #[serde(rename = "timeoutMs", deserialize_with = "deserialize_http_timeout")]
        timeout_ms: u64,
    },
    Browser {
        url: String,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_fetch_timeout"
        )]
        timeout_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        waits: Option<Vec<BrowserWait>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
    },
}

fn deserialize_http_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_u64(deserializer, 1, 60_000, "HTTP timeoutMs")
}

pub(crate) fn deserialize_browser_fetch_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_FETCH_TIMEOUT_MS,
        "Browser timeoutMs",
    )
}

pub(crate) fn deserialize_browser_wait_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_WAIT_TIMEOUT_MS,
        "Browser wait timeoutMs",
    )
}

pub(crate) fn deserialize_browser_interaction_count<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_u64(
        deserializer,
        1,
        MAX_BROWSER_INTERACTION_COUNT,
        "Browser interaction maxCount",
    )
}

pub(crate) fn deserialize_browser_wait_after<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<u64>::deserialize(deserializer)?;
    if value.is_none_or(|value| value <= MAX_BROWSER_WAIT_AFTER_MS) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(format!(
            "Browser interaction waitAfterMs must be between 0 and {MAX_BROWSER_WAIT_AFTER_MS}"
        )))
    }
}

pub(crate) fn deserialize_non_empty_selector<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        Err(serde::de::Error::custom(
            "Browser selector must not be empty",
        ))
    } else {
        Ok(value)
    }
}

fn deserialize_bounded_u64<'de, D>(
    deserializer: D,
    min: u64,
    max: u64,
    label: &str,
) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(format!(
            "{label} must be between {min} and {max}"
        )))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
}

impl Fetch {
    pub(crate) fn http_parts(
        &self,
    ) -> Option<(
        Option<HttpMethod>,
        &str,
        Option<&BTreeMap<String, String>>,
        Option<&RequestBody>,
        u64,
    )> {
        match self {
            Self::Http {
                method,
                url,
                headers,
                body,
                timeout_ms,
            } => Some((*method, url, headers.as_ref(), body.as_ref(), *timeout_ms)),
            Self::Browser { .. } => None,
        }
    }
}

impl HttpMethod {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestBody {
    Json { value: JsonObject },
    Text { value: String },
    Form { fields: BTreeMap<String, String> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserWait {
    Selector {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
    NetworkIdle {
        #[serde(
            rename = "timeoutMs",
            deserialize_with = "deserialize_browser_wait_timeout"
        )]
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserInteraction {
    ClickIfVisible {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        #[serde(deserialize_with = "deserialize_non_empty_selector")]
        selector: String,
        #[serde(
            rename = "maxCount",
            deserialize_with = "deserialize_browser_interaction_count"
        )]
        max_count: u64,
        #[serde(
            rename = "waitAfterMs",
            default,
            deserialize_with = "deserialize_browser_wait_after",
            skip_serializing_if = "Option::is_none"
        )]
        wait_after_ms: Option<u64>,
    },
}
