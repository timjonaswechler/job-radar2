use regex::Regex;
use reqwest::Url;
use std::fmt;

/// Resolves variables for the shared declarative template renderer.
///
/// Context implementations own the variable vocabulary (for example detection
/// input URL variables or inventory source/item variables). The renderer owns
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

/// Renders `{{variable|filter}}` placeholders with one shared filter set.
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
    let mut parts = expression.split('|').map(str::trim);
    let variable = parts
        .next()
        .filter(|variable| !variable.is_empty())
        .ok_or_else(|| {
            TemplateError::Invalid("template expression must not be empty".to_string())
        })?;

    let mut value = context
        .resolve_variable(variable)?
        .ok_or_else(|| TemplateError::MissingVariable(variable.to_string()))?;
    for filter in parts {
        if filter.is_empty() {
            return Err(TemplateError::Invalid(
                "template filter must not be empty".to_string(),
            ));
        }
        value = apply_template_filter(filter, &value)?;
    }

    Ok(value)
}

fn apply_template_filter(filter: &str, value: &str) -> Result<String, TemplateError> {
    match filter {
        "technicalKey" => Ok(to_technical_key(value)),
        "titleCase" => Ok(title_case(value)),
        "domainKey" => Ok(to_technical_key(&company_domain_label(value)?)),
        "domainTitle" => Ok(title_case(&company_domain_label(value)?)),
        "urlDecode" => Ok(percent_decode_lossy(value)),
        "slugToTitle" => Ok(slug_to_title(value)),
        _ => Err(TemplateError::Invalid(format!(
            "unsupported template filter `{filter}`"
        ))),
    }
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

fn company_domain_label(value: &str) -> Result<String, TemplateError> {
    let url = parse_http_url(value).map_err(|error| {
        TemplateError::Invalid(format!(
            "template domain filter requires an HTTP(S) URL: {error}"
        ))
    })?;
    let host = normalized_host(&url);
    let label = host
        .split('.')
        .find(|label| !is_generic_host_label(label))
        .or_else(|| host.split('.').next())
        .unwrap_or_default();

    if label.is_empty() {
        Err(TemplateError::Invalid(
            "template domain filter could not derive a domain label".to_string(),
        ))
    } else {
        Ok(label.to_string())
    }
}

fn parse_http_url(input: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string());
    }
    let with_protocol = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let url = Url::parse(&with_protocol)
        .map_err(|_| "Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())
    }
}

fn normalized_host(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

fn is_generic_host_label(label: &str) -> bool {
    matches!(
        label,
        "www"
            | "app"
            | "api"
            | "jobs"
            | "job"
            | "careers"
            | "career"
            | "join"
            | "boards"
            | "job-boards"
    )
}

fn slug_to_title(value: &str) -> String {
    title_case_without_default(&collapse_whitespace(&value.replace(['-', '_'], " ")))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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
    fn shared_filters_cover_detection_and_inventory_templates() {
        let context = MapTemplateContext {
            variables: HashMap::from([
                ("raw", "Héllo GmbH & Co. KG"),
                ("slug", "senior+rust%2Dengineer"),
                ("website", "https://jobs.focused-energy.co/careers"),
                ("sourceName", "Focused Energy"),
            ]),
        };

        assert_eq!(
            render_template("{{raw|technicalKey}}", &context).unwrap(),
            "h_llo_gmbh_co_kg"
        );
        assert_eq!(
            render_template("{{raw|titleCase}}", &context).unwrap(),
            "Héllo GmbH & Co. KG"
        );
        assert_eq!(
            render_template("{{website|domainKey}}", &context).unwrap(),
            "focused_energy"
        );
        assert_eq!(
            render_template("{{website|domainTitle}}", &context).unwrap(),
            "Focused Energy"
        );
        assert_eq!(
            render_template("{{slug|urlDecode|slugToTitle}}", &context).unwrap(),
            "Senior Rust Engineer"
        );
        assert_eq!(
            render_template("{{sourceName}}", &context).unwrap(),
            "Focused Energy"
        );
    }
}
