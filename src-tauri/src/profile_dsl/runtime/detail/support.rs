use super::*;
use crate::profile_dsl::template::{
    render_template as render_compiled_template, CompiledTemplate, TemplateReference,
    TemplateValueView,
};

pub(super) struct TemplateRuntimeContext<'a> {
    pub(super) source_config: &'a SourceConfig,
    pub(super) source_name: &'a str,
    pub(super) posting: &'a PostingOccurrence,
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
                "url" => Some(self.posting.reference.provider_url.clone()),
                "title" => self.posting.provider_values.title.clone(),
                "company" => self.posting.provider_values.company.clone(),
                "descriptionText" => self.posting.provider_values.description_text.clone(),
                "locations" if !self.posting.provider_values.locations.is_empty() => {
                    Some(self.posting.provider_values.locations.join(", "))
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
