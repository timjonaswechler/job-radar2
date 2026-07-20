use super::*;

pub(super) fn validate_template_string(
    template: &str,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
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
            dependencies,
        );
    }
}

pub(super) fn validate_variable_reference(
    namespace: &str,
    key: &str,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    let known = match namespace {
        "sourceConfig" => source_config_keys.contains(key),
        "captures" => captures.contains(key),
        "postingMeta" => {
            if matches!(context, TemplateContext::Discovery) {
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
            if matches!(context, TemplateContext::Discovery) {
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
    } else if namespace == "source" && key == "name" {
        dependencies.insert(SourceRuntimeBinding::Name);
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

fn canonical_posting_keys() -> HashSet<&'static str> {
    ["title", "company", "url", "locations", "descriptionText"]
        .into_iter()
        .collect()
}

fn canonical_source_keys() -> HashSet<&'static str> {
    ["name"].into_iter().collect()
}
