use super::validation::{validate_template_string, validate_variable_reference};
use super::*;

pub(super) fn validate_discovery_field_templates(
    discovery: &DiscoveryStep,
    strategy_index: usize,
    strategy_path: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    let strategy = &discovery.strategies[strategy_index];
    let base = format!("{strategy_path}/extract/fields");
    validate_field_expression_templates(
        &strategy.extract.fields.title,
        &format!("{base}/title"),
        strategy.key.as_str(),
        context,
        source_config_keys,
        captures,
        posting_meta_keys,
        diagnostics,
        dependencies,
    );
    validate_field_expression_templates(
        &strategy.extract.fields.company,
        &format!("{base}/company"),
        strategy.key.as_str(),
        context,
        source_config_keys,
        captures,
        posting_meta_keys,
        diagnostics,
        dependencies,
    );
    validate_field_expression_templates(
        &strategy.extract.fields.url,
        &format!("{base}/url"),
        strategy.key.as_str(),
        context,
        source_config_keys,
        captures,
        posting_meta_keys,
        diagnostics,
        dependencies,
    );
    if let Some(locations) = &strategy.extract.fields.locations {
        validate_list_field_expression_templates(
            locations,
            &format!("{base}/locations"),
            strategy.key.as_str(),
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        );
    }
    if let Some(posting_meta) = &strategy.extract.fields.posting_meta {
        for (key, expression) in posting_meta {
            validate_field_expression_templates(
                expression,
                &format!("{base}/postingMeta/{key}"),
                strategy.key.as_str(),
                context,
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
                dependencies,
            );
        }
    }
    if let Some(description_text) = &strategy.extract.fields.description_text {
        validate_field_expression_templates(
            description_text,
            &format!("{base}/descriptionText"),
            strategy.key.as_str(),
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        );
    }
}

fn validate_list_field_expression_templates(
    expression: &ListFieldExpression,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    match expression {
        ListFieldExpression::Single(expression) => validate_field_expression_templates(
            expression,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        ListFieldExpression::Multiple(expressions) => {
            for (index, expression) in expressions.iter().enumerate() {
                validate_field_expression_templates(
                    expression,
                    &format!("{path}/{index}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                    dependencies,
                );
            }
        }
    }
}

pub(super) fn validate_field_expression_templates(
    expression: &FieldExpression,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    match expression {
        FieldExpression::Template { template, .. } => validate_template_string(
            template,
            &format!("{path}/template"),
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        FieldExpression::SourceConfig { key, .. } => validate_variable_reference(
            "sourceConfig",
            key,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        FieldExpression::PostingMeta { key, .. } => validate_variable_reference(
            "postingMeta",
            key,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        FieldExpression::Capture { key, .. } => validate_variable_reference(
            "captures",
            key,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        FieldExpression::Combine { parts, .. } => {
            for (index, part) in parts.iter().enumerate() {
                validate_field_expression_templates(
                    &part.value,
                    &format!("{path}/parts/{index}/value"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                    dependencies,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn discovery_posting_meta_keys(discovery: &DiscoveryStep) -> HashSet<String> {
    discovery
        .strategies
        .iter()
        .filter_map(|strategy| strategy.extract.fields.posting_meta.as_ref())
        .flat_map(|posting_meta| posting_meta.keys().cloned())
        .collect()
}
