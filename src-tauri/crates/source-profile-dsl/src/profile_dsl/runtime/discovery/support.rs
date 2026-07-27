use super::*;
use crate::profile_dsl::template::{
    render_template, CompiledTemplate, TemplateReference, TemplateValueView,
};

pub(super) struct DiscoveryTemplateValues<'a> {
    pub(super) source_config: &'a SourceConfig,
    pub(super) source_name: &'a str,
}
impl TemplateValueView for DiscoveryTemplateValues<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            Some("sourceConfig") => self
                .source_config
                .get(&reference.key)
                .and_then(json_scalar_as_string),
            Some("source") if reference.key == "name" => Some(self.source_name.to_string()),
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
