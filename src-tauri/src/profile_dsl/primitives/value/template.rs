use crate::profile_dsl::{
    primitives::{cardinality::CompiledCardinality, transform::CompiledTransformPipeline},
    template::{
        compile_template_all, render_template, CompiledTemplate, TemplateCompileErrorKind,
        TemplateDescriptor, TemplateReference, TemplateValueView,
    },
};

use super::{
    error, finish_values, json_scalar_string, member_path, CompiledValue, CompiledValueResult,
    ValueCompileContext, ValueCompileError, ValueCompileErrorKind, ValueDescriptor,
    ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind,
};
use crate::profile_dsl::primitives::transform::TransformValue;

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "template" };

pub(super) fn compile(
    authored: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    let template = compile_template_all(authored, &descriptor(context)).map_err(|errors| {
        let transform_pipe = errors
            .iter()
            .any(|error| error.kind == TemplateCompileErrorKind::TransformPipeUnsupported);
        error(
            if transform_pipe {
                ValueCompileErrorKind::TemplateTransformPipe
            } else {
                ValueCompileErrorKind::Template
            },
            &member_path(path, "template"),
            if transform_pipe {
                "Template transform pipes are unsupported; use transforms[]"
            } else {
                "Value template references unavailable context"
            },
        )
    })?;
    Ok(CompiledValue::Template {
        template,
        cardinality,
        transforms,
    })
}

pub(super) fn references_source_name(template: &CompiledTemplate) -> bool {
    template.references(Some("source"), "name")
}

pub(super) fn execute<'a, 'doc, 'body>(
    template: &CompiledTemplate,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let rendered = render_template(template, context).map_err(|_| {
        super::eval_error(
            ValueEvaluationErrorKind::Template,
            path,
            "compiled Template runtime context is missing a declared value",
        )
    })?;
    finish_values(
        vec![TransformValue::Text(rendered)],
        cardinality,
        transforms,
        path,
    )
}

fn descriptor(context: &ValueCompileContext) -> TemplateDescriptor {
    let mut descriptor = TemplateDescriptor::new()
        .allow_namespace("sourceConfig", context.source_config_keys.iter().cloned())
        .allow_namespace("source", ["name"]);
    if context.placement.admits_posting() {
        descriptor = descriptor
            .allow_namespace("postingMeta", context.posting_meta_keys.iter().cloned())
            .allow_namespace(
                "posting",
                ["title", "company", "url", "locations", "descriptionText"],
            );
    }
    if context.placement.admits_captures() {
        descriptor = descriptor.allow_namespace("captures", context.capture_keys.iter().cloned());
    }
    descriptor
}

impl TemplateValueView for ValueEvaluationContext<'_, '_, '_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            Some("source") if reference.key == "name" => {
                Some(self.source().source_name.to_string())
            }
            Some("sourceConfig") => self
                .source()
                .source_config
                .get(&reference.key)
                .and_then(json_scalar_string),
            Some("postingMeta") => self.posting()?.posting_meta.get(&reference.key).cloned(),
            Some("captures") => self.captures()?.get(&reference.key).cloned(),
            Some("posting") => {
                let posting = self.posting()?;
                match reference.key.as_str() {
                    "title" => Some(posting.title.to_string()),
                    "company" => Some(posting.company.to_string()),
                    "url" => Some(posting.url.to_string()),
                    "locations" => Some(posting.locations.join(", ")),
                    "descriptionText" => posting.description_text.map(str::to_string),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
