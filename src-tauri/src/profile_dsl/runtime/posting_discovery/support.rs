use super::*;

pub(super) fn render_source_config_template(
    template: &str,
    source_config: &SourceConfig,
) -> Result<String, String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered = placeholder_regex
        .replace_all(template, |captures: &regex::Captures<'_>| {
            let variable = captures[1].trim();
            match render_source_config_variable(variable, source_config) {
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

fn render_source_config_variable(
    variable: &str,
    source_config: &SourceConfig,
) -> Result<String, String> {
    let Some(key) = variable.strip_prefix("sourceConfig:") else {
        return Err(format!("unsupported template variable `{variable}`"));
    };
    let value = source_config
        .get(key)
        .ok_or_else(|| format!("sourceConfig `{key}` is missing"))?;
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Err(format!("sourceConfig `{key}` is null")),
        Value::Array(_) | Value::Object(_) => Err(format!(
            "sourceConfig `{key}` must be a string, number, or boolean for template rendering"
        )),
    }
}

pub(super) fn push_browser_fetch_diagnostic(
    error: ProfileBrowserFetchError,
    rendered_url: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) {
    let (code, path) = match error.kind {
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
