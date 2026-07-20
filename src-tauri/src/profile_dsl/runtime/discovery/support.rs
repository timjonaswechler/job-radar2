use super::*;
use crate::profile_dsl::template::{
    render_template, CompiledTemplate, TemplateReference, TemplateValueView,
};

struct DiscoveryTemplateValues<'a> {
    source_config: &'a SourceConfig,
    source_name: &'a str,
    captures: Option<&'a BTreeMap<String, String>>,
}
impl TemplateValueView for DiscoveryTemplateValues<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            Some("sourceConfig") => self
                .source_config
                .get(&reference.key)
                .and_then(json_scalar_as_string),
            Some("source") if reference.key == "name" => Some(self.source_name.to_string()),
            Some("captures") => self
                .captures
                .and_then(|captures| captures.get(&reference.key))
                .cloned(),
            _ => None,
        }
    }
}
pub(super) fn render_source_config_template(
    template: &CompiledTemplate,
    source_config: &SourceConfig,
    source_name: &str,
) -> Result<String, String> {
    render_template(
        template,
        &DiscoveryTemplateValues {
            source_config,
            source_name,
            captures: None,
        },
    )
    .map_err(|error| error.to_string())
}
pub(super) fn render_template_with_captures(
    template: &CompiledTemplate,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
) -> Result<String, String> {
    render_template(
        template,
        &DiscoveryTemplateValues {
            source_config,
            source_name,
            captures: Some(captures),
        },
    )
    .map_err(|error| error.to_string())
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn push_browser_fetch_diagnostic(
    error: ProfileBrowserFetchError,
    _rendered_url: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) {
    let (code, path) = match error.kind {
        ProfileBrowserFetchErrorKind::Cancelled => {
            unreachable!("cancellation is typed control flow")
        }
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
        format!("Browser fetch failed: {}", error.message),
        path,
        strategy_key,
        json!({ "error": error.message }),
    ));
}
