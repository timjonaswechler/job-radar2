use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transform {
    Trim,
    #[serde(alias = "normalizeWhitespace")]
    NormalizeWhitespace,
    #[serde(alias = "htmlToText")]
    HtmlToText,
    #[serde(alias = "urlDecode")]
    UrlDecode,
    #[serde(alias = "slugToTitle")]
    SlugToTitle,
    Dedupe,
    #[serde(alias = "toString")]
    ToString,
    Split {
        separator: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        trim_parts: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        drop_empty: Option<bool>,
    },
    Join {
        separator: String,
    },
    RegexReplace {
        pattern: String,
        replacement: String,
    },
}
