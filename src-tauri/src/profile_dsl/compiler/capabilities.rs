use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    DetailStep, DiscoveryStep, FieldExpression, ListFieldExpression, ParseType, Select,
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
        validate_select_compatibility(
            strategy.parse.parse_type,
            &strategy.select,
            &format!("{strategy_path}/select"),
            &strategy.key,
            diagnostics,
        );
        validate_discovery_extract_compatibility(
            strategy.parse.parse_type,
            discovery,
            index,
            &strategy_path,
            diagnostics,
        );
    }
    if let Some(detail) = detail {
        for (index, strategy) in detail.strategies.iter().enumerate() {
            let strategy_path = format!("{base_path}/detail/strategies/{index}");
            validate_select_compatibility(
                strategy.parse.parse_type,
                &strategy.select,
                &format!("{strategy_path}/select"),
                &strategy.key,
                diagnostics,
            );
            validate_field_extract_compatibility(
                strategy.parse.parse_type,
                &strategy.extract.fields.description_text,
                &format!("{strategy_path}/extract/fields/descriptionText"),
                &strategy.key,
                diagnostics,
            );
            if let Some(field_match) = &strategy.field_match {
                validate_field_extract_compatibility(
                    strategy.parse.parse_type,
                    &field_match.left,
                    &format!("{strategy_path}/match/left"),
                    &strategy.key,
                    diagnostics,
                );
                validate_field_extract_compatibility(
                    strategy.parse.parse_type,
                    &field_match.right,
                    &format!("{strategy_path}/match/right"),
                    &strategy.key,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_select_compatibility(
    parse_type: ParseType,
    select: &Select,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let compatible = matches!(
        (parse_type, select),
        (_, Select::Document)
            | (ParseType::Json, Select::JsonPath { .. })
            | (ParseType::Xml, Select::XmlElement { .. })
            | (ParseType::Xml, Select::XmlText { .. })
            | (ParseType::Xml, Select::SitemapUrls { .. })
            | (ParseType::Html, Select::Css { .. })
    );
    if !compatible {
        push_capability_diagnostic(
            diagnostics,
            "incompatible_parse_select_capability",
            format!(
                "Parse type `{}` is not compatible with select type `{}`",
                parse_type_name(parse_type),
                select_type_name(select)
            ),
            path,
            strategy_key,
            parse_type_name(parse_type),
            select_type_name(select),
        );
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
    match parse_type {
        ParseType::Json => "json",
        ParseType::Xml => "xml",
        ParseType::Html => "html",
        ParseType::Text => "text",
    }
}

fn select_type_name(select: &Select) -> &'static str {
    match select {
        Select::Document => "document",
        Select::JsonPath { .. } => "json_path",
        Select::XmlElement { .. } => "xml_element",
        Select::XmlText { .. } => "xml_text",
        Select::Css { .. } => "css",
        Select::SitemapUrls { .. } => "sitemap_urls",
    }
}
