use std::collections::{BTreeMap, BTreeSet};

use dom_query::Document as HtmlDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};

mod html;
mod json;
mod xml;

pub use html::{HtmlParse, HtmlParsePlan};
pub use json::{JsonParse, JsonParsePlan};
pub use xml::{XmlParse, XmlParsePlan};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Parse {
    Json(JsonParse),
    Xml(XmlParse),
    Html(HtmlParse),
}

impl Parse {
    pub const fn parse_type(&self) -> ParseType {
        match self {
            Self::Json(_) => ParseType::Json,
            Self::Xml(_) => ParseType::Xml,
            Self::Html(_) => ParseType::Html,
        }
    }

    pub fn charset(&self) -> Option<&str> {
        match self {
            Self::Json(authored) => authored.charset.as_deref(),
            Self::Xml(authored) => authored.charset.as_deref(),
            Self::Html(authored) => authored.charset.as_deref(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseType {
    Json,
    Xml,
    Html,
}

impl ParseType {
    pub const ALL: [Self; 3] = [Self::Json, Self::Xml, Self::Html];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Html => "html",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseDescriptor {
    pub key: &'static str,
}

const PARSE_DESCRIPTORS: [ParseDescriptor; 3] =
    [json::DESCRIPTOR, xml::DESCRIPTOR, html::DESCRIPTOR];

pub fn parse_descriptors() -> &'static [ParseDescriptor] {
    &PARSE_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseRegistryError {
    Duplicate {
        layer: &'static str,
        keys: Vec<String>,
    },
    Missing {
        layer: &'static str,
        keys: Vec<String>,
    },
    Extra {
        layer: &'static str,
        keys: Vec<String>,
    },
}

pub fn validate_parse_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), ParseRegistryError> {
    for (layer, keys) in [
        ("schema", schema_keys),
        ("serde", serde_keys),
        ("registration", registration_keys),
    ] {
        let duplicates = duplicate_keys(keys);
        if !duplicates.is_empty() {
            return Err(ParseRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }

    let schema = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [("serde", serde_keys), ("registration", registration_keys)] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = schema.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ParseRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&schema).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(ParseRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

fn duplicate_keys(keys: &[String]) -> Vec<String> {
    let mut counts = BTreeMap::new();
    for key in keys {
        *counts.entry(key.clone()).or_insert(0_usize) += 1;
    }
    counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect()
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseInputKind {
    DecodedHttp,
    BrowserRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedHttpText<'a>(&'a str);

impl<'a> DecodedHttpText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrowserRenderedText<'a>(&'a str);

impl<'a> BrowserRenderedText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseInput<'a> {
    DecodedHttp(DecodedHttpText<'a>),
    BrowserRendered(BrowserRenderedText<'a>),
}

impl<'a> ParseInput<'a> {
    pub fn kind(self) -> ParseInputKind {
        match self {
            Self::DecodedHttp(_) => ParseInputKind::DecodedHttp,
            Self::BrowserRendered(_) => ParseInputKind::BrowserRendered,
        }
    }

    pub fn text(self) -> &'a str {
        match self {
            Self::DecodedHttp(text) => text.as_str(),
            Self::BrowserRendered(text) => text.as_str(),
        }
    }
}

pub(crate) enum CompleteParseText {
    DecodedHttp(String),
    BrowserRendered(String),
}

impl CompleteParseText {
    pub(crate) fn as_input(&self) -> ParseInput<'_> {
        match self {
            Self::DecodedHttp(text) => ParseInput::DecodedHttp(DecodedHttpText::new(text)),
            Self::BrowserRendered(text) => {
                ParseInput::BrowserRendered(BrowserRenderedText::new(text))
            }
        }
    }
}

pub enum ParsedDocument<'a> {
    Json(Value),
    Xml(roxmltree::Document<'a>),
    Html(HtmlDocument),
}

impl<'a> ParsedDocument<'a> {
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            Self::Json(document) => Some(document),
            _ => None,
        }
    }

    pub fn as_xml(&self) -> Option<&roxmltree::Document<'a>> {
        match self {
            Self::Xml(document) => Some(document),
            _ => None,
        }
    }

    pub fn as_html(&self) -> Option<&HtmlDocument> {
        match self {
            Self::Html(document) => Some(document),
            _ => None,
        }
    }
}

impl std::fmt::Debug for ParsedDocument<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ParsedDocument")
            .field(&match self {
                Self::Json(_) => "json",
                Self::Xml(_) => "xml",
                Self::Html(_) => "html",
            })
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailureKind {
    InputKindMismatch,
    MalformedJson,
    MalformedXml,
    MalformedHtml,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseFailure {
    pub kind: ParseFailureKind,
    pub input_kind: ParseInputKind,
    pub message: String,
}

impl ParseFailure {
    pub(super) fn malformed(
        kind: ParseFailureKind,
        input_kind: ParseInputKind,
        message: impl AsRef<str>,
    ) -> Self {
        Self {
            kind,
            input_kind,
            message: bounded_message(message.as_ref()),
        }
    }
}

fn bounded_message(message: &str) -> String {
    const MAX_BYTES: usize = 512;
    if message.len() <= MAX_BYTES {
        return message.to_string();
    }
    let mut end = MAX_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_string()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParsePlanContext {
    input_kind: ParseInputKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    charset: Option<String>,
}

impl ParsePlanContext {
    fn new(input_kind: ParseInputKind, charset: Option<&str>) -> Self {
        Self {
            input_kind,
            charset: charset.map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledParse {
    Json(JsonParsePlan),
    Xml(XmlParsePlan),
    Html(HtmlParsePlan),
}

impl CompiledParse {
    pub fn descriptor(&self) -> ParseDescriptor {
        match self {
            Self::Json(_) => json::DESCRIPTOR,
            Self::Xml(_) => xml::DESCRIPTOR,
            Self::Html(_) => html::DESCRIPTOR,
        }
    }

    pub fn input_kind(&self) -> ParseInputKind {
        match self {
            Self::Json(plan) => plan.context.input_kind,
            Self::Xml(plan) => plan.context.input_kind,
            Self::Html(plan) => plan.context.input_kind,
        }
    }

    pub fn authored_charset(&self) -> Option<&str> {
        match self {
            Self::Json(plan) => plan.context.charset.as_deref(),
            Self::Xml(plan) => plan.context.charset.as_deref(),
            Self::Html(plan) => plan.context.charset.as_deref(),
        }
    }

    pub fn parse<'a>(&self, input: ParseInput<'a>) -> Result<ParsedDocument<'a>, ParseFailure> {
        if input.kind() != self.input_kind() {
            return Err(ParseFailure {
                kind: ParseFailureKind::InputKindMismatch,
                input_kind: input.kind(),
                message: "parse input kind does not match the immutable compiled plan".to_string(),
            });
        }
        match self {
            Self::Json(plan) => json::parse(plan, input),
            Self::Xml(plan) => xml::parse(plan, input),
            Self::Html(plan) => html::parse(plan, input),
        }
    }

    pub(crate) fn parse_with_diagnostics<'a>(
        &self,
        input: ParseInput<'a>,
        context: ParseDiagnosticContext<'_>,
        diagnostics: &mut Diagnostics,
    ) -> Option<ParsedDocument<'a>> {
        match self.parse(input) {
            Ok(document) => Some(document),
            Err(failure) => {
                let (code, label) = match failure.kind {
                    ParseFailureKind::MalformedJson => ("json_parse_failed", "JSON"),
                    ParseFailureKind::MalformedXml => ("xml_parse_failed", "XML"),
                    ParseFailureKind::MalformedHtml => ("html_parse_failed", "HTML"),
                    ParseFailureKind::InputKindMismatch => {
                        ("parse_input_kind_mismatch", "the configured format")
                    }
                };
                diagnostics.push(Diagnostic {
                    category: DiagnosticCategory::Runtime,
                    code: code.to_string(),
                    message: format!(
                        "Complete input could not be parsed as {label}: {}",
                        failure.message
                    ),
                    severity: DiagnosticSeverity::Error,
                    path: format!("{}/parse", context.base_path),
                    strategy_key: context.strategy_key.map(ToString::to_string),
                    details: Some(serde_json::json!({
                        "parseType": self.descriptor().key,
                        "inputKind": failure.input_kind,
                        "error": failure.message,
                    })),
                });
                None
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ParseDiagnosticContext<'a> {
    pub(crate) base_path: &'a str,
    pub(crate) strategy_key: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileParseError {
    pub message: String,
}

pub fn compile_parse(
    authored: &Parse,
    input_kind: ParseInputKind,
) -> Result<CompiledParse, CompileParseError> {
    if input_kind == ParseInputKind::BrowserRendered && authored.charset().is_some() {
        return Err(CompileParseError {
            message: "charset is valid only for strictly decoded HTTP parse input".to_string(),
        });
    }

    Ok(match authored {
        Parse::Json(authored) => CompiledParse::Json(json::compile(authored, input_kind)),
        Parse::Xml(authored) => CompiledParse::Xml(xml::compile(authored, input_kind)),
        Parse::Html(authored) => CompiledParse::Html(html::compile(authored, input_kind)),
    })
}
