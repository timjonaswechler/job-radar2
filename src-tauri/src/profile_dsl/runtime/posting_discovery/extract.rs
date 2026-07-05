use super::document::RuntimeItem;
use super::support::render_source_config_template;
use super::values::{
    css_attribute_values, css_text_values, json_value_to_strings, xml_descendant_elements,
    xml_node_text, xml_path_texts, JsonStringsResult,
};
use super::*;
use crate::profile_dsl::documents::select::{CaptureRule, Captures, Filter};

mod captures;
mod fields;

use captures::evaluate_strategy_captures;
pub(super) use fields::RawFieldValues;
use fields::{apply_transforms, evaluate_string_field, raw_field_values, FieldEvaluation};

pub(super) fn extract_candidate(
    item: &RuntimeItem<'_, '_>,
    capture_rules: Option<&Captures>,
    conditions: Option<&Vec<Filter>>,
    fields: &ExecutionPlanPostingDiscoveryFields,
    source_config: &SourceConfig,
    source_name: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryCandidate> {
    let captures = evaluate_strategy_captures(
        item,
        capture_rules,
        source_config,
        source_name,
        base_path,
        strategy_key,
        item_index,
        diagnostics,
    )?;

    if !item_matches_conditions(
        item,
        conditions,
        source_config,
        source_name,
        &captures,
        base_path,
        strategy_key,
        item_index,
        diagnostics,
    )? {
        return None;
    }

    let title = extract_required_string_field(
        item,
        source_config,
        source_name,
        &captures,
        &fields.title,
        &format!("{base_path}/extract/fields/title"),
        strategy_key,
        item_index,
        diagnostics,
    );
    let company = extract_required_string_field(
        item,
        source_config,
        source_name,
        &captures,
        &fields.company,
        &format!("{base_path}/extract/fields/company"),
        strategy_key,
        item_index,
        diagnostics,
    );
    let url = extract_required_string_field(
        item,
        source_config,
        source_name,
        &captures,
        &fields.url,
        &format!("{base_path}/extract/fields/url"),
        strategy_key,
        item_index,
        diagnostics,
    );

    let locations = fields
        .locations
        .as_ref()
        .map(|expression| {
            extract_locations_field(
                item,
                source_config,
                source_name,
                &captures,
                expression,
                &format!("{base_path}/extract/fields/locations"),
                strategy_key,
                item_index,
                diagnostics,
            )
        })
        .unwrap_or_default();

    let posting_meta = fields
        .posting_meta
        .as_ref()
        .map(|meta_fields| {
            let mut meta = BTreeMap::new();
            for (key, expression) in meta_fields {
                if let FieldEvaluation {
                    value: Some(value),
                    failed: false,
                } = evaluate_string_field(
                    item,
                    source_config,
                    source_name,
                    &captures,
                    expression,
                    &format!("{base_path}/extract/fields/postingMeta/{key}"),
                    strategy_key,
                    item_index,
                    diagnostics,
                ) {
                    meta.insert(key.clone(), value);
                }
            }
            meta
        })
        .unwrap_or_default();

    let description_text = fields.description_text.as_ref().and_then(|expression| {
        match evaluate_string_field(
            item,
            source_config,
            source_name,
            &captures,
            expression,
            &format!("{base_path}/extract/fields/descriptionText"),
            strategy_key,
            item_index,
            diagnostics,
        ) {
            FieldEvaluation {
                value: Some(value),
                failed: false,
            } => Some(value),
            _ => None,
        }
    });

    match (title, company, url) {
        (Some(title), Some(company), Some(url)) => Some(PostingDiscoveryCandidate {
            title,
            company,
            url,
            locations,
            posting_meta,
            description_text,
        }),
        _ => None,
    }
}

fn item_matches_conditions(
    item: &RuntimeItem<'_, '_>,
    conditions: Option<&Vec<Filter>>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let Some(conditions) = conditions else {
        return Some(true);
    };

    for (condition_index, condition) in conditions.iter().enumerate() {
        let condition_path = format!("{base_path}/where/{condition_index}");
        match condition {
            Filter::NonEmpty { field } => {
                let evaluation = evaluate_string_field(
                    item,
                    source_config,
                    source_name,
                    captures,
                    field,
                    &format!("{condition_path}/field"),
                    strategy_key,
                    item_index,
                    diagnostics,
                );
                if evaluation.failed {
                    return None;
                }
                if evaluation.value.is_none() {
                    return Some(false);
                }
            }
            Filter::Regex { field, pattern } => {
                let regex = match Regex::new(pattern) {
                    Ok(regex) => regex,
                    Err(error) => {
                        diagnostics.push(runtime_error(
                            "where_pattern_invalid",
                            format!("Where filter regex pattern is invalid: {error}"),
                            format!("{condition_path}/pattern"),
                            strategy_key,
                            json!({ "pattern": pattern, "error": error.to_string() }),
                        ));
                        return None;
                    }
                };
                let evaluation = evaluate_string_field(
                    item,
                    source_config,
                    source_name,
                    captures,
                    field,
                    &format!("{condition_path}/field"),
                    strategy_key,
                    item_index,
                    diagnostics,
                );
                if evaluation.failed {
                    return None;
                }
                let Some(value) = evaluation.value else {
                    return Some(false);
                };
                if !regex.is_match(&value) {
                    return Some(false);
                }
            }
        }
    }

    Some(true)
}

fn extract_required_string_field(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    match evaluate_string_field(
        item,
        source_config,
        source_name,
        captures,
        expression,
        path,
        strategy_key,
        item_index,
        diagnostics,
    ) {
        FieldEvaluation {
            value: Some(value),
            failed: false,
        } => Some(value),
        FieldEvaluation {
            value: None,
            failed: false,
        } => {
            diagnostics.push(runtime_error(
                "required_field_missing",
                "Required postingDiscovery field did not resolve to a non-empty string",
                path,
                strategy_key,
                json!({ "itemIndex": item_index }),
            ));
            None
        }
        FieldEvaluation { failed: true, .. } => None,
    }
}

fn extract_locations_field(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &ListFieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Vec<String> {
    let (expressions, is_single): (Vec<&FieldExpression>, bool) = match expression {
        ListFieldExpression::Single(expression) => (vec![expression], true),
        ListFieldExpression::Multiple(expressions) => (expressions.iter().collect(), false),
    };

    let mut locations = Vec::new();
    for (index, expression) in expressions.into_iter().enumerate() {
        let expression_path = if is_single {
            path.to_string()
        } else {
            format!("{path}/{index}")
        };
        let RawFieldValues {
            values,
            failed,
            transforms,
            ..
        } = raw_field_values(
            item,
            source_config,
            source_name,
            captures,
            expression,
            &expression_path,
            strategy_key,
            item_index,
            diagnostics,
        );
        if failed {
            continue;
        }
        let Some(values) = apply_transforms(
            values,
            transforms,
            &expression_path,
            strategy_key,
            item_index,
            diagnostics,
        ) else {
            continue;
        };
        for value in values {
            let value = normalize_whitespace(value.trim());
            if !value.is_empty() && !locations.contains(&value) {
                locations.push(value);
            }
        }
    }
    locations
}
