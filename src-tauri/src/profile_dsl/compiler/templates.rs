use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep, Fetch};
use crate::profile_dsl::primitives::fetch::http::compile_http_fetch;
use crate::profile_dsl::template::{
    descriptor_for_placement, TemplateAdmissionKeys, TemplatePlacement,
};

use super::{compiler_error, SourceRuntimeBinding, SourceRuntimeBindingDependencies};

#[derive(Clone, Copy)]
enum TemplateContext {
    Discovery,
    Detail,
}

fn browser_url_placement(context: TemplateContext) -> TemplatePlacement {
    match context {
        TemplateContext::Discovery => TemplatePlacement::DiscoveryBrowserUrl,
        TemplateContext::Detail => TemplatePlacement::DetailBrowserUrl,
    }
}

fn http_placements(
    context: TemplateContext,
) -> (TemplatePlacement, TemplatePlacement, TemplatePlacement) {
    match context {
        TemplateContext::Discovery => (
            TemplatePlacement::DiscoveryHttpUrl,
            TemplatePlacement::DiscoveryHttpHeader,
            TemplatePlacement::DiscoveryHttpBody,
        ),
        TemplateContext::Detail => (
            TemplatePlacement::DetailHttpUrl,
            TemplatePlacement::DetailHttpHeader,
            TemplatePlacement::DetailHttpBody,
        ),
    }
}

pub(super) fn validate_template_variables(
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    source_config_keys: HashSet<String>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) -> SourceRuntimeBindingDependencies {
    let mut dependencies = SourceRuntimeBindingDependencies::default();
    let posting_meta_keys = discovery_posting_meta_keys(discovery);
    for (index, strategy) in discovery.strategies.iter().enumerate() {
        validate_fetch_templates(
            &strategy.fetch,
            &format!("{base_path}/discovery/strategies/{index}/fetch"),
            strategy.key.as_str(),
            TemplateContext::Discovery,
            &source_config_keys,
            &HashSet::new(),
            &posting_meta_keys,
            diagnostics,
            &mut dependencies,
        );
    }
    if let Some(detail) = detail {
        for (index, strategy) in detail.strategies.iter().enumerate() {
            let captures = strategy
                .captures
                .as_ref()
                .map(|captures| captures.keys().cloned().collect::<HashSet<_>>())
                .unwrap_or_default();
            validate_fetch_templates(
                &strategy.fetch,
                &format!("{base_path}/detail/strategies/{index}/fetch"),
                strategy.key.as_str(),
                TemplateContext::Detail,
                &source_config_keys,
                &captures,
                &posting_meta_keys,
                diagnostics,
                &mut dependencies,
            );
        }
    }
    dependencies
}

#[allow(clippy::too_many_arguments)]
fn validate_fetch_templates(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    match fetch {
        Fetch::Http {
            method,
            url,
            headers,
            body,
            timeout_ms,
        } => {
            let keys = TemplateAdmissionKeys {
                source_config: source_config_keys.iter().cloned().collect(),
                captures: captures.iter().cloned().collect(),
                posting_meta: posting_meta_keys.iter().cloned().collect(),
            };
            let (url_placement, header_placement, body_placement) = http_placements(context);
            let result = compile_http_fetch(
                *method,
                url,
                headers.as_ref(),
                body.as_ref(),
                *timeout_ms,
                &descriptor_for_placement(url_placement, &keys),
                &descriptor_for_placement(header_placement, &keys),
                &descriptor_for_placement(body_placement, &keys),
            );
            match result {
                Ok(compiled) => {
                    if compiled.references_source_name() {
                        dependencies.insert(SourceRuntimeBinding::Name);
                    }
                }
                Err(error) => {
                    let mut diagnostic = compiler_error(
                        error.code,
                        error.message,
                        format!("{path}{}", error.path),
                        serde_json::json!({ "invariant": "canonical_http_fetch" }),
                    );
                    diagnostic.strategy_key = Some(strategy_key.to_string());
                    diagnostics.push(diagnostic);
                }
            }
        }
        Fetch::Browser { url, .. } => validate_template_string(
            url,
            &format!("{path}/url"),
            strategy_key,
            context,
            browser_url_placement(context),
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
    }
}

pub(super) fn discovery_posting_meta_keys(discovery: &DiscoveryStep) -> HashSet<String> {
    discovery
        .strategies
        .iter()
        .filter_map(|strategy| strategy.extract.posting_meta.as_ref())
        .flat_map(|posting_meta| posting_meta.keys().cloned())
        .collect()
}

fn validate_template_string(
    template: &str,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    placement: TemplatePlacement,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    use crate::profile_dsl::template::{
        compile_template_all, TemplateCompileErrorKind, TemplateSegment,
    };
    let descriptor = descriptor_for_placement(
        placement,
        &TemplateAdmissionKeys {
            source_config: source_config_keys.iter().cloned().collect(),
            captures: captures.iter().cloned().collect(),
            posting_meta: posting_meta_keys.iter().cloned().collect(),
        },
    );
    match compile_template_all(template, &descriptor) {
        Ok(compiled) => {
            if compiled.0.iter().any(|segment| matches!(segment, TemplateSegment::Reference { reference } if reference.namespace.as_deref() == Some("source") && reference.key == "name")) {
                dependencies.insert(SourceRuntimeBinding::Name);
            }
        }
        Err(errors) => for error in errors {
            let code = match error.kind {
                TemplateCompileErrorKind::TransformPipeUnsupported => "template_transform_pipes_unsupported",
                TemplateCompileErrorKind::UnknownNamespace
                    if error.reference.as_ref().and_then(|reference| reference.namespace.as_deref())
                        .is_some_and(|namespace| {
                            (matches!(namespace, "posting" | "postingMeta") && matches!(context, TemplateContext::Discovery))
                                || (namespace == "captures" && matches!(placement,
                                    TemplatePlacement::DiscoveryHttpUrl | TemplatePlacement::DiscoveryHttpHeader | TemplatePlacement::DiscoveryHttpBody | TemplatePlacement::DiscoveryBrowserUrl |
                                    TemplatePlacement::DetailHttpUrl | TemplatePlacement::DetailHttpHeader | TemplatePlacement::DetailHttpBody | TemplatePlacement::DetailBrowserUrl))
                        }) => "template_namespace_unavailable",
                TemplateCompileErrorKind::UnknownNamespace => "invalid_template_namespace",
                TemplateCompileErrorKind::UnknownKey => "unknown_template_key",
                _ => "invalid_template_reference",
            };
            let mut diagnostic = compiler_error(code, format!("Template is invalid: {error}"), path, serde_json::json!({ "kind": format!("{:?}", error.kind), "offset": error.offset }));
            diagnostic.strategy_key = Some(strategy_key.to_string());
            diagnostics.push(diagnostic);
        }
    }
}

fn canonical_posting_keys() -> HashSet<&'static str> {
    ["title", "company", "url", "locations", "descriptionText"]
        .into_iter()
        .collect()
}

fn canonical_source_keys() -> HashSet<&'static str> {
    ["name"].into_iter().collect()
}
