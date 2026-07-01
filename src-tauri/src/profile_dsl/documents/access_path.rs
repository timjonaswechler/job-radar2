use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::posting_detail::PostingDetailStep;
use crate::profile_dsl::documents::posting_discovery::PostingDiscoveryStep;
use crate::profile_dsl::documents::support::{JsonSchemaObject, SupportNote};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReusableAccessPathDocument {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_issues: Option<Vec<SupportNote>>,
    pub posting_discovery: PostingDiscoveryStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<PostingDetailStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}
