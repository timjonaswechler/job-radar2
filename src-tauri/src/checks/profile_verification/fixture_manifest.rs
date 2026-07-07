use std::collections::BTreeMap;

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::profile_dsl::documents::JsonObject;

pub const FIXTURE_MANIFEST_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureManifest {
    pub schema_version: u64,
    pub profile_key: String,
    pub access_path_key: String,
    pub source_config: JsonObject,
    pub requests: Vec<FixtureManifestRequestMapping>,
    pub checks: FixtureManifestChecks,
}

impl<'de> Deserialize<'de> for FixtureManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct FixtureManifestUnchecked {
            schema_version: u64,
            profile_key: String,
            access_path_key: String,
            source_config: JsonObject,
            requests: Vec<FixtureManifestRequestMapping>,
            checks: FixtureManifestChecks,
        }

        let unchecked = FixtureManifestUnchecked::deserialize(deserializer)?;
        let manifest = FixtureManifest {
            schema_version: unchecked.schema_version,
            profile_key: unchecked.profile_key,
            access_path_key: unchecked.access_path_key,
            source_config: unchecked.source_config,
            requests: unchecked.requests,
            checks: unchecked.checks,
        };

        validate_fixture_manifest_contract(&manifest).map_err(de::Error::custom)?;
        Ok(manifest)
    }
}

fn validate_fixture_manifest_contract(manifest: &FixtureManifest) -> Result<(), String> {
    if manifest.schema_version != FIXTURE_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported Fixture Manifest schemaVersion {}; expected {}",
            manifest.schema_version, FIXTURE_MANIFEST_SCHEMA_VERSION
        ));
    }

    if manifest.requests.is_empty() {
        return Err("Fixture Manifest requests must contain at least one mapping".to_string());
    }

    if manifest.checks.posting_discovery.is_none() && manifest.checks.posting_detail.is_none() {
        return Err(
            "Fixture Manifest checks must include postingDiscovery or postingDetail".to_string(),
        );
    }

    if let Some(posting_detail) = &manifest.checks.posting_detail {
        if posting_detail.cases.is_empty() {
            return Err("Fixture Manifest postingDetail cases must not be empty".to_string());
        }
    }

    validate_technical_key("profileKey", &manifest.profile_key)?;
    validate_technical_key("accessPathKey", &manifest.access_path_key)?;

    for request in &manifest.requests {
        validate_technical_key("request key", &request.key)?;
        validate_absolute_http_url("request match url", &request.request_match.url)?;

        if !(100..=599).contains(&request.response.status) {
            return Err(format!(
                "Fixture Manifest request `{}` response status {} is outside the HTTP status range 100..=599",
                request.key, request.response.status
            ));
        }

        if request.response.body_file.is_empty() {
            return Err(format!(
                "Fixture Manifest request `{}` response bodyFile must not be empty",
                request.key
            ));
        }
    }

    Ok(())
}

fn validate_technical_key(label: &str, value: &str) -> Result<(), String> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(format!("Fixture Manifest {label} must not be empty"));
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(format!(
            "Fixture Manifest {label} `{value}` must start with a lowercase ASCII letter or digit"
        ));
    }

    if chars.any(|ch| !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '_') {
        return Err(format!(
            "Fixture Manifest {label} `{value}` may only contain lowercase ASCII letters, digits, and underscores"
        ));
    }

    Ok(())
}

fn validate_absolute_http_url(label: &str, value: &str) -> Result<(), String> {
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.chars().any(char::is_whitespace)
    {
        return Err(format!(
            "Fixture Manifest {label} `{value}` must be an absolute HTTP(S) URL"
        ));
    }

    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestRequestMapping {
    pub key: String,
    #[serde(rename = "match")]
    pub request_match: FixtureManifestRequestMatch,
    pub response: FixtureManifestResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestRequestMatch {
    pub method: FixtureManifestRequestMethod,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FixtureManifestRequestMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestResponse {
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    pub body_file: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_discovery: Option<FixtureManifestPostingDiscoveryCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<FixtureManifestPostingDetailCheck>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestPostingDiscoveryCheck {
    pub expect: FixtureManifestDiscoveryExpect,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestDiscoveryExpect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_candidates: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_fields: Option<Vec<FixtureManifestPostingField>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_candidates: Option<Vec<FixtureManifestExpectedCandidate>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FixtureManifestPostingField {
    Title,
    Company,
    Url,
    Locations,
    PostingMeta,
    DescriptionText,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestExpectedCandidate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestPostingDetailCheck {
    pub cases: Vec<FixtureManifestPostingDetailCase>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestPostingDetailCase {
    pub key: String,
    pub posting: FixtureManifestPostingInput,
    pub expect: FixtureManifestPostingDetailExpect,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestPostingInput {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<JsonObject>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureManifestPostingDetailExpect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_description_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_contains: Option<Vec<String>>,
}
