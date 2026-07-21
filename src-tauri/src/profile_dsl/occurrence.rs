use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PostingOccurrenceIdentity {
    ProviderPostingId {
        source_key: String,
        provider_posting_id: String,
    },
    NormalizedUrl {
        source_key: String,
        normalized_url: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingReference {
    pub provider_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_posting_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HintUse {
    SearchPrefilter,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryHint {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint_use: Option<HintUse>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingOccurrence {
    pub identity: PostingOccurrenceIdentity,
    pub reference: PostingReference,
    #[serde(default, skip_serializing_if = "ProviderValues::is_empty")]
    pub provider_values: ProviderValues,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hints: BTreeMap<String, DiscoveryHint>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub posting_meta: BTreeMap<String, String>,
}

impl ProviderValues {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.company.is_none()
            && self.locations.is_empty()
            && self.description_text.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OccurrenceReferenceError {
    InvalidUrl,
    UserInfo,
    EmptyProviderPostingId,
    FragmentWithoutProviderPostingId,
}

pub fn validate_posting_reference(
    source_key: &str,
    provider_url: &str,
    provider_posting_id: Option<String>,
) -> Result<(PostingReference, PostingOccurrenceIdentity), OccurrenceReferenceError> {
    let provider_url = provider_url.trim_matches(|character: char| character.is_ascii_whitespace());
    let parsed = Url::parse(provider_url).map_err(|_| OccurrenceReferenceError::InvalidUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(OccurrenceReferenceError::InvalidUrl);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(OccurrenceReferenceError::UserInfo);
    }

    let provider_posting_id = provider_posting_id;
    if provider_posting_id.as_deref() == Some("") {
        return Err(OccurrenceReferenceError::EmptyProviderPostingId);
    }
    if provider_posting_id.is_none() && parsed.fragment().is_some() {
        return Err(OccurrenceReferenceError::FragmentWithoutProviderPostingId);
    }

    let identity = match &provider_posting_id {
        Some(provider_posting_id) => PostingOccurrenceIdentity::ProviderPostingId {
            source_key: source_key.to_string(),
            provider_posting_id: provider_posting_id.clone(),
        },
        None => PostingOccurrenceIdentity::NormalizedUrl {
            source_key: source_key.to_string(),
            normalized_url: parsed.to_string(),
        },
    };
    Ok((
        PostingReference {
            provider_url: provider_url.to_string(),
            provider_posting_id,
        },
        identity,
    ))
}
