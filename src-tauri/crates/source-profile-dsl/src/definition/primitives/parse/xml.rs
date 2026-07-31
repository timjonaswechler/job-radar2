use serde::{Deserialize, Serialize};

use super::{
    ParseDescriptor, ParseFailure, ParseFailureKind, ParseInput, ParseInputKind, ParsePlanContext,
    ParsedDocument,
};

pub const DESCRIPTOR: ParseDescriptor = ParseDescriptor { key: "xml" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XmlParse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) charset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct XmlParsePlan {
    pub(super) context: ParsePlanContext,
}

pub(super) fn compile(authored: &XmlParse, input_kind: ParseInputKind) -> XmlParsePlan {
    XmlParsePlan {
        context: ParsePlanContext::new(input_kind, authored.charset.as_deref()),
    }
}

pub(super) fn parse<'a>(
    _plan: &XmlParsePlan,
    input: ParseInput<'a>,
) -> Result<ParsedDocument<'a>, ParseFailure> {
    roxmltree::Document::parse(input.text())
        .map(ParsedDocument::Xml)
        .map_err(|error| {
            ParseFailure::malformed(
                ParseFailureKind::MalformedXml,
                input.kind(),
                error.to_string(),
            )
        })
}
