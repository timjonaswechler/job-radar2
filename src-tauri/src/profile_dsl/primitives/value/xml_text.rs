use crate::profile_dsl::{
    documents::ParseType,
    primitives::{
        cardinality::CompiledCardinality,
        select::{xml_text, SelectedItem, XmlTextSelectPlan},
        transform::{CompiledTransformPipeline, TransformValue},
    },
};

use super::{
    error, eval_error, finish_values, member_path, require_document, CompiledValue,
    CompiledValueResult, ValueCompileContext, ValueCompileError, ValueCompileErrorKind,
    ValueDescriptor, ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "xml_text" };

pub(super) fn compile(
    authored: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    require_document(context, path, ParseType::Xml, "xml")?;
    let selector = xml_text::compile(authored).map_err(|message| {
        error(
            ValueCompileErrorKind::SelectorSyntax,
            &member_path(path, "textPath"),
            &message,
        )
    })?;
    Ok(CompiledValue::XmlText {
        selector,
        cardinality,
        transforms,
    })
}

pub(super) fn execute<'a, 'doc, 'body>(
    selector: &XmlTextSelectPlan,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let selected = context.selected().ok_or_else(|| {
        eval_error(
            ValueEvaluationErrorKind::TypeMismatch,
            path,
            "selected item is unavailable",
        )
    })?;
    let SelectedItem::Xml(root) = selected else {
        return Err(eval_error(
            ValueEvaluationErrorKind::TypeMismatch,
            path,
            "compiled XML Value received an incompatible selected item",
        ));
    };
    let values = xml_text::execute(selector, *root)
        .into_vec()
        .into_iter()
        .map(TransformValue::from)
        .collect();
    finish_values(values, cardinality, transforms, path)
}
