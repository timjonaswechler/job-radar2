//! Declarative browser source-inventory adapter backed by registry execution plans.
//!
//! This adapter satisfies the `SourceExecutor` seam for Quellen with
//! `adapter_key = declarative_browser_inventory`. The external representation is
//! the resolved source registry access path: optional `query`, ordered
//! `interactions`, and `inventory` definitions. The module translates that JSON
//! shape into Job Radar `SourceCandidate` values and maps selector/browser
//! failures to `SourceExecutionError::Failed`.
//!
//! Minimal browser inventory language:
//!
//! - `executionPlan.query` is optional and can build a query-parameterized URL
//!   from `baseUrl`, `path`, and an ordered `params` array. When absent,
//!   `sourceConfig.startUrl` is used as the page URL.
//! - Query param templates may use `{{searchRequest:titleText}}`,
//!   `{{searchRequest:firstLocation}}`, and `{{searchRequest:radiusKm}}`.
//! - The first `waitFor` entry in `executionPlan.interactions` is passed to the
//!   managed browser runtime.
//! - `executionPlan.inventory.items.select` is a CSS selector for job cards.
//! - `executionPlan.inventory.fields.title`, `company`, and `url` use exactly
//!   one of `selectorText` or `selectorAttribute`.
//! - `executionPlan.inventory.fields.locations` is an array of the same field
//!   expressions and may yield zero or more locations.

use dom_query::{Document, Matcher, Selection};
use reqwest::Url;
use serde_json::{Map, Value};

use crate::search::run::{SourceCandidate, SourceExecutionError, SourceExecutionSource};

use super::*;

pub(super) fn extract_candidates(
    source: &SourceExecutionSource,
    rendered_html: &str,
    page_url: &Url,
) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
    let inventory = source
        .inventory()
        .and_then(Value::as_object)
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "executionPlan.inventory must be a JSON object for source {}",
                source.key
            ))
        })?;
    let items = required_object_value(
        inventory,
        "items",
        &plan_path(source, "executionPlan.inventory.items"),
    )?;
    validate_allowed_keys(
        items,
        &["select"],
        &plan_path(source, "executionPlan.inventory.items"),
    )?;
    let item_selector = required_string(
        items,
        "select",
        &plan_path(source, "executionPlan.inventory.items.select"),
    )?;
    let item_matcher = compile_selector(
        item_selector,
        &plan_path(source, "executionPlan.inventory.items.select"),
    )?;

    let fields = required_object_value(
        inventory,
        "fields",
        &plan_path(source, "executionPlan.inventory.fields"),
    )?;
    validate_allowed_keys(
        fields,
        &["title", "company", "url", "locations"],
        &plan_path(source, "executionPlan.inventory.fields"),
    )?;

    let document = Document::from(rendered_html);
    let mut candidates = Vec::new();
    for item in document.select_matcher(&item_matcher).iter() {
        let title = render_required_field(source, fields, "title", &item)?;
        let company = render_required_field(source, fields, "company", &item)?;
        let raw_url = render_required_field(source, fields, "url", &item)?;
        let url = resolve_http_candidate_url(&raw_url, page_url).unwrap_or_default();
        let locations = render_locations(source, fields, &item)?;

        if title.trim().is_empty() || company.trim().is_empty() || url.trim().is_empty() {
            continue;
        }

        candidates.push(SourceCandidate {
            title,
            company,
            url,
            locations,
        });
    }

    Ok(candidates)
}

pub(super) fn render_required_field(
    source: &SourceExecutionSource,
    fields: &Map<String, Value>,
    field_name: &str,
    item: &Selection<'_>,
) -> Result<String, SourceExecutionError> {
    let path = plan_path(
        source,
        &format!("executionPlan.inventory.fields.{field_name}"),
    );
    let field = fields
        .get(field_name)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} is required")))?;
    let values = render_field_values(field, item, &path)?;

    Ok(values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default())
}

pub(super) fn render_locations(
    source: &SourceExecutionSource,
    fields: &Map<String, Value>,
    item: &Selection<'_>,
) -> Result<Vec<String>, SourceExecutionError> {
    let path = plan_path(source, "executionPlan.inventory.fields.locations");
    let locations = fields
        .get("locations")
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be an array")))?;
    let locations = locations
        .as_array()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be an array")))?;

    let mut values = Vec::new();
    for (index, location) in locations.iter().enumerate() {
        values.extend(render_field_values(
            location,
            item,
            &plan_path(
                source,
                &format!("executionPlan.inventory.fields.locations[{index}]"),
            ),
        )?);
    }

    Ok(values)
}

pub(super) fn render_field_values(
    value: &Value,
    item: &Selection<'_>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    let has_selector_text = object.contains_key("selectorText");
    let has_selector_attribute = object.contains_key("selectorAttribute");

    match (has_selector_text, has_selector_attribute) {
        (true, false) => {
            validate_allowed_keys(object, &["selectorText"], path)?;
            let selector = required_string(object, "selectorText", &format!("{path}.selectorText"))?;
            selector_text_values(item, selector, &format!("{path}.selectorText"))
        }
        (false, true) => {
            validate_allowed_keys(object, &["selectorAttribute"], path)?;
            let selector_attribute = required_object_value(
                object,
                "selectorAttribute",
                &format!("{path}.selectorAttribute"),
            )?;
            validate_allowed_keys(
                selector_attribute,
                &["selector", "attribute"],
                &format!("{path}.selectorAttribute"),
            )?;
            let selector = required_string(
                selector_attribute,
                "selector",
                &format!("{path}.selectorAttribute.selector"),
            )?;
            let attribute = required_string(
                selector_attribute,
                "attribute",
                &format!("{path}.selectorAttribute.attribute"),
            )?;
            selector_attribute_values(
                item,
                selector,
                attribute,
                &format!("{path}.selectorAttribute.selector"),
            )
        }
        _ => Err(SourceExecutionError::Failed(format!(
            "{path} must contain exactly one browser field expression: selectorText or selectorAttribute"
        ))),
    }
}

pub(super) fn selector_text_values(
    item: &Selection<'_>,
    selector: &str,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let matcher = compile_selector(selector, path)?;
    Ok(item
        .select_matcher(&matcher)
        .iter()
        .map(|selection| selection.text().to_string())
        .collect())
}

pub(super) fn selector_attribute_values(
    item: &Selection<'_>,
    selector: &str,
    attribute: &str,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let matcher = compile_selector(selector, path)?;
    Ok(item
        .select_matcher(&matcher)
        .iter()
        .filter_map(|selection| selection.attr(attribute).map(|value| value.to_string()))
        .collect())
}

pub(super) fn compile_selector(
    selector: &str,
    path: &str,
) -> Result<Matcher, SourceExecutionError> {
    Matcher::new(selector).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "{path} must be a valid CSS selector for the browser inventory language: {error:?}"
        ))
    })
}
