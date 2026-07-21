use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    DetailStep, DiscoveryStep, FieldExpression, ListFieldExpression, Pagination, ParseType, Select,
};
use crate::profile_dsl::primitives::select::{
    compile_select, CompileSelectErrorKind, SelectCompileContext, SelectPhase, SelectPlacement,
};
use crate::profile_dsl::primitives::transform::{
    compile_transform_pipeline, CompileTransformErrorKind,
};

use super::compiler_error;

pub(super) fn validate_capability_compatibility(
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    for (index, strategy) in discovery.strategies.iter().enumerate() {
        let strategy_path = format!("{base_path}/discovery/strategies/{index}");
        validate_select_compilation(
            strategy.parse.parse_type(),
            &strategy.select,
            SelectPhase::Discovery,
            SelectPlacement::Strategy,
            &format!("{strategy_path}/select"),
            &strategy.key,
            diagnostics,
        );
        validate_sitemap_selectors(
            strategy.parse.parse_type(),
            strategy.pagination.as_ref(),
            &format!("{strategy_path}/pagination"),
            &strategy.key,
            diagnostics,
        );
        validate_discovery_extract_compatibility(
            strategy.parse.parse_type(),
            discovery,
            index,
            &strategy_path,
            diagnostics,
        );
    }
    if let Some(detail) = detail {
        for (index, strategy) in detail.strategies.iter().enumerate() {
            let strategy_path = format!("{base_path}/detail/strategies/{index}");
            validate_select_compilation(
                strategy.parse.parse_type(),
                &strategy.select,
                SelectPhase::Detail,
                SelectPlacement::Strategy,
                &format!("{strategy_path}/select"),
                &strategy.key,
                diagnostics,
            );
            validate_field_extract_compatibility(
                strategy.parse.parse_type(),
                &strategy.extract.fields.description_text,
                &format!("{strategy_path}/extract/fields/descriptionText"),
                &strategy.key,
                diagnostics,
            );
            if let Some(field_match) = &strategy.field_match {
                validate_field_extract_compatibility(
                    strategy.parse.parse_type(),
                    &field_match.left,
                    &format!("{strategy_path}/match/left"),
                    &strategy.key,
                    diagnostics,
                );
                validate_field_extract_compatibility(
                    strategy.parse.parse_type(),
                    &field_match.right,
                    &format!("{strategy_path}/match/right"),
                    &strategy.key,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_select_compilation(
    parse_type: ParseType,
    select: &Select,
    phase: SelectPhase,
    placement: SelectPlacement,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    if let Err(error) = compile_select(
        select,
        SelectCompileContext {
            document_type: parse_type,
            phase,
            placement,
        },
    ) {
        let code = match error.kind {
            CompileSelectErrorKind::DocumentIncompatible => "incompatible_parse_select_capability",
            CompileSelectErrorKind::Syntax => "invalid_select_syntax",
            CompileSelectErrorKind::Placement => "invalid_select_placement",
        };
        let path = if error.kind == CompileSelectErrorKind::Syntax {
            format!("{path}/{}", select_syntax_member(select))
        } else {
            path.to_string()
        };
        let mut diagnostic = compiler_error(
            code,
            error.message,
            path,
            serde_json::json!({ "parseType": parse_type.key() }),
        );
        diagnostic.strategy_key = Some(strategy_key.to_string());
        diagnostics.push(diagnostic);
    }
}

fn validate_sitemap_selectors(
    parse_type: ParseType,
    pagination: Option<&Pagination>,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let Some(Pagination::Sitemap {
        child_sitemap_selector,
        posting_url_selector,
        ..
    }) = pagination
    else {
        return;
    };

    if let Some(select) = child_sitemap_selector {
        validate_select_compilation(
            parse_type,
            select,
            SelectPhase::Discovery,
            SelectPlacement::SitemapChild,
            &format!("{path}/childSitemapSelector"),
            strategy_key,
            diagnostics,
        );
    }

    let default_posting = Select::SitemapUrls(Default::default());
    validate_select_compilation(
        parse_type,
        posting_url_selector.as_ref().unwrap_or(&default_posting),
        SelectPhase::Discovery,
        SelectPlacement::SitemapPosting,
        &format!("{path}/postingUrlSelector"),
        strategy_key,
        diagnostics,
    );
}

fn select_syntax_member(select: &Select) -> &'static str {
    match select.kind() {
        crate::profile_dsl::primitives::select::SelectKind::Document => "type",
        crate::profile_dsl::primitives::select::SelectKind::JsonPath => "jsonPath",
        crate::profile_dsl::primitives::select::SelectKind::XmlElement => "element",
        crate::profile_dsl::primitives::select::SelectKind::XmlText => "textPath",
        crate::profile_dsl::primitives::select::SelectKind::Css => "selector",
        crate::profile_dsl::primitives::select::SelectKind::SitemapUrls => "urlPattern",
    }
}

fn validate_discovery_extract_compatibility(
    parse_type: ParseType,
    discovery: &DiscoveryStep,
    strategy_index: usize,
    strategy_path: &str,
    diagnostics: &mut Diagnostics,
) {
    let strategy = &discovery.strategies[strategy_index];
    validate_field_extract_compatibility(
        parse_type,
        &strategy.extract.fields.title,
        &format!("{strategy_path}/extract/fields/title"),
        &strategy.key,
        diagnostics,
    );
    validate_field_extract_compatibility(
        parse_type,
        &strategy.extract.fields.company,
        &format!("{strategy_path}/extract/fields/company"),
        &strategy.key,
        diagnostics,
    );
    validate_field_extract_compatibility(
        parse_type,
        &strategy.extract.fields.url,
        &format!("{strategy_path}/extract/fields/url"),
        &strategy.key,
        diagnostics,
    );
    if let Some(locations) = &strategy.extract.fields.locations {
        validate_list_extract_compatibility(
            parse_type,
            locations,
            &format!("{strategy_path}/extract/fields/locations"),
            &strategy.key,
            diagnostics,
        );
    }
    if let Some(posting_meta) = &strategy.extract.fields.posting_meta {
        for (key, expression) in posting_meta {
            validate_field_extract_compatibility(
                parse_type,
                expression,
                &format!("{strategy_path}/extract/fields/postingMeta/{key}"),
                &strategy.key,
                diagnostics,
            );
        }
    }
    if let Some(description_text) = &strategy.extract.fields.description_text {
        validate_field_extract_compatibility(
            parse_type,
            description_text,
            &format!("{strategy_path}/extract/fields/descriptionText"),
            &strategy.key,
            diagnostics,
        );
    }
}

fn validate_list_extract_compatibility(
    parse_type: ParseType,
    expression: &ListFieldExpression,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    match expression {
        ListFieldExpression::Single(expression) => validate_field_extract_compatibility(
            parse_type,
            expression,
            path,
            strategy_key,
            diagnostics,
        ),
        ListFieldExpression::Multiple(expressions) => {
            for (index, expression) in expressions.iter().enumerate() {
                validate_field_extract_compatibility(
                    parse_type,
                    expression,
                    &format!("{path}/{index}"),
                    strategy_key,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_field_extract_compatibility(
    parse_type: ParseType,
    expression: &FieldExpression,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    if let Some(transforms) = expression.transforms() {
        if let Err(error) = compile_transform_pipeline(transforms) {
            let code = match error.kind {
                CompileTransformErrorKind::EmptySeparator => "transform_empty_separator",
                CompileTransformErrorKind::InvalidRegex => "transform_invalid_regex",
            };
            let mut diagnostic = compiler_error(
                code,
                error.message,
                format!("{path}/transforms/{}", error.transform_index),
                serde_json::json!({ "transformIndex": error.transform_index }),
            );
            diagnostic.strategy_key = Some(strategy_key.to_string());
            diagnostics.push(diagnostic);
        }
    }

    match expression {
        FieldExpression::JsonPath { .. } if parse_type != ParseType::Json => {
            push_capability_diagnostic(
                diagnostics,
                "incompatible_parse_extract_capability",
                format!(
                    "Parse type `{}` is not compatible with JSONPath extraction",
                    parse_type_name(parse_type)
                ),
                path,
                strategy_key,
                parse_type_name(parse_type),
                "json_path",
            )
        }
        FieldExpression::XmlText { .. } | FieldExpression::XmlElement { .. }
            if parse_type != ParseType::Xml =>
        {
            push_capability_diagnostic(
                diagnostics,
                "incompatible_parse_extract_capability",
                format!(
                    "Parse type `{}` is not compatible with XML extraction",
                    parse_type_name(parse_type)
                ),
                path,
                strategy_key,
                parse_type_name(parse_type),
                "xml",
            )
        }
        FieldExpression::CssText { .. } | FieldExpression::CssAttribute { .. }
            if parse_type != ParseType::Html =>
        {
            push_capability_diagnostic(
                diagnostics,
                "incompatible_parse_extract_capability",
                format!(
                    "Parse type `{}` is not compatible with CSS extraction",
                    parse_type_name(parse_type)
                ),
                path,
                strategy_key,
                parse_type_name(parse_type),
                "css",
            )
        }
        FieldExpression::Combine { parts, .. } => {
            for (index, part) in parts.iter().enumerate() {
                validate_field_extract_compatibility(
                    parse_type,
                    &part.value,
                    &format!("{path}/parts/{index}/value"),
                    strategy_key,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn push_capability_diagnostic(
    diagnostics: &mut Diagnostics,
    code: &str,
    message: String,
    path: &str,
    strategy_key: &str,
    parse_type: &str,
    capability_type: &str,
) {
    let mut diagnostic = compiler_error(
        code,
        message,
        path,
        serde_json::json!({
            "parseType": parse_type,
            "capabilityType": capability_type,
        }),
    );
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn parse_type_name(parse_type: ParseType) -> &'static str {
    parse_type.key()
}
