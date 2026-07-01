use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::JsonObject;

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
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<RetryPolicy>,
    },
    Browser {
        url: String,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        waits: Option<Vec<BrowserWait>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestBody {
    Json { value: JsonObject },
    Text { value: String },
    Form { fields: BTreeMap<String, String> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryPolicy {
    #[serde(rename = "maxAttempts", skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserWait {
    Selector {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    NetworkIdle {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserInteraction {
    ClickIfVisible {
        selector: String,
        #[serde(rename = "maxCount", skip_serializing_if = "Option::is_none")]
        max_count: Option<u64>,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        selector: String,
        #[serde(rename = "maxCount", skip_serializing_if = "Option::is_none")]
        max_count: Option<u64>,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
    ExecuteScript {
        script: String,
    },
    Eval {
        expression: String,
    },
    MutateDom {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mutation: Option<String>,
    },
    LoginFlow {
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
    },
    CaptchaBypass {
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
    },
}
