use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::extract::FieldExpression;
pub use crate::profile_dsl::primitives::select::Select;

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
