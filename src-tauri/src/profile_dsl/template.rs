use regex::Regex;
use std::fmt;

/// Resolves variables for the shared declarative template renderer.
///
/// Context implementations own the variable vocabulary (for example detection
/// input URL variables or runtime source/item variables). The renderer owns
/// placeholder parsing and the shared filter pipeline.
pub(crate) trait TemplateContext {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TemplateError {
    MissingVariable(String),
    Invalid(String),
}

impl TemplateError {
    pub(crate) fn missing_variable(&self) -> Option<&str> {
        match self {
            Self::MissingVariable(variable) => Some(variable.as_str()),
            Self::Invalid(_) => None,
        }
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVariable(variable) => {
                write!(formatter, "template variable `{variable}` is not available")
            }
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

/// Renders `{{variable}}` placeholders.
///
/// Transform pipes are intentionally rejected by the shared renderer because
/// Profile DSL transforms must be declared explicitly in `transforms[]`.
pub(crate) fn render_template(
    template: &str,
    context: &dyn TemplateContext,
) -> Result<String, TemplateError> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered =
        placeholder_regex
            .replace_all(template, |placeholder: &regex::Captures<'_>| {
                match render_template_expression(&placeholder[1], context) {
                    Ok(value) => value,
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                        String::new()
                    }
                }
            })
            .to_string();

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(rendered)
    }
}

fn render_template_expression(
    expression: &str,
    context: &dyn TemplateContext,
) -> Result<String, TemplateError> {
    if expression.contains('|') {
        return Err(TemplateError::Invalid(
            "template transform pipes are not supported; transforms must be declared in transforms[]"
                .to_string(),
        ));
    }

    let variable = expression.trim();
    if variable.is_empty() {
        return Err(TemplateError::Invalid(
            "template expression must not be empty".to_string(),
        ));
    }

    context
        .resolve_variable(variable)?
        .ok_or_else(|| TemplateError::MissingVariable(variable.to_string()))
}

pub(crate) fn to_technical_key(value: &str) -> String {
    let mut key = String::new();
    let mut last_was_separator = false;
    for ch in value.to_lowercase().chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'ä' => Some('a'),
            'ö' => Some('o'),
            'ü' => Some('u'),
            'ß' => {
                key.push_str("ss");
                last_was_separator = false;
                None
            }
            _ => None,
        };

        if let Some(ch) = mapped {
            key.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !key.is_empty() {
            key.push('_');
            last_was_separator = true;
        }
    }

    let key = key.trim_matches('_').to_string();
    if key.is_empty() {
        "quelle".to_string()
    } else {
        key
    }
}

pub(crate) fn title_case(value: &str) -> String {
    let title = title_case_without_default(&value.replace(['-', '_'], " "));
    if title.is_empty() {
        "Neue Quelle".to_string()
    } else {
        title
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapTemplateContext {
        variables: HashMap<&'static str, &'static str>,
    }

    impl TemplateContext for MapTemplateContext {
        fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
            if variable == "unsupported" {
                return Err(TemplateError::Invalid(
                    "unsupported template variable `unsupported`".to_string(),
                ));
            }
            Ok(self
                .variables
                .get(variable)
                .map(|value| (*value).to_string()))
        }
    }

    #[test]
    fn renders_variables_through_public_template_context_interface() {
        let context = MapTemplateContext {
            variables: HashMap::from([("company", "Focused Energy"), ("role", "Rust Engineer")]),
        };

        let rendered = render_template("{{company}} sucht {{ role }}", &context).unwrap();

        assert_eq!(rendered, "Focused Energy sucht Rust Engineer");
    }

    #[test]
    fn reports_missing_variables_without_context_internals() {
        let context = MapTemplateContext {
            variables: HashMap::new(),
        };

        let error = render_template("{{missing}}", &context).unwrap_err();

        assert_eq!(error, TemplateError::MissingVariable("missing".to_string()));
        assert_eq!(error.missing_variable(), Some("missing"));
    }

    #[test]
    fn rejects_transform_pipes_in_template_expressions() {
        let context = MapTemplateContext {
            variables: HashMap::from([("raw", "Héllo GmbH & Co. KG")]),
        };

        let error = render_template("{{raw|technicalKey}}", &context).unwrap_err();

        assert_eq!(
            error,
            TemplateError::Invalid(
                "template transform pipes are not supported; transforms must be declared in transforms[]"
                    .to_string()
            )
        );
    }

    #[test]
    fn default_candidate_helpers_are_explicit_non_template_behaviour() {
        assert_eq!(to_technical_key("Héllo GmbH & Co. KG"), "h_llo_gmbh_co_kg");
        assert_eq!(title_case("acme_corp"), "Acme Corp");
    }
}
