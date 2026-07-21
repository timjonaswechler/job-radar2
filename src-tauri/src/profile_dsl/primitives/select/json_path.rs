use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor = super::SelectDescriptor { key: "json_path" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsonPathSelect {
    pub(super) json_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonPathSelectPlan {
    segments: Vec<String>,
}

impl<'de> Deserialize<'de> for JsonPathSelectPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            segments: Vec<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.segments.iter().any(|segment| !valid_segment(segment)) {
            return Err(serde::de::Error::custom(
                "compiled JSONPath contains an invalid segment",
            ));
        }
        Ok(Self {
            segments: wire.segments,
        })
    }
}

pub(crate) fn compile(json_path: &str) -> Result<JsonPathSelectPlan, String> {
    Ok(JsonPathSelectPlan {
        segments: parse_simple_json_path(json_path)?,
    })
}

pub(crate) fn execute<'doc>(
    plan: &JsonPathSelectPlan,
    root: &'doc Value,
) -> SelectedSequence<'doc, 'static> {
    resolve_segments(root, &plan.segments)
        .map(SelectedItem::Json)
        .map(SelectedSequence::one)
        .unwrap_or_default()
}

pub(crate) fn resolve_authored_json_path<'a>(
    root: &'a Value,
    json_path: &str,
) -> Result<Option<&'a Value>, String> {
    let segments = parse_simple_json_path(json_path)?;
    Ok(resolve_segments(root, &segments))
}

fn resolve_segments<'a>(root: &'a Value, segments: &[String]) -> Option<&'a Value> {
    let mut current = root;
    for segment in segments {
        current = current.get(segment)?;
    }
    Some(current)
}

fn parse_simple_json_path(json_path: &str) -> Result<Vec<String>, String> {
    let path = json_path.trim();
    if path == "$" {
        return Ok(Vec::new());
    }
    let Some(rest) = path.strip_prefix("$.") else {
        return Err(error(path));
    };
    let segments = rest.split('.').map(str::to_string).collect::<Vec<_>>();
    if segments.is_empty() || segments.iter().any(|segment| !valid_segment(segment)) {
        return Err(error(path));
    }
    Ok(segments)
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn error(path: &str) -> String {
    format!("`{path}` is not supported; use simple dot JSONPath only, for example $.jobs or $.title (filters and wildcards are not supported)")
}
