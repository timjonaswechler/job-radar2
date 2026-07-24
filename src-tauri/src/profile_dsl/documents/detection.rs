use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::{Fetch, JsonObject};
use crate::profile_dsl::policy::StrategyPolicy;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "DetectionDocumentWire"
)]
pub struct DetectionDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<StrategyPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategies: Option<Vec<DetectionStrategy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_access_path_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config: Option<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_candidates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_candidates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Vec<DetectionEvidence>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetectionDocumentWire {
    policy: Option<StrategyPolicy>,
    strategies: Option<Vec<DetectionStrategy>>,
    recommended_access_path_key: Option<String>,
    source_config: Option<JsonObject>,
    key_candidates: Option<Vec<String>>,
    name_candidates: Option<Vec<String>>,
    evidence: Option<Vec<DetectionEvidence>>,
}

impl TryFrom<DetectionDocumentWire> for DetectionDocument {
    type Error = &'static str;

    fn try_from(value: DetectionDocumentWire) -> Result<Self, Self::Error> {
        let final_route_present = value.policy.is_some() || value.strategies.is_some();
        if final_route_present && (value.policy.is_none() || value.strategies.is_none()) {
            return Err("final Detection requires both policy and strategies");
        }
        if value.strategies.as_deref().is_some_and(|strategies| {
            strategies.iter().any(|strategy| {
                matches!(
                    strategy,
                    DetectionStrategy::Http {
                        captures: Some(_),
                        regex: None,
                        ..
                    } | DetectionStrategy::Browser {
                        captures: Some(_),
                        regex: None,
                        ..
                    }
                )
            })
        }) {
            return Err("Detection HTTP and Browser captures require regex");
        }
        if value.strategies.as_deref().is_some_and(|strategies| {
            strategies.iter().any(|strategy| {
                matches!(
                    strategy,
                    DetectionStrategy::Browser {
                        contains: None,
                        regex: None,
                        ..
                    }
                )
            })
        }) {
            return Err("Detection Browser requires contains or regex");
        }
        Ok(Self {
            policy: value.policy,
            strategies: value.strategies,
            recommended_access_path_key: value.recommended_access_path_key,
            source_config: value.source_config,
            key_candidates: value.key_candidates,
            name_candidates: value.name_candidates,
            evidence: value.evidence,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DetectionStrategy {
    Url {
        key: String,
        input: DetectionUrlInput,
    },
    Http {
        key: String,
        #[serde(deserialize_with = "deserialize_http_fetch")]
        fetch: Fetch,
        #[serde(rename = "expectStatus", skip_serializing_if = "Option::is_none")]
        expect_status: Option<u16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        regex: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        captures: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evidence: Option<String>,
    },
    Browser {
        key: String,
        #[serde(deserialize_with = "deserialize_browser_fetch")]
        fetch: Fetch,
        #[serde(skip_serializing_if = "Option::is_none")]
        contains: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        regex: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        captures: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evidence: Option<String>,
    },
}

fn deserialize_http_fetch<'de, D>(deserializer: D) -> Result<Fetch, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let fetch = Fetch::deserialize(deserializer)?;
    if matches!(fetch, Fetch::Http { .. }) {
        Ok(fetch)
    } else {
        Err(serde::de::Error::custom(
            "Detection HTTP requires an HTTP Fetch",
        ))
    }
}

fn deserialize_browser_fetch<'de, D>(deserializer: D) -> Result<Fetch, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let fetch = Fetch::deserialize(deserializer)?;
    if matches!(fetch, Fetch::Browser { .. }) {
        Ok(fetch)
    } else {
        Err(serde::de::Error::custom(
            "Detection Browser requires a Browser Fetch",
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DetectionStrategyKind {
    Url,
    Http,
    Browser,
}

impl DetectionStrategy {
    pub fn key(&self) -> &str {
        match self {
            Self::Url { key, .. } | Self::Http { key, .. } | Self::Browser { key, .. } => key,
        }
    }

    pub const fn kind(&self) -> DetectionStrategyKind {
        match self {
            Self::Url { .. } => DetectionStrategyKind::Url,
            Self::Http { .. } => DetectionStrategyKind::Http,
            Self::Browser { .. } => DetectionStrategyKind::Browser,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DetectionUrlInput {
    PatternAlternatives { alternatives: Vec<InputUrlPattern> },
    AbsoluteUrl,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DetectionUrlInputKind {
    PatternAlternatives,
    AbsoluteUrl,
}

impl DetectionUrlInput {
    pub const fn kind(&self) -> DetectionUrlInputKind {
        match self {
            Self::PatternAlternatives { .. } => DetectionUrlInputKind::PatternAlternatives,
            Self::AbsoluteUrl => DetectionUrlInputKind::AbsoluteUrl,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputUrlPattern {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionEvidence {
    pub kind: DetectionEvidenceKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionEvidenceKind {
    Url,
    Http,
    Html,
    Browser,
}
