use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    Fetch, FieldExpression, JsonObject, ListFieldExpression, PostingDetailStep,
    PostingDiscoveryStep, RequestBody,
};

use super::compiler_error;

#[derive(Clone, Copy)]
enum TemplateContext {
    PostingDiscovery,
    PostingDetail,
}

pub(super) fn validate_template_variables(
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    source_config_keys: HashSet<String>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    let posting_meta_keys = posting_discovery_posting_meta_keys(posting_discovery);
    for (index, strategy) in posting_discovery.strategies.iter().enumerate() {
        let captures = strategy
            .captures
            .as_ref()
            .map(|captures| captures.keys().cloned().collect::<HashSet<_>>())
            .unwrap_or_default();
        let strategy_path = format!("{base_path}/postingDiscovery/strategies/{index}");
        validate_fetch_templates(
            &strategy.fetch,
            &format!("{strategy_path}/fetch"),
            strategy.key.as_str(),
            TemplateContext::PostingDiscovery,
            &source_config_keys,
            &captures,
            &posting_meta_keys,
            diagnostics,
        );
        validate_discovery_field_templates(
            posting_discovery,
            index,
            &strategy_path,
            TemplateContext::PostingDiscovery,
            &source_config_keys,
            &captures,
            &posting_meta_keys,
            diagnostics,
        );
    }

    if let Some(posting_detail) = posting_detail {
        for (index, strategy) in posting_detail.strategies.iter().enumerate() {
            let captures = strategy
                .captures
                .as_ref()
                .map(|captures| captures.keys().cloned().collect::<HashSet<_>>())
                .unwrap_or_default();
            let strategy_path = format!("{base_path}/postingDetail/strategies/{index}");
            validate_fetch_templates(
                &strategy.fetch,
                &format!("{strategy_path}/fetch"),
                strategy.key.as_str(),
                TemplateContext::PostingDetail,
                &source_config_keys,
                &captures,
                &posting_meta_keys,
                diagnostics,
            );
            validate_field_expression_templates(
                &strategy.extract.fields.description_text,
                &format!("{strategy_path}/extract/fields/descriptionText"),
                strategy.key.as_str(),
                TemplateContext::PostingDetail,
                &source_config_keys,
                &captures,
                &posting_meta_keys,
                diagnostics,
            );
            if let Some(field_match) = &strategy.field_match {
                validate_field_expression_templates(
                    &field_match.left,
                    &format!("{strategy_path}/match/left"),
                    strategy.key.as_str(),
                    TemplateContext::PostingDetail,
                    &source_config_keys,
                    &captures,
                    &posting_meta_keys,
                    diagnostics,
                );
                validate_field_expression_templates(
                    &field_match.right,
                    &format!("{strategy_path}/match/right"),
                    strategy.key.as_str(),
                    TemplateContext::PostingDetail,
                    &source_config_keys,
                    &captures,
                    &posting_meta_keys,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_discovery_field_templates(
    posting_discovery: &PostingDiscoveryStep,
    strategy_index: usize,
    strategy_path: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    let strategy = &posting_discovery.strategies[strategy_index];
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
        );
    }
}

fn validate_fetch_templates(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match fetch {
        Fetch::Http {
            url, headers, body, ..
        } => {
            validate_template_string(
                url,
                &format!("{path}/url"),
                strategy_key,
                context,
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
            );
            if let Some(headers) = headers {
                for (header, value) in headers {
                    validate_template_string(
                        value,
                        &format!("{path}/headers/{header}"),
                        strategy_key,
                        context,
                        source_config_keys,
                        captures,
                        posting_meta_keys,
                        diagnostics,
                    );
                }
            }
            if let Some(body) = body {
                validate_request_body_templates(
                    body,
                    &format!("{path}/body"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        Fetch::Browser { url, .. } => {
            validate_template_string(
                url,
                &format!("{path}/url"),
                strategy_key,
                context,
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
            );
        }
    }
}

fn validate_request_body_templates(
    body: &RequestBody,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match body {
        RequestBody::Json { value } => validate_json_object_templates(
            value,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        RequestBody::Text { value } => validate_template_string(
            value,
            &format!("{path}/value"),
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        RequestBody::Form { fields } => {
            for (key, value) in fields {
                validate_template_string(
                    value,
                    &format!("{path}/fields/{key}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_json_object_templates(
    value: &JsonObject,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    for (key, value) in value {
        validate_json_value_templates(
            value,
            &format!("{path}/{key}"),
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        );
    }
}

fn validate_json_value_templates(
    value: &serde_json::Value,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    match value {
        serde_json::Value::String(value) => validate_template_string(
            value,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        ),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_json_value_templates(
                    value,
                    &format!("{path}/{index}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_json_value_templates(
                    value,
                    &format!("{path}/{key}"),
                    strategy_key,
                    context,
                    source_config_keys,
                    captures,
                    posting_meta_keys,
                    diagnostics,
                );
            }
        }
        _ => {}
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
                );
            }
        }
    }
}

fn validate_field_expression_templates(
    expression: &FieldExpression,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
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
                );
            }
        }
        _ => {}
    }
}

fn validate_template_string(
    template: &str,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    for reference in template_references(template) {
        if reference.contains('|') {
            let mut diagnostic = compiler_error(
                "template_transform_pipes_unsupported",
                format!(
                    "Template reference `{{{{{reference}}}}}` uses pipe syntax; transforms must be declared in transforms[]"
                ),
                path,
                serde_json::json!({ "reference": reference }),
            );
            diagnostic.strategy_key = Some(strategy_key.to_string());
            diagnostics.push(diagnostic);
            continue;
        }

        let Some((namespace, key)) = split_template_reference(&reference) else {
            let mut diagnostic = compiler_error(
                "invalid_template_reference",
                format!("Template reference `{{{{{reference}}}}}` must use namespace:key syntax"),
                path,
                serde_json::json!({ "reference": reference }),
            );
            diagnostic.strategy_key = Some(strategy_key.to_string());
            diagnostics.push(diagnostic);
            continue;
        };
        validate_variable_reference(
            namespace,
            key,
            path,
            strategy_key,
            context,
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
        );
    }
}

fn validate_variable_reference(
    namespace: &str,
    key: &str,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
) {
    let known = match namespace {
        "sourceConfig" => source_config_keys.contains(key),
        "captures" => captures.contains(key),
        "postingMeta" => {
            if matches!(context, TemplateContext::PostingDiscovery) {
                push_template_diagnostic(
                    diagnostics,
                    "template_namespace_unavailable",
                    format!(
                        "Template namespace `{namespace}` is not available in postingDiscovery"
                    ),
                    path,
                    strategy_key,
                    namespace,
                    key,
                );
                return;
            }
            posting_meta_keys.contains(key)
        }
        "posting" => {
            if matches!(context, TemplateContext::PostingDiscovery) {
                push_template_diagnostic(diagnostics, "template_namespace_unavailable", format!("Template namespace `{namespace}` is not available before a posting occurrence exists"), path, strategy_key, namespace, key);
                return;
            }
            canonical_posting_keys().contains(key)
        }
        "source" => canonical_source_keys().contains(key),
        _ => {
            push_template_diagnostic(
                diagnostics,
                "invalid_template_namespace",
                format!("Template namespace `{namespace}` is not supported"),
                path,
                strategy_key,
                namespace,
                key,
            );
            return;
        }
    };
    if !known {
        push_template_diagnostic(
            diagnostics,
            "unknown_template_key",
            format!("Template reference `{namespace}:{key}` does not match a declared key"),
            path,
            strategy_key,
            namespace,
            key,
        );
    }
}

fn push_template_diagnostic(
    diagnostics: &mut Diagnostics,
    code: &str,
    message: String,
    path: &str,
    strategy_key: &str,
    namespace: &str,
    key: &str,
) {
    let mut diagnostic = compiler_error(
        code,
        message,
        path,
        serde_json::json!({
            "namespace": namespace,
            "key": key,
        }),
    );
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn template_references(template: &str) -> Vec<String> {
    let mut references = Vec::new();
    let mut remainder = template;
    while let Some(start) = remainder.find("{{") {
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        references.push(after_start[..end].trim().to_string());
        remainder = &after_start[end + 2..];
    }
    references
}

fn split_template_reference(reference: &str) -> Option<(&str, &str)> {
    reference
        .split_once(':')
        .or_else(|| reference.split_once('.'))
        .filter(|(namespace, key)| !namespace.is_empty() && !key.is_empty())
}

fn posting_discovery_posting_meta_keys(
    posting_discovery: &PostingDiscoveryStep,
) -> HashSet<String> {
    posting_discovery
        .strategies
        .iter()
        .filter_map(|strategy| strategy.extract.fields.posting_meta.as_ref())
        .flat_map(|posting_meta| posting_meta.keys().cloned())
        .collect()
}

fn canonical_posting_keys() -> HashSet<&'static str> {
    ["title", "company", "url", "locations", "descriptionText"]
        .into_iter()
        .collect()
}

fn canonical_source_keys() -> HashSet<&'static str> {
    ["name"].into_iter().collect()
}
