use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::extract::{FieldExpression, ListFieldExpression};
use crate::profile_dsl::documents::fetch::Fetch;
use crate::profile_dsl::documents::pagination::Pagination;
use crate::profile_dsl::documents::parse::Parse;
use crate::profile_dsl::documents::select::{Captures, Filter, Select};
use crate::profile_dsl::documents::strategy::Acceptance;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingDiscoveryStep {
    pub strategies: Vec<PostingDiscoveryStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingDiscoveryStrategy {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fetch: Fetch,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
    pub parse: Parse,
    pub select: Select,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Filter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Captures>,
    pub extract: PostingDiscoveryExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingDiscoveryExtraction {
    pub fields: PostingDiscoveryFields,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostingDiscoveryFields {
    pub title: FieldExpression,
    pub company: FieldExpression,
    pub url: FieldExpression,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<ListFieldExpression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_meta: Option<BTreeMap<String, FieldExpression>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<FieldExpression>,
}
