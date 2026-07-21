use std::collections::BTreeSet;

use crate::profile_dsl::{
    diagnostics::Diagnostics,
    documents::{DetailStep, DiscoveryStep, FieldExpression, ListFieldExpression, ParseType},
    primitives::value::{
        compile_value_foundation, value_expression_node_count, ValueCompileContext,
        ValueCompileError, ValueCompileErrorKind, ValuePlacement, VALUE_MAX_NODES,
    },
};

use super::{compiler_error, templates::discovery_posting_meta_keys};

pub(super) fn validate_value_context_foundation(
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    source_config_keys: BTreeSet<String>,
    base_path: &str,
    total_nodes: &mut usize,
    diagnostics: &mut Diagnostics,
) {
    let posting_meta_keys = discovery_posting_meta_keys(discovery)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for (index, strategy) in discovery.strategies.iter().enumerate() {
        let strategy_path = format!("{base_path}/discovery/strategies/{index}");
        let capture_keys = strategy
            .captures
            .as_ref()
            .into_iter()
            .flat_map(|captures| captures.keys().cloned())
            .collect::<BTreeSet<_>>();
        let capture_context = context(
            ValuePlacement::DiscoveryCaptureSource,
            Some(strategy.parse.parse_type()),
            &source_config_keys,
            &posting_meta_keys,
            &BTreeSet::new(),
        );
        for (key, rule) in strategy.captures.iter().flatten() {
            validate_expression(
                &rule.from,
                &format!(
                    "{strategy_path}/captures/{}/from",
                    crate::profile_dsl::template::json_pointer_segment(key)
                ),
                &strategy.key,
                &capture_context,
                total_nodes,
                diagnostics,
            );
        }

        let output_context = context(
            ValuePlacement::DiscoveryFilterOutput,
            Some(strategy.parse.parse_type()),
            &source_config_keys,
            &posting_meta_keys,
            &capture_keys,
        );
        validate_filters(
            strategy.conditions.as_ref(),
            &format!("{strategy_path}/where"),
            &strategy.key,
            &output_context,
            total_nodes,
            diagnostics,
        );
        let fields_path = format!("{strategy_path}/extract/fields");
        validate_expression(
            &strategy.extract.fields.title,
            &format!("{fields_path}/title"),
            &strategy.key,
            &output_context,
            total_nodes,
            diagnostics,
        );
        validate_expression(
            &strategy.extract.fields.company,
            &format!("{fields_path}/company"),
            &strategy.key,
            &output_context,
            total_nodes,
            diagnostics,
        );
        validate_expression(
            &strategy.extract.fields.url,
            &format!("{fields_path}/url"),
            &strategy.key,
            &output_context,
            total_nodes,
            diagnostics,
        );
        if let Some(locations) = &strategy.extract.fields.locations {
            validate_list(
                locations,
                &format!("{fields_path}/locations"),
                &strategy.key,
                &output_context,
                total_nodes,
                diagnostics,
            );
        }
        for (key, expression) in strategy.extract.fields.posting_meta.iter().flatten() {
            validate_expression(
                expression,
                &format!(
                    "{fields_path}/postingMeta/{}",
                    crate::profile_dsl::template::json_pointer_segment(key)
                ),
                &strategy.key,
                &output_context,
                total_nodes,
                diagnostics,
            );
        }
        if let Some(expression) = &strategy.extract.fields.description_text {
            validate_expression(
                expression,
                &format!("{fields_path}/descriptionText"),
                &strategy.key,
                &output_context,
                total_nodes,
                diagnostics,
            );
        }
    }

    if let Some(detail) = detail {
        for (index, strategy) in detail.strategies.iter().enumerate() {
            let strategy_path = format!("{base_path}/detail/strategies/{index}");
            let capture_keys = strategy
                .captures
                .as_ref()
                .into_iter()
                .flat_map(|captures| captures.keys().cloned())
                .collect::<BTreeSet<_>>();
            let capture_context = context(
                ValuePlacement::DetailCaptureSource,
                None,
                &source_config_keys,
                &posting_meta_keys,
                &BTreeSet::new(),
            );
            for (key, rule) in strategy.captures.iter().flatten() {
                validate_expression(
                    &rule.from,
                    &format!(
                        "{strategy_path}/captures/{}/from",
                        crate::profile_dsl::template::json_pointer_segment(key)
                    ),
                    &strategy.key,
                    &capture_context,
                    total_nodes,
                    diagnostics,
                );
            }
            let output_context = context(
                ValuePlacement::DetailMatchFilterOutput,
                Some(strategy.parse.parse_type()),
                &source_config_keys,
                &posting_meta_keys,
                &capture_keys,
            );
            validate_filters(
                strategy.conditions.as_ref(),
                &format!("{strategy_path}/where"),
                &strategy.key,
                &output_context,
                total_nodes,
                diagnostics,
            );
            if let Some(field_match) = &strategy.field_match {
                validate_expression(
                    &field_match.left,
                    &format!("{strategy_path}/match/left"),
                    &strategy.key,
                    &output_context,
                    total_nodes,
                    diagnostics,
                );
                validate_expression(
                    &field_match.right,
                    &format!("{strategy_path}/match/right"),
                    &strategy.key,
                    &output_context,
                    total_nodes,
                    diagnostics,
                );
            }
            validate_expression(
                &strategy.extract.fields.description_text,
                &format!("{strategy_path}/extract/fields/descriptionText"),
                &strategy.key,
                &output_context,
                total_nodes,
                diagnostics,
            );
        }
    }
}

fn context(
    placement: ValuePlacement,
    document_type: Option<ParseType>,
    source_config_keys: &BTreeSet<String>,
    posting_meta_keys: &BTreeSet<String>,
    capture_keys: &BTreeSet<String>,
) -> ValueCompileContext {
    ValueCompileContext {
        placement,
        document_type,
        source_config_keys: source_config_keys.clone(),
        posting_meta_keys: posting_meta_keys.clone(),
        capture_keys: capture_keys.clone(),
    }
}

fn validate_filters(
    filters: Option<&Vec<crate::profile_dsl::documents::select::Filter>>,
    path: &str,
    strategy_key: &str,
    context: &ValueCompileContext,
    total_nodes: &mut usize,
    diagnostics: &mut Diagnostics,
) {
    for (index, filter) in filters.into_iter().flatten().enumerate() {
        let field = match filter {
            crate::profile_dsl::documents::select::Filter::NonEmpty { field }
            | crate::profile_dsl::documents::select::Filter::Regex { field, .. } => field,
        };
        validate_expression(
            field,
            &format!("{path}/{index}/field"),
            strategy_key,
            context,
            total_nodes,
            diagnostics,
        );
    }
}

fn validate_list(
    list: &ListFieldExpression,
    path: &str,
    strategy_key: &str,
    context: &ValueCompileContext,
    total_nodes: &mut usize,
    diagnostics: &mut Diagnostics,
) {
    match list {
        ListFieldExpression::Single(expression) => validate_expression(
            expression,
            path,
            strategy_key,
            context,
            total_nodes,
            diagnostics,
        ),
        ListFieldExpression::Multiple(expressions) => {
            for (index, expression) in expressions.iter().enumerate() {
                validate_expression(
                    expression,
                    &format!("{path}/{index}"),
                    strategy_key,
                    context,
                    total_nodes,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_expression(
    expression: &FieldExpression,
    path: &str,
    strategy_key: &str,
    context: &ValueCompileContext,
    total_nodes: &mut usize,
    diagnostics: &mut Diagnostics,
) {
    let nodes = value_expression_node_count(expression);
    let previous_total = *total_nodes;
    *total_nodes = total_nodes.saturating_add(nodes);
    if *total_nodes > VALUE_MAX_NODES {
        if previous_total > VALUE_MAX_NODES {
            return;
        }
        let mut diagnostic = compiler_error(
            "value_node_limit_exceeded",
            "Complete Effective Source behavior exceeds the immutable Value expression node maximum",
            path,
            serde_json::json!({ "actual": *total_nodes, "maximum": VALUE_MAX_NODES }),
        );
        diagnostic.strategy_key = Some(strategy_key.to_string());
        diagnostics.push(diagnostic);
        return;
    }
    if let Err(error) = compile_value_foundation(expression, context) {
        push_error(path, strategy_key, error, diagnostics);
    }
}

fn push_error(
    base_path: &str,
    strategy_key: &str,
    error: ValueCompileError,
    diagnostics: &mut Diagnostics,
) {
    let code = match error.kind {
        ValueCompileErrorKind::UnknownSourceConfigKey => "value_unknown_source_config_key",
        ValueCompileErrorKind::PostingMetaUnavailable => "value_posting_meta_unavailable",
        ValueCompileErrorKind::UnknownPostingMetaKey => "value_unknown_posting_meta_key",
        ValueCompileErrorKind::CaptureUnavailable => "value_capture_unavailable",
        ValueCompileErrorKind::UnknownCaptureKey => "value_unknown_capture_key",
        ValueCompileErrorKind::SelectedItemUnavailable => "value_selected_item_unavailable",
        ValueCompileErrorKind::DocumentIncompatible => "value_document_incompatible",
        ValueCompileErrorKind::Template => "value_template_context_unavailable",
        ValueCompileErrorKind::TemplateTransformPipe => "template_transform_pipes_unsupported",
        ValueCompileErrorKind::DepthLimitExceeded => "value_depth_limit_exceeded",
        ValueCompileErrorKind::NodeLimitExceeded => "value_node_limit_exceeded",
        ValueCompileErrorKind::EmptyCandidates => "value_empty_candidates",
        ValueCompileErrorKind::CandidateLimitExceeded => "value_candidate_limit_exceeded",
        ValueCompileErrorKind::TransformEmptySeparator => "transform_empty_separator",
        ValueCompileErrorKind::TransformInvalidRegex => "transform_invalid_regex",
    };
    let path = if error.path.is_empty() {
        base_path.to_string()
    } else {
        format!("{base_path}{}", error.path)
    };
    let mut details = if matches!(
        error.kind,
        ValueCompileErrorKind::TransformEmptySeparator
            | ValueCompileErrorKind::TransformInvalidRegex
    ) {
        serde_json::json!({
            "transformIndex": error
                .path
                .rsplit('/')
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0)
        })
    } else {
        serde_json::json!({ "placement": context_free_placement_hint(code) })
    };
    if let Some(actual) = error.actual {
        details["actual"] = serde_json::json!(actual);
    }
    if let Some(maximum) = error.maximum {
        details["maximum"] = serde_json::json!(maximum);
    }
    let mut diagnostic = compiler_error(code, error.message, path, details);
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn context_free_placement_hint(code: &str) -> &'static str {
    match code {
        "value_posting_meta_unavailable" => "phase_restricted",
        "value_capture_unavailable" => "capture_source",
        "value_selected_item_unavailable" => "detail_capture_source",
        _ => "value",
    }
}
