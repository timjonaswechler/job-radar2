use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::extract::FieldExpression;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Select {
    #[serde(rename = "document")]
    Document,
    JsonPath {
        #[serde(rename = "jsonPath")]
        json_path: String,
    },
    XmlElement {
        element: String,
    },
    XmlText {
        #[serde(rename = "textPath")]
        text_path: String,
    },
    #[serde(rename = "css")]
    Css {
        selector: String,
    },
    SitemapUrls {
        #[serde(rename = "urlPattern", skip_serializing_if = "Option::is_none")]
        url_pattern: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Filter {
    NonEmpty {
        field: FieldExpression,
    },
    Regex {
        field: FieldExpression,
        pattern: String,
    },
}

pub type Captures = BTreeMap<String, CaptureRule>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRule {
    pub from: FieldExpression,
    pub pattern: String,
}
