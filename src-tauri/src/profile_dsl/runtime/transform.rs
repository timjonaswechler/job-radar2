use dom_query::Document as HtmlDocument;
use regex::Regex;
use serde_json::{json, Value};

use crate::profile_dsl::documents::transform::Transform;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransformPipelineError {
    pub code: &'static str,
    pub message: String,
    pub transform: Value,
}

pub(crate) fn apply_transform_pipeline(
    mut values: Vec<String>,
    transforms: Option<&Vec<Transform>>,
) -> Result<Vec<String>, TransformPipelineError> {
    for transform in transforms.into_iter().flatten() {
        match transform {
            Transform::Trim => {
                values = values
                    .into_iter()
                    .map(|value| value.trim().to_string())
                    .collect();
            }
            Transform::NormalizeWhitespace => {
                values = values
                    .into_iter()
                    .map(|value| normalize_whitespace(&value))
                    .collect();
            }
            Transform::UrlDecode => {
                values = values
                    .into_iter()
                    .map(|value| percent_decode_lossy(&value))
                    .collect();
            }
            Transform::SlugToTitle => {
                values = values
                    .into_iter()
                    .map(|value| slug_to_title(&value))
                    .collect();
            }
            Transform::HtmlToText => {
                values = values
                    .into_iter()
                    .map(|value| html_to_text(&value))
                    .collect();
            }
            Transform::Split {
                separator,
                trim_parts,
                drop_empty,
            } => {
                if separator.is_empty() {
                    return Err(transform_error(
                        "invalid_split_separator",
                        "split transform separator must not be empty",
                        transform,
                    ));
                }
                let should_trim = trim_parts.unwrap_or(false);
                let should_drop_empty = drop_empty.unwrap_or(false);
                values = values
                    .into_iter()
                    .flat_map(|value| {
                        value
                            .split(separator)
                            .map(|part| {
                                if should_trim {
                                    part.trim().to_string()
                                } else {
                                    part.to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .filter(|value| !(should_drop_empty && value.is_empty()))
                    .collect();
            }
            Transform::Join { separator } => {
                values = vec![values.join(separator)];
            }
            Transform::RegexReplace {
                pattern,
                replacement,
            } => {
                let regex = Regex::new(pattern).map_err(|error| {
                    transform_error(
                        "invalid_regex_replace_pattern",
                        format!("regex_replace transform pattern is invalid: {error}"),
                        transform,
                    )
                })?;
                values = values
                    .into_iter()
                    .map(|value| regex.replace_all(&value, replacement.as_str()).to_string())
                    .collect();
            }
            Transform::Dedupe => values = dedupe_preserving_order(values),
            Transform::ToString => {}
        }
    }

    Ok(values)
}

fn transform_error(
    code: &'static str,
    message: impl Into<String>,
    transform: &Transform,
) -> TransformPipelineError {
    TransformPipelineError {
        code,
        message: message.into(),
        transform: serde_json::to_value(transform).unwrap_or_else(|_| json!({})),
    }
}

pub(crate) fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn html_to_text(value: &str) -> String {
    normalize_whitespace(&HtmlDocument::fragment(value).formatted_text().to_string())
}

fn slug_to_title(value: &str) -> String {
    title_case_without_default(&normalize_whitespace(&value.replace(['-', '_'], " ")))
}

fn title_case_without_default(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn percent_decode_lossy(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'+' {
            decoded.push(b' ');
            index += 1;
            continue;
        }
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(&value) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_trim_and_dedupe_are_ordered_array_transforms() {
        let values = apply_transform_pipeline(
            vec![" Berlin ;Remote; Berlin; München ".to_string()],
            Some(&vec![
                Transform::Split {
                    separator: ";".to_string(),
                    trim_parts: None,
                    drop_empty: None,
                },
                Transform::Trim,
                Transform::Dedupe,
            ]),
        )
        .expect("transform pipeline should succeed");

        assert_eq!(values, vec!["Berlin", "Remote", "München"]);
    }

    #[test]
    fn dedupe_preserves_first_seen_order_without_sorting() {
        let values = apply_transform_pipeline(
            vec![
                "Remote".to_string(),
                "Berlin".to_string(),
                "Remote".to_string(),
            ],
            Some(&vec![Transform::Dedupe]),
        )
        .expect("transform pipeline should succeed");

        assert_eq!(values, vec!["Remote", "Berlin"]);
    }
}
