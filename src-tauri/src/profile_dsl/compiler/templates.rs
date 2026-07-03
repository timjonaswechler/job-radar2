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

mod fetch;
mod fields;
mod validation;

use fetch::validate_fetch_templates;
use fields::{
    posting_discovery_posting_meta_keys, validate_discovery_field_templates,
    validate_field_expression_templates,
};
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
