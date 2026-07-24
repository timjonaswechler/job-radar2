use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::JsonObject;

pub use crate::profile_dsl::primitives::fetch::browser::{BrowserInteraction, BrowserWait};
pub(crate) use crate::profile_dsl::primitives::fetch::browser::{
    MAX_BROWSER_FETCH_TIMEOUT_MS, MAX_BROWSER_INTERACTION_COUNT, MAX_BROWSER_WAIT_AFTER_MS,
    MAX_BROWSER_WAIT_TIMEOUT_MS,
};

fn deserialize_http_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if (1..=60_000).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "HTTP timeoutMs must be between 1 and 60000",
        ))
    }
}

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
            deserialize_with = "crate::profile_dsl::primitives::fetch::browser::deserialize_browser_fetch_timeout"
        )]
        timeout_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        waits: Option<Vec<BrowserWait>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
    },
}

impl Fetch {
    pub const fn browser_descriptor(
        &self,
    ) -> Option<&'static crate::profile_dsl::primitives::fetch::browser::BrowserPrimitiveDescriptor>
    {
        match self {
            Self::Http { .. } => None,
            Self::Browser { .. } => {
                Some(&crate::profile_dsl::primitives::fetch::browser::BROWSER_FETCH_DESCRIPTOR)
            }
        }
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
