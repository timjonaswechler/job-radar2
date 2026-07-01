use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::documents::transform::Transform;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    One,
    First,
    Optional,
    All,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldExpression {
    Const {
        value: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Template {
        template: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    SourceConfig {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    PostingMeta {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Capture {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    ItemField {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    JsonPath {
        #[serde(rename = "jsonPath")]
        json_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    XmlText {
        #[serde(rename = "textPath")]
        text_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    XmlElement {
        element: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    CssText {
        selector: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    CssAttribute {
        selector: String,
        attribute: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Combine {
        parts: Vec<CombinePart>,
        #[serde(skip_serializing_if = "Option::is_none")]
        join: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cardinality: Option<Cardinality>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ListFieldExpression {
    Single(FieldExpression),
    Multiple(Vec<FieldExpression>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombinePart {
    pub value: Box<FieldExpression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}
