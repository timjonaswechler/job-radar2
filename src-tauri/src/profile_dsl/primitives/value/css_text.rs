use crate::profile_dsl::{
    documents::ParseType,
    primitives::{
        cardinality::CompiledCardinality,
        select::{css, CssSelectPlan, SelectedItem},
        transform::{CompiledTransformPipeline, TransformValue},
    },
};

use super::{
    error, eval_error, finish_values, member_path, require_document, CompiledValue,
    CompiledValueResult, ValueCompileContext, ValueCompileError, ValueCompileErrorKind,
    ValueDescriptor, ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "css_text" };

pub(super) fn compile(
    authored: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    require_document(context, path, ParseType::Html, "css")?;
    let selector = css::compile(authored).map_err(|message| {
        error(
            ValueCompileErrorKind::SelectorSyntax,
            &member_path(path, "selector"),
            &message,
        )
    })?;
    Ok(CompiledValue::CssText {
        selector,
        cardinality,
        transforms,
    })
}

pub(super) fn execute<'a, 'doc, 'body>(
    selector: &CssSelectPlan,
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
    let SelectedItem::Html(root) = selected else {
        return Err(eval_error(
            ValueEvaluationErrorKind::TypeMismatch,
            path,
            "compiled CSS Value received an incompatible selected item",
        ));
    };
    let values = css::execute_relative(selector, root)
        .into_vec()
        .into_iter()
        .filter_map(|selected| match selected {
            SelectedItem::Html(node) => {
                Some(TransformValue::Text(node.formatted_text().to_string()))
            }
            _ => None,
        })
        .collect();
    finish_values(values, cardinality, transforms, path)
}
