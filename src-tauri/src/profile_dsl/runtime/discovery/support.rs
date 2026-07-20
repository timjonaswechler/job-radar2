use super::*;

pub(super) fn render_source_config_template(
    template: &str,
    source_config: &SourceConfig,
    source_name: &str,
) -> Result<String, String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered = placeholder_regex
        .replace_all(template, |captures: &regex::Captures<'_>| {
            let variable = captures[1].trim();
            match render_template_variable(variable, source_config, source_name) {
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

fn render_template_variable(
    variable: &str,
    source_config: &SourceConfig,
    source_name: &str,
) -> Result<String, String> {
    let Some((namespace, key)) = split_template_variable(variable) else {
        return Err(format!(
            "template variable `{variable}` must use namespace:key syntax"
        ));
    };

    match namespace {
        "sourceConfig" => source_config_value_as_string(source_config, key)
            .ok_or_else(|| format!("sourceConfig `{key}` is missing or not scalar")),
        "source" if key == "name" => Ok(source_name.to_string()),
        "source" => Err(format!("source `{key}` is missing or not scalar")),
        _ => Err(format!("unsupported template namespace `{namespace}`")),
    }
}

fn split_template_variable(variable: &str) -> Option<(&str, &str)> {
    variable
        .split_once(':')
        .or_else(|| variable.split_once('.'))
        .filter(|(namespace, key)| !namespace.is_empty() && !key.is_empty())
}

fn source_config_value_as_string(source_config: &SourceConfig, key: &str) -> Option<String> {
    match source_config.get(key)? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn push_browser_fetch_diagnostic(
    error: ProfileBrowserFetchError,
    rendered_url: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) {
    if error.kind == ProfileBrowserFetchErrorKind::Cancelled {
        push_runtime_execution_cancelled(diagnostics, format!("{base_path}/fetch"), strategy_key);
        return;
    }

    let (code, path) = match error.kind {
        ProfileBrowserFetchErrorKind::Cancelled => unreachable!("handled above"),
        ProfileBrowserFetchErrorKind::RuntimeUnavailable => {
            ("browser_runtime_unavailable", format!("{base_path}/fetch"))
        }
        ProfileBrowserFetchErrorKind::NavigationFailed => (
            "browser_navigation_failed",
            format!("{base_path}/fetch/url"),
        ),
        ProfileBrowserFetchErrorKind::WaitTimeout { wait_index } => (
            "browser_wait_timeout",
            wait_index
                .map(|index| format!("{base_path}/fetch/waits/{index}"))
                .unwrap_or_else(|| format!("{base_path}/fetch/waits")),
        ),
        ProfileBrowserFetchErrorKind::InteractionFailed { interaction_index } => (
            "browser_interaction_failed",
            interaction_index
                .map(|index| format!("{base_path}/fetch/interactions/{index}"))
                .unwrap_or_else(|| format!("{base_path}/fetch/interactions")),
        ),
        ProfileBrowserFetchErrorKind::RenderTimeout => (
            "browser_render_timeout",
            format!("{base_path}/fetch/timeoutMs"),
        ),
        ProfileBrowserFetchErrorKind::ContentReadFailed => {
            ("browser_content_read_failed", format!("{base_path}/fetch"))
        }
    };

    diagnostics.push(runtime_error(
        code,
        format!("Browser fetch failed for {rendered_url}: {}", error.message),
        path,
        strategy_key,
        json!({ "url": rendered_url, "error": error.message }),
    ));
}
