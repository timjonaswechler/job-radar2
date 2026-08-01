use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::definition::documents::JsonObject;

pub use crate::definition::primitives::fetch::browser::{BrowserInteraction, BrowserWait};
pub use crate::definition::primitives::fetch::browser::{
    MAX_BROWSER_FETCH_TIMEOUT_MS, MAX_BROWSER_INTERACTION_COUNT, MAX_BROWSER_WAIT_AFTER_MS,
    MAX_BROWSER_WAIT_TIMEOUT_MS,
};

fn deserialize_public_http_headers<'de, D>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let headers = Option::<BTreeMap<String, String>>::deserialize(deserializer)?;
    if let Some(name) = headers.as_ref().and_then(|headers| {
        headers
            .keys()
            .find(|name| !crate::definition::primitives::fetch::http::is_public_http_header(name))
    }) {
        return Err(serde::de::Error::custom(format!(
            "HTTP header {name} is not in the canonical public allowlist"
        )));
    }
    Ok(headers)
}

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
        #[serde(
            default,
            deserialize_with = "deserialize_public_http_headers",
            skip_serializing_if = "Option::is_none"
        )]
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
            deserialize_with = "crate::definition::primitives::fetch::browser::deserialize_browser_fetch_timeout"
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
    ) -> Option<&'static crate::definition::primitives::fetch::browser::BrowserPrimitiveDescriptor>
    {
        match self {
            Self::Http { .. } => None,
            Self::Browser { .. } => {
                Some(&crate::definition::primitives::fetch::browser::BROWSER_FETCH_DESCRIPTOR)
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
    pub fn http_parts(
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
    pub fn label(self) -> &'static str {
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

fn authored_fetch_shape(value: &Fetch) -> (&'static str, &'static [&'static str]) {
    match value {
        Fetch::Http {
            method: _,
            url: _,
            headers: _,
            body: _,
            timeout_ms: _,
        } => ("http", &["url", "headers", "timeoutMs"]),
        Fetch::Browser {
            url: _,
            timeout_ms: _,
            waits: _,
            interactions: _,
        } => ("browser", &["url", "timeoutMs", "waits", "interactions"]),
    }
}

fn authored_method_key(value: HttpMethod) -> &'static str {
    match value {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
    }
}

fn authored_body_shape(value: &RequestBody) -> (&'static str, &'static [&'static str]) {
    match value {
        RequestBody::Json { value: _ } => ("json", &["value"]),
        RequestBody::Text { value: _ } => ("text", &["value"]),
        RequestBody::Form { fields: _ } => ("form", &["fields"]),
    }
}

fn authored_wait_shape(value: &BrowserWait) -> (&'static str, &'static [&'static str]) {
    match value {
        BrowserWait::Selector {
            selector: _,
            timeout_ms: _,
        } => ("selector", &["selector", "timeoutMs"]),
        BrowserWait::NetworkIdle { timeout_ms: _ } => ("network_idle", &["timeoutMs"]),
    }
}

fn authored_interaction_shape(
    value: &BrowserInteraction,
) -> (&'static str, &'static [&'static str]) {
    match value {
        BrowserInteraction::ClickIfVisible {
            selector: _,
            max_count: _,
            wait_after_ms: _,
        } => ("click_if_visible", &["selector", "maxCount", "waitAfterMs"]),
        BrowserInteraction::ClickUntilGone {
            selector: _,
            max_count: _,
            wait_after_ms: _,
        } => ("click_until_gone", &["selector", "maxCount", "waitAfterMs"]),
    }
}

pub fn completeness_serde_shapes() -> Vec<crate::definition::primitives::completeness::SerdeShape> {
    use crate::definition::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::{ParentOption, Tagged},
        Family,
        PrimitiveContext::{Detail, DetectionBrowser, DetectionHttp, Discovery},
    };
    let mut out = Vec::new();
    let authored_fetches = [
        Fetch::Http {
            method: Some(HttpMethod::Post),
            url: String::new(),
            headers: Some(BTreeMap::new()),
            body: Some(RequestBody::Json {
                value: JsonObject::new(),
            }),
            timeout_ms: 1,
        },
        Fetch::Browser {
            url: String::new(),
            timeout_ms: 1,
            waits: Some(Vec::new()),
            interactions: Some(Vec::new()),
        },
    ];
    for authored in &authored_fetches {
        let (key, options) = authored_fetch_shape(authored);
        let (family, contexts) = if key == "http" {
            (Family::Fetch, &[Discovery, Detail, DetectionHttp][..])
        } else {
            (Family::Browser, &[Discovery, Detail, DetectionBrowser][..])
        };
        out.push(serde_shape(
            family,
            key,
            contexts,
            Tagged,
            "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
        ));
        for option in options {
            out.push(serde_shape(
                family,
                format!("{key}.{option}"),
                contexts,
                ParentOption,
                "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
            ));
        }
    }
    for method in [HttpMethod::Get, HttpMethod::Post] {
        out.push(serde_shape(
            Family::Fetch,
            format!("http.method.{}", authored_method_key(method)),
            &[Discovery, Detail, DetectionHttp],
            ParentOption,
            "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
        ));
    }
    let bodies = [
        RequestBody::Json {
            value: JsonObject::new(),
        },
        RequestBody::Text {
            value: String::new(),
        },
        RequestBody::Form {
            fields: BTreeMap::new(),
        },
    ];
    for body in &bodies {
        let (key, options) = authored_body_shape(body);
        out.push(serde_shape(
            Family::Fetch,
            format!("http.body.{key}"),
            &[Discovery, Detail, DetectionHttp],
            ParentOption,
            "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
        ));
        for option in options {
            out.push(serde_shape(
                Family::Fetch,
                format!("http.body.{key}.{option}"),
                &[Discovery, Detail, DetectionHttp],
                ParentOption,
                "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
            ));
        }
    }
    let waits = [
        BrowserWait::Selector {
            selector: String::new(),
            timeout_ms: 1,
        },
        BrowserWait::NetworkIdle { timeout_ms: 1 },
    ];
    for authored in &waits {
        let (key, options) = authored_wait_shape(authored);
        out.push(serde_shape(
            Family::Browser,
            key,
            &[Discovery, Detail, DetectionBrowser],
            Tagged,
            "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
        ));
        for option in options {
            out.push(serde_shape(
                Family::Browser,
                format!("{key}.{option}"),
                &[Discovery, Detail, DetectionBrowser],
                ParentOption,
                "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
            ));
        }
    }
    let interactions = [
        BrowserInteraction::ClickIfVisible {
            selector: String::new(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
        BrowserInteraction::ClickUntilGone {
            selector: String::new(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
    ];
    for authored in &interactions {
        let (key, options) = authored_interaction_shape(authored);
        out.push(serde_shape(
            Family::Browser,
            key,
            &[Discovery, Detail, DetectionBrowser],
            Tagged,
            "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
        ));
        for option in options {
            out.push(serde_shape(
                Family::Browser,
                format!("{key}.{option}"),
                &[Discovery, Detail, DetectionBrowser],
                ParentOption,
                "src-tauri/crates/source-engine/src/definition/documents/fetch.rs",
            ));
        }
    }
    out
}
