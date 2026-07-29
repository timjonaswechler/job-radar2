use dom_query::Document as HtmlDocument;
use serde::{Deserialize, Serialize};

use super::{
    ParseDescriptor, ParseFailure, ParseInput, ParseInputKind, ParsePlanContext, ParsedDocument,
};

pub const DESCRIPTOR: ParseDescriptor = ParseDescriptor { key: "html" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HtmlParse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) charset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct HtmlParsePlan {
    pub(super) context: ParsePlanContext,
}

pub(super) fn compile(authored: &HtmlParse, input_kind: ParseInputKind) -> HtmlParsePlan {
    HtmlParsePlan {
        context: ParsePlanContext::new(input_kind, authored.charset.as_deref()),
    }
}

pub(super) fn parse<'a>(
    _plan: &HtmlParsePlan,
    input: ParseInput<'a>,
) -> Result<ParsedDocument<'a>, ParseFailure> {
    let text = input.text();
    // Parse external HTML with browser-style HTML5 error recovery. Source markup
    // is passed through unchanged apart from supplying fragment parser context;
    // html5ever's repaired DOM remains usable by downstream selection and
    // acceptance instead of turning ordinary authoring errors into hard failures.
    let has_doctype = text
        .trim_start()
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"));
    let parse_text = if has_doctype {
        std::borrow::Cow::Borrowed(text)
    } else {
        std::borrow::Cow::Owned(format!("<!doctype html>{text}"))
    };
    Ok(ParsedDocument::Html(HtmlDocument::from(
        parse_text.as_ref(),
    )))
}
