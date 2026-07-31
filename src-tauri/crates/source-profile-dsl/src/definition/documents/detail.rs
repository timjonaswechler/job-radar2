use serde::{Deserialize, Deserializer, Serialize};

use crate::definition::diagnostics::Diagnostics;
use crate::definition::documents::extract::{FieldExpression, ListFieldExpression};
use crate::definition::documents::fetch::Fetch;
use crate::definition::documents::limits::PhaseLimits;
use crate::definition::documents::parse::Parse;
use crate::definition::documents::select::{Captures, Select};
use crate::definition::documents::strategy::Acceptance;
use crate::definition::policy::StrategyPolicy;
use crate::definition::primitives::predicate::{
    deserialize_detail_match, deserialize_where_predicates, Predicate,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailStep {
    pub policy: StrategyPolicy,
    pub strategies: Vec<DetailStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<PhaseLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailStrategy {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fetch: Fetch,
    pub parse: Parse,
    pub select: Select,
    #[serde(
        rename = "where",
        default,
        deserialize_with = "deserialize_where_predicates",
        skip_serializing_if = "Option::is_none"
    )]
    pub conditions: Option<Vec<Predicate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Captures>,
    #[serde(
        rename = "match",
        default,
        deserialize_with = "deserialize_detail_match",
        skip_serializing_if = "Option::is_none"
    )]
    pub field_match: Option<Predicate>,
    pub extract: DetailExtraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_when: Option<Acceptance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailExtraction {
    pub fields: DetailFields,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailFields {
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<FieldExpression>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub company: Option<FieldExpression>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub locations: Option<ListFieldExpression>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub description_text: Option<FieldExpression>,
}

fn non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
