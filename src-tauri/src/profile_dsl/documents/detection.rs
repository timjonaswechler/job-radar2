use std::collections::BTreeSet;

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
    #[serde(default, deserialize_with = "deserialize_optional_technical_key")]
    recommended_access_path_key: Option<String>,
    source_config: Option<JsonObject>,
    key_candidates: Option<Vec<String>>,
    name_candidates: Option<Vec<String>>,
    evidence: Option<Vec<DetectionEvidence>>,
}

impl TryFrom<DetectionDocumentWire> for DetectionDocument {
    type Error = &'static str;

    fn try_from(value: DetectionDocumentWire) -> Result<Self, Self::Error> {
        if value.policy != Some(StrategyPolicy::AllRequired) {
            return Err("final Detection requires the all_required policy");
        }
        let Some(strategies) = value.strategies.as_deref() else {
            return Err("final Detection requires strategies");
        };
        if !matches!(strategies.first(), Some(DetectionStrategy::Url { .. })) {
            return Err("final Detection requires a URL-first non-empty strategy set");
        }
        if strategies[1..]
            .iter()
            .any(|strategy| matches!(strategy, DetectionStrategy::Url { .. }))
        {
            return Err("final Detection allows the URL strategy only in first position");
        }
        if strategies.iter().any(|strategy| {
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
        }) {
            return Err("Detection HTTP and Browser captures require regex");
        }
        if strategies.iter().any(|strategy| {
            matches!(
                strategy,
                DetectionStrategy::Browser {
                    contains: None,
                    regex: None,
                    ..
                }
            )
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

fn deserialize_optional_detection_status<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let status = Option::<u16>::deserialize(deserializer)?;
    if status.is_some_and(|status| !(100..=599).contains(&status)) {
        return Err(serde::de::Error::custom(
            "Detection expectStatus must be between 100 and 599",
        ));
    }
    Ok(status)
}

fn deserialize_optional_non_empty_string<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    if value.as_deref().is_some_and(str::is_empty) {
        return Err(serde::de::Error::custom(
            "Detection string options must be non-empty",
        ));
    }
    Ok(value)
}

fn is_technical_key(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn deserialize_optional_technical_key<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    if value
        .as_deref()
        .is_some_and(|value| !is_technical_key(value))
    {
        return Err(serde::de::Error::custom(
            "Detection keys must use the canonical technical-key grammar",
        ));
    }
    Ok(value)
}

fn deserialize_technical_key<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if is_technical_key(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "Detection keys must use the canonical technical-key grammar",
        ))
    }
}

fn is_capture_key(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn deserialize_optional_capture_keys<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let captures = Option::<Vec<String>>::deserialize(deserializer)?;
    if let Some(captures) = &captures {
        if captures.iter().any(|capture| !is_capture_key(capture)) {
            return Err(serde::de::Error::custom(
                "Detection captures must use canonical capture keys",
            ));
        }
        if captures.iter().collect::<BTreeSet<_>>().len() != captures.len() {
            return Err(serde::de::Error::custom(
                "Detection captures must be unique",
            ));
        }
    }
    Ok(captures)
}

fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        Err(serde::de::Error::custom(
            "Detection strings must be non-empty",
        ))
    } else {
        Ok(value)
    }
}

fn deserialize_non_empty_url_patterns<'de, D>(
    deserializer: D,
) -> Result<Vec<InputUrlPattern>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let alternatives = Vec::<InputUrlPattern>::deserialize(deserializer)?;
    if alternatives.is_empty() {
        Err(serde::de::Error::custom(
            "Detection URL pattern alternatives must be non-empty",
        ))
    } else {
        Ok(alternatives)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DetectionStrategy {
    Url {
        #[serde(deserialize_with = "deserialize_technical_key")]
        key: String,
        input: DetectionUrlInput,
    },
    Http {
        #[serde(deserialize_with = "deserialize_technical_key")]
        key: String,
        #[serde(deserialize_with = "deserialize_http_fetch")]
        fetch: Fetch,
        #[serde(
            default,
            rename = "expectStatus",
            deserialize_with = "deserialize_optional_detection_status",
            skip_serializing_if = "Option::is_none"
        )]
        expect_status: Option<u16>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
        contains: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
        regex: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_capture_keys",
            skip_serializing_if = "Option::is_none"
        )]
        captures: Option<Vec<String>>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
        evidence: Option<String>,
    },
    Browser {
        #[serde(deserialize_with = "deserialize_technical_key")]
        key: String,
        #[serde(deserialize_with = "deserialize_browser_fetch")]
        fetch: Fetch,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
        contains: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
        regex: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_capture_keys",
            skip_serializing_if = "Option::is_none"
        )]
        captures: Option<Vec<String>>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_empty_string",
            skip_serializing_if = "Option::is_none"
        )]
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
    PatternAlternatives {
        #[serde(deserialize_with = "deserialize_non_empty_url_patterns")]
        alternatives: Vec<InputUrlPattern>,
    },
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
    #[serde(deserialize_with = "deserialize_non_empty_string")]
    pub pattern: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_capture_keys",
        skip_serializing_if = "Option::is_none"
    )]
    pub captures: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionEvidence {
    pub kind: DetectionEvidenceKind,
    #[serde(deserialize_with = "deserialize_non_empty_string")]
    pub message: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_empty_string",
        skip_serializing_if = "Option::is_none"
    )]
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
