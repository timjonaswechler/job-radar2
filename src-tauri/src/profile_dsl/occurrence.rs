use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
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

impl PostingOccurrenceIdentity {
    pub fn source_key(&self) -> &str {
        match self {
            Self::ProviderPostingId { source_key, .. } | Self::NormalizedUrl { source_key, .. } => {
                source_key
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingReference {
    pub provider_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_posting_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DetailField {
    Title,
    Company,
    Locations,
    DescriptionText,
}

impl DetailField {
    pub const ALL: [Self; 4] = [
        Self::Title,
        Self::Company,
        Self::Locations,
        Self::DescriptionText,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Company => "company",
            Self::Locations => "locations",
            Self::DescriptionText => "descriptionText",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RequestedDetailFields(Vec<DetailField>);

impl RequestedDetailFields {
    pub fn new(fields: impl IntoIterator<Item = DetailField>) -> Result<Self, &'static str> {
        let mut requested = fields.into_iter().collect::<Vec<_>>();
        requested.sort_unstable();
        requested.dedup();
        if requested.is_empty() {
            return Err("requested Detail fields must not be empty");
        }
        Ok(Self(requested))
    }

    pub fn description_text() -> Self {
        Self(vec![DetailField::DescriptionText])
    }

    pub fn contains(&self, field: DetailField) -> bool {
        self.0.binary_search(&field).is_ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = DetailField> + '_ {
        self.0.iter().copied()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DetailFieldCapabilities(Vec<DetailField>);

impl DetailFieldCapabilities {
    pub(crate) fn new(fields: impl IntoIterator<Item = DetailField>) -> Self {
        let mut fields = fields.into_iter().collect::<Vec<_>>();
        fields.sort_unstable();
        fields.dedup();
        Self(fields)
    }

    pub fn contains(&self, field: DetailField) -> bool {
        self.0.binary_search(&field).is_ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = DetailField> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for DetailFieldCapabilities {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::new(Vec::<DetailField>::deserialize(deserializer)?))
    }
}

impl<'de> Deserialize<'de> for RequestedDetailFields {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = Vec::<DetailField>::deserialize(deserializer)?;
        Self::new(fields).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailPatch {
    #[serde(
        default,
        deserialize_with = "deserialize_non_empty_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_non_empty_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub company: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_non_empty_locations",
        skip_serializing_if = "Option::is_none"
    )]
    pub locations: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_non_empty_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub description_text: Option<String>,
}

fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(D::Error::custom("Detail patch values must not be empty"));
    }
    Ok(Some(value))
}

fn deserialize_non_empty_locations<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<String>::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(D::Error::custom("Detail patch locations must not be empty"));
    }
    Ok(Some(value))
}

impl DetailPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.company.is_none()
            && self.locations.is_none()
            && self.description_text.is_none()
    }
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

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "ContributionOriginWire"
)]
pub struct ContributionOrigin {
    pub strategy_key: String,
    pub attempt_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_item_index: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContributionOriginWire {
    strategy_key: String,
    attempt_index: usize,
    provider_item_index: Option<usize>,
}

impl TryFrom<ContributionOriginWire> for ContributionOrigin {
    type Error = &'static str;

    fn try_from(origin: ContributionOriginWire) -> Result<Self, Self::Error> {
        if origin.strategy_key.is_empty() {
            return Err("contribution strategy key must not be empty");
        }
        Ok(Self {
            strategy_key: origin.strategy_key,
            attempt_index: origin.attempt_index,
            provider_item_index: origin.provider_item_index,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DiscoveryResponsibility {
    Url,
    Title,
    Company,
    Locations,
    DescriptionText,
    Hint { key: String },
    PostingMeta { key: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryContributionEvidence {
    pub group_index: usize,
    pub responsibility: DiscoveryResponsibility,
    pub contributors: Vec<ContributionOrigin>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DiscoveryRejection {
    RequiredProviderUrlConflict {
        group_index: usize,
        contributors: Vec<ContributionOrigin>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailContributionEvidence {
    pub field: DetailField,
    pub contributors: Vec<ContributionOrigin>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DetailRejection {
    UnrequestedField {
        field: DetailField,
        contributors: Vec<ContributionOrigin>,
    },
    OccurrenceIdentityMismatch {
        contributors: Vec<ContributionOrigin>,
    },
}

impl ProviderValues {
    pub(crate) fn is_empty(&self) -> bool {
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
