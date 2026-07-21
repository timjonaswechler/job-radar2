use super::compiler_error;
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep, Pagination, ParseType, Select};
use crate::profile_dsl::primitives::select::{
    compile_select, CompileSelectErrorKind, SelectCompileContext, SelectPhase, SelectPlacement,
};

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
