use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    DetailStep, DiscoveryStep, Fetch, FieldExpression, JsonObject, ListFieldExpression, RequestBody,
};

use super::{compiler_error, SourceRuntimeBinding, SourceRuntimeBindingDependencies};

#[derive(Clone, Copy)]
enum TemplateContext {
    Discovery,
    Detail,
}

mod fetch;
mod fields;
mod validation;

use fetch::validate_fetch_templates;
use fields::{
    discovery_posting_meta_keys, validate_discovery_field_templates,
    validate_field_expression_templates,
};
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
            TemplateContext::Discovery,
            &source_config_keys,
            &captures,
            &posting_meta_keys,
            diagnostics,
            &mut dependencies,
        );
        validate_discovery_field_templates(
            discovery,
            index,
            &strategy_path,
            TemplateContext::Discovery,
            &source_config_keys,
            &captures,
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
            let strategy_path = format!("{base_path}/postingDetail/strategies/{index}");
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
            validate_field_expression_templates(
                &strategy.extract.fields.description_text,
                &format!("{strategy_path}/extract/fields/descriptionText"),
                strategy.key.as_str(),
                TemplateContext::Detail,
                &source_config_keys,
                &captures,
                &posting_meta_keys,
                diagnostics,
                &mut dependencies,
            );
            if let Some(field_match) = &strategy.field_match {
                validate_field_expression_templates(
                    &field_match.left,
                    &format!("{strategy_path}/match/left"),
                    strategy.key.as_str(),
                    TemplateContext::Detail,
                    &source_config_keys,
                    &captures,
                    &posting_meta_keys,
                    diagnostics,
                    &mut dependencies,
                );
                validate_field_expression_templates(
                    &field_match.right,
                    &format!("{strategy_path}/match/right"),
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
    }
    dependencies
}
