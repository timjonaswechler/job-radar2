use super::*;
use crate::profile_dsl::template::{
    render_template as render_compiled_template, CompiledTemplate, TemplateReference,
    TemplateValueView,
};

pub(super) struct TemplateRuntimeContext<'a> {
    pub(super) source_config: &'a SourceConfig,
    pub(super) source_name: &'a str,
    pub(super) posting: &'a DetailPostingOccurrence,
    pub(super) posting_meta: &'a BTreeMap<String, String>,
    pub(super) captures: &'a BTreeMap<String, String>,
}
impl TemplateValueView for TemplateRuntimeContext<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            Some("sourceConfig") => self.source_config.get(&reference.key).and_then(json_scalar),
            Some("captures") => self.captures.get(&reference.key).cloned(),
            Some("postingMeta") => self.posting_meta.get(&reference.key).cloned(),
            Some("posting") => match reference.key.as_str() {
                "url" => Some(self.posting.url.clone()),
                "title" => self.posting.title.clone(),
                "company" => self.posting.company.clone(),
                "descriptionText" => self.posting.description_text.clone(),
                "locations" if !self.posting.locations.is_empty() => {
                    Some(self.posting.locations.join(", "))
                }
                _ => None,
            },
            Some("source") if reference.key == "name" => Some(self.source_name.to_string()),
            _ => None,
        }
    }
}
pub(super) fn render_template(
    template: &CompiledTemplate,
    context: &TemplateRuntimeContext<'_>,
) -> Result<String, String> {
    render_compiled_template(template, context).map_err(|error| error.to_string())
}
fn json_scalar(value: &Value) -> Option<String> {
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
