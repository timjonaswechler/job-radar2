use serde::{Deserialize, Serialize};

use super::{
    ParseDescriptor, ParseFailure, ParseFailureKind, ParseInput, ParseInputKind, ParsePlanContext,
    ParsedDocument,
};

pub const DESCRIPTOR: ParseDescriptor = ParseDescriptor { key: "json" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsonParse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) charset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct JsonParsePlan {
    pub(super) context: ParsePlanContext,
}

pub(super) fn compile(authored: &JsonParse, input_kind: ParseInputKind) -> JsonParsePlan {
    JsonParsePlan {
        context: ParsePlanContext::new(input_kind, authored.charset.as_deref()),
    }
}

pub(super) fn parse<'a>(
    _plan: &JsonParsePlan,
    input: ParseInput<'a>,
) -> Result<ParsedDocument<'a>, ParseFailure> {
    serde_json::from_str(input.text())
        .map(ParsedDocument::Json)
        .map_err(|error| {
            ParseFailure::malformed(
                ParseFailureKind::MalformedJson,
                input.kind(),
                error.to_string(),
            )
        })
}
