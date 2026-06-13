use serde_json::Value;
use std::fmt;

/// Error returned when a profile asks for JSONPath features outside the MVP.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SimpleJsonPathError {
    json_path: String,
}

impl fmt::Display for SimpleJsonPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not supported; use simple dot JSONPath only, for example $.jobs or $.title (filters and wildcards are not supported)",
            self.json_path
        )
    }
}

/// Resolves `$` or simple `$.a.b.c` object paths without filters or wildcards.
pub(crate) fn resolve_simple_json_path<'a>(
    root: &'a Value,
    json_path: &str,
) -> Result<Option<&'a Value>, SimpleJsonPathError> {
    let segments = parse_simple_json_path(json_path)?;
    let mut current = root;
    for segment in segments {
        let Some(next) = current.get(segment) else {
            return Ok(None);
        };
        current = next;
    }
    Ok(Some(current))
}

pub(crate) fn simple_json_path_exists(root: &Value, json_path: &str) -> bool {
    resolve_simple_json_path(root, json_path)
        .ok()
        .flatten()
        .is_some()
}

fn parse_simple_json_path(json_path: &str) -> Result<Vec<&str>, SimpleJsonPathError> {
    let json_path = json_path.trim();
    if json_path == "$" {
        return Ok(Vec::new());
    }
    let Some(rest) = json_path.strip_prefix("$.") else {
        return Err(simple_json_path_error(json_path));
    };

    let mut segments = Vec::new();
    for segment in rest.split('.') {
        if segment.is_empty()
            || !segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(simple_json_path_error(json_path));
        }
        segments.push(segment);
    }

    Ok(segments)
}

fn simple_json_path_error(json_path: &str) -> SimpleJsonPathError {
    SimpleJsonPathError {
        json_path: json_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_found_value() {
        let value = json!({ "jobs": [{ "title": "Rust Engineer" }] });

        assert_eq!(
            resolve_simple_json_path(&value, "$.jobs").unwrap(),
            Some(&value["jobs"])
        );
    }

    #[test]
    fn returns_none_for_missing_path() {
        let value = json!({ "jobs": [] });

        assert_eq!(resolve_simple_json_path(&value, "$.missing").unwrap(), None);
    }

    #[test]
    fn returns_none_for_wrong_shape_on_dot_path() {
        let value = json!({ "company": "Focused Energy" });

        assert_eq!(
            resolve_simple_json_path(&value, "$.company.name").unwrap(),
            None
        );
    }

    #[test]
    fn resolves_nested_dot_path() {
        let value = json!({ "outer": { "inner": { "title": "Photonics Engineer" } } });

        assert_eq!(
            resolve_simple_json_path(&value, "$.outer.inner.title").unwrap(),
            Some(&value["outer"]["inner"]["title"])
        );
    }

    #[test]
    fn rejects_wildcards_and_filters() {
        let error = resolve_simple_json_path(&json!({ "jobs": [] }), "$.jobs[*]").unwrap_err();

        assert!(error.to_string().contains("simple dot JSONPath only"));
        assert!(error
            .to_string()
            .contains("filters and wildcards are not supported"));
    }
}
