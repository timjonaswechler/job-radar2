use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep, Fetch, JsonObject, RequestBody};
use crate::profile_dsl::template::{
    descriptor_for_placement, json_pointer_segment, TemplateAdmissionKeys, TemplatePlacement,
};

use super::{compiler_error, SourceRuntimeBinding, SourceRuntimeBindingDependencies};

#[derive(Clone, Copy)]
enum TemplateContext {
    Discovery,
    Detail,
}

fn http_url_placement(context: TemplateContext) -> TemplatePlacement {
    match context {
        TemplateContext::Discovery => TemplatePlacement::DiscoveryHttpUrl,
        TemplateContext::Detail => TemplatePlacement::DetailHttpUrl,
    }
}
fn http_header_placement(context: TemplateContext) -> TemplatePlacement {
    match context {
        TemplateContext::Discovery => TemplatePlacement::DiscoveryHttpHeader,
        TemplateContext::Detail => TemplatePlacement::DetailHttpHeader,
    }
}
fn http_body_placement(context: TemplateContext) -> TemplatePlacement {
    match context {
        TemplateContext::Discovery => TemplatePlacement::DiscoveryHttpBody,
        TemplateContext::Detail => TemplatePlacement::DetailHttpBody,
    }
}
fn browser_url_placement(context: TemplateContext) -> TemplatePlacement {
    match context {
        TemplateContext::Discovery => TemplatePlacement::DiscoveryBrowserUrl,
        TemplateContext::Detail => TemplatePlacement::DetailBrowserUrl,
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
        let strategy_path = format!("{base_path}/discovery/strategies/{index}");
        validate_fetch_templates(
            &strategy.fetch,
            &format!("{strategy_path}/fetch"),
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
            let strategy_path = format!("{base_path}/detail/strategies/{index}");
            validate_fetch_templates(
                &strategy.fetch,
                &format!("{strategy_path}/fetch"),
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
            url, headers, body, ..
        } => {
            validate_template_string(
                url,
                &format!("{path}/url"),
                strategy_key,
                context,
                http_url_placement(context),
                source_config_keys,
                captures,
                posting_meta_keys,
                diagnostics,
                dependencies,
            );
            if let Some(headers) = headers {
                for (header, value) in headers {
                    validate_template_string(
                        value,
                        &format!("{path}/headers/{}", json_pointer_segment(header)),
                        strategy_key,
                        context,
                        http_header_placement(context),
                        source_config_keys,
                        captures,
                        posting_meta_keys,
                        diagnostics,
                        dependencies,
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
                    dependencies,
                );
            }
        }
        Fetch::Browser { url, .. } => {
            validate_template_string(
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
    dependencies: &mut SourceRuntimeBindingDependencies,
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
            dependencies,
        ),
        RequestBody::Text { value } => validate_template_string(
            value,
            &format!("{path}/value"),
            strategy_key,
            context,
            http_body_placement(context),
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
        ),
        RequestBody::Form { fields } => {
            for (key, value) in fields {
                validate_template_string(
                    value,
                    &format!("{path}/fields/{}", json_pointer_segment(key)),
                    strategy_key,
                    context,
                    http_body_placement(context),
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

fn validate_json_object_templates(
    value: &JsonObject,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    for (key, value) in value {
        validate_json_value_templates(
            value,
            &format!("{path}/{}", json_pointer_segment(key)),
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

fn validate_json_value_templates(
    value: &serde_json::Value,
    path: &str,
    strategy_key: &str,
    context: TemplateContext,
    source_config_keys: &HashSet<String>,
    captures: &HashSet<String>,
    posting_meta_keys: &HashSet<String>,
    diagnostics: &mut Diagnostics,
    dependencies: &mut SourceRuntimeBindingDependencies,
) {
    match value {
        serde_json::Value::String(value) => validate_template_string(
            value,
            path,
            strategy_key,
            context,
            http_body_placement(context),
            source_config_keys,
            captures,
            posting_meta_keys,
            diagnostics,
            dependencies,
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
                    dependencies,
                );
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_json_value_templates(
                    value,
                    &format!("{path}/{}", json_pointer_segment(key)),
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

pub(super) fn validate_detection_templates(
    profile: &crate::source_profile::documents::SourceProfileDocument,
    diagnostics: &mut Diagnostics,
) {
    use crate::profile_dsl::template::{compile_template, TemplateDescriptor};
    let Some(detection) = &profile.detection else {
        return;
    };
    let mut pattern_captures = detection
        .input_url_patterns
        .as_ref()
        .into_iter()
        .flatten()
        .map(|pattern| {
            let mut captures = pattern
                .captures
                .as_ref()
                .into_iter()
                .flatten()
                .cloned()
                .collect::<HashSet<_>>();
            extend_regex_captures(&mut captures, &pattern.pattern);
            captures
        });
    let mut captures = pattern_captures.next().unwrap_or_default();
    for available in pattern_captures {
        captures.retain(|capture| available.contains(capture));
    }
    let mut source_config_keys = profile
        .source_config_schema
        .iter()
        .chain(
            profile
                .access_paths
                .iter()
                .filter_map(|path| path.source_config_schema.as_ref()),
        )
        .filter_map(|schema| {
            schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
        })
        .flat_map(|properties| properties.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    source_config_keys.extend(
        detection
            .source_config
            .as_ref()
            .into_iter()
            .flat_map(|values| values.keys().cloned()),
    );
    let keys_for = |captures: &HashSet<String>| TemplateAdmissionKeys {
        source_config: source_config_keys.clone(),
        captures: captures.iter().cloned().collect(),
        posting_meta: Default::default(),
    };
    let mut validate = |value: &str,
                        path: String,
                        descriptor: &TemplateDescriptor,
                        strategy: Option<&str>| {
        if let Err(error) = compile_template(value, descriptor) {
            let mut diagnostic = compiler_error(
                "invalid_detection_template",
                format!("Detection Template is invalid: {error}"),
                path,
                serde_json::json!({ "kind": format!("{:?}", error.kind), "offset": error.offset }),
            );
            diagnostic.strategy_key = strategy.map(str::to_string);
            diagnostics.push(diagnostic);
        }
    };

    for (index, check) in detection
        .http_checks
        .as_ref()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let descriptor =
            descriptor_for_placement(TemplatePlacement::DetectionHttpUrl, &keys_for(&captures));
        validate(
            &check.url,
            format!("/detection/httpChecks/{index}/url"),
            &descriptor,
            Some(&check.key),
        );
        if let Some(pattern) = &check.regex {
            extend_regex_captures(&mut captures, pattern);
        }
    }

    let proposal_before_browser =
        descriptor_for_placement(TemplatePlacement::DetectionProposal, &keys_for(&captures));
    fn visit(
        value: &serde_json::Value,
        path: String,
        descriptor: &TemplateDescriptor,
        validate: &mut impl FnMut(&str, String, &TemplateDescriptor, Option<&str>),
    ) {
        match value {
            serde_json::Value::String(value) => validate(value, path, descriptor, None),
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    visit(value, format!("{path}/{index}"), descriptor, validate);
                }
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    visit(
                        value,
                        format!("{path}/{}", json_pointer_segment(key)),
                        descriptor,
                        validate,
                    );
                }
            }
            _ => {}
        }
    }
    for (key, value) in detection.source_config.as_ref().into_iter().flatten() {
        visit(
            value,
            format!("/detection/sourceConfig/{}", json_pointer_segment(key)),
            &proposal_before_browser,
            &mut validate,
        );
    }

    for (index, probe) in detection
        .browser_probes
        .as_ref()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let descriptor =
            descriptor_for_placement(TemplatePlacement::DetectionBrowserUrl, &keys_for(&captures));
        validate(
            &probe.url,
            format!("/detection/browserProbes/{index}/url"),
            &descriptor,
            Some(&probe.key),
        );
        if let Some(pattern) = &probe.html_regex {
            extend_regex_captures(&mut captures, pattern);
        }
    }

    let proposal =
        descriptor_for_placement(TemplatePlacement::DetectionProposal, &keys_for(&captures));
    for (index, value) in detection
        .key_candidates
        .as_ref()
        .into_iter()
        .flatten()
        .enumerate()
    {
        validate(
            value,
            format!("/detection/keyCandidates/{index}"),
            &proposal,
            None,
        );
    }
    for (index, value) in detection
        .name_candidates
        .as_ref()
        .into_iter()
        .flatten()
        .enumerate()
    {
        validate(
            value,
            format!("/detection/nameCandidates/{index}"),
            &proposal,
            None,
        );
    }
}

fn extend_regex_captures(captures: &mut HashSet<String>, pattern: &str) {
    if let Ok(regex) = regex::Regex::new(pattern) {
        captures.extend(regex.capture_names().flatten().map(str::to_string));
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
