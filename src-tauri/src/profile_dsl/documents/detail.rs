use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::extract::FieldExpression;
use crate::profile_dsl::documents::fetch::Fetch;
use crate::profile_dsl::documents::parse::Parse;
use crate::profile_dsl::documents::select::{Captures, Filter, Select};
use crate::profile_dsl::documents::strategy::{Acceptance, FieldMatch};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailStep {
    pub strategies: Vec<DetailStrategy>,
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
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Filter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Captures>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub field_match: Option<FieldMatch>,
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
    pub description_text: FieldExpression,
}
