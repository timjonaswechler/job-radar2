use quick_xml::{escape::unescape, events::Event, Reader};
use regex::Regex;
use reqwest::Url;
use serde_json::Value;
use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use crate::{
    declarative_template::{render_template, TemplateContext, TemplateError},
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutionSource, SourceExecutor,
    },
    simple_json_path::{resolve_simple_json_path, SimpleJsonPathError},
};

const DECLARATIVE_HTTP_ADAPTER_KEY: &str = "declarative_endpoint_inventory";
const DECLARATIVE_SITEMAP_ADAPTER_KEY: &str = "declarative_sitemap_inventory";

pub(crate) struct DeclarativeInventoryExecutor<C = ReqwestInventoryHttpClient> {
    client: C,
}

impl DeclarativeInventoryExecutor<ReqwestInventoryHttpClient> {
    pub(crate) fn new_reqwest() -> Self {
        Self {
            client: ReqwestInventoryHttpClient,
        }
    }
}

impl<C> DeclarativeInventoryExecutor<C> {
    #[cfg(test)]
    fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C> SourceExecutor for DeclarativeInventoryExecutor<C>
where
    C: InventoryHttpClient + Send + Sync,
{
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(input).await })
    }
}

impl<C> DeclarativeInventoryExecutor<C>
where
    C: InventoryHttpClient + Send + Sync,
{
    async fn execute_source(
        &self,
        input: SourceExecutionInput<'_>,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        let source = input.source;
        if !matches!(
            source.adapter_key.as_str(),
            DECLARATIVE_HTTP_ADAPTER_KEY | DECLARATIVE_SITEMAP_ADAPTER_KEY
        ) {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by DeclarativeInventoryExecutor",
                source.adapter_key
            )));
        }

        let inventory = source
            .inventory()
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SourceExecutionError::Failed(format!(
                    "executionPlan.inventory must be a JSON object for source {}",
                    source.key
                ))
            })?;
        let fetch = required_object_value(inventory, "fetch", "executionPlan.inventory.fetch")?;
        let fetch_url_template =
            required_string(fetch, "url", "executionPlan.inventory.fetch.url")?;
        let empty_captures = HashMap::new();
        let fetch_context = InventoryTemplateContext {
            source,
            item: None,
            captures: &empty_captures,
        };
        let fetch_url = render_template(fetch_url_template, &fetch_context).map_err(|error| {
            SourceExecutionError::Failed(format!(
                "executionPlan.inventory.fetch.url is invalid: {error}"
            ))
        })?;
        let fetch_url = parse_http_url(&fetch_url, "executionPlan.inventory.fetch.url")?;

        let parse = required_object_value(inventory, "parse", "executionPlan.inventory.parse")?;
        let parse_as = required_string(parse, "as", "executionPlan.inventory.parse.as")?;
        let items = required_object_value(inventory, "items", "executionPlan.inventory.items")?;
        let inventory_items = match parse_as {
            "xml" => {
                let body = self.fetch_inventory_text(fetch_url.clone()).await?;
                select_xml_items(&body, items)?
            }
            "json" => {
                self.select_json_inventory_items(fetch_url.clone(), fetch, items)
                    .await?
            }
            other => {
                return Err(SourceExecutionError::Failed(format!(
                    "executionPlan.inventory.parse.as `{other}` is not supported by this executor slice"
                )));
            }
        };

        let where_regexes =
            compile_regex_list(items.get("where"), "executionPlan.inventory.items.where")?;
        let capture_regexes = compile_regex_list(
            items.get("captures"),
            "executionPlan.inventory.items.captures",
        )?;
        let fields = required_object_value(inventory, "fields", "executionPlan.inventory.fields")?;

        let mut candidates = Vec::new();
        for inventory_item in inventory_items {
            let captures = match inventory_item.text() {
                Some(item_text) => {
                    if !where_regexes.iter().all(|regex| regex.is_match(item_text)) {
                        continue;
                    }
                    let Some(captures) = capture_item(&capture_regexes, item_text) else {
                        continue;
                    };
                    captures
                }
                None => {
                    if !where_regexes.is_empty() {
                        return Err(SourceExecutionError::Failed(
                            "executionPlan.inventory.items.where is only supported for text item selections"
                                .to_string(),
                        ));
                    }
                    if !capture_regexes.is_empty() {
                        return Err(SourceExecutionError::Failed(
                            "executionPlan.inventory.items.captures is only supported for text item selections"
                                .to_string(),
                        ));
                    }
                    HashMap::new()
                }
            };
            let context = InventoryTemplateContext {
                source,
                item: Some(&inventory_item),
                captures: &captures,
            };

            let title = render_required_field(fields, "title", &context)?;
            let raw_url = render_required_field(fields, "url", &context)?;
            let url = resolve_http_candidate_url(&raw_url, &fetch_url)
                .unwrap_or_else(|| raw_url.trim().to_string());
            let company = render_required_field(fields, "company", &context)?;
            let locations = render_locations(fields, &context)?;

            if title.trim().is_empty() || url.trim().is_empty() || company.trim().is_empty() {
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

    async fn fetch_inventory_text(&self, fetch_url: Url) -> Result<String, SourceExecutionError> {
        self.client
            .get_text(fetch_url.clone())
            .await
            .map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not fetch inventory {}: {error}",
                    fetch_url.as_str()
                ))
            })
    }

    async fn select_json_inventory_items(
        &self,
        fetch_url: Url,
        fetch: &serde_json::Map<String, Value>,
        items: &serde_json::Map<String, Value>,
    ) -> Result<Vec<InventoryItem>, SourceExecutionError> {
        let Some(pagination_value) = fetch.get("pagination") else {
            let body = self.fetch_inventory_text(fetch_url).await?;
            return select_json_items(&body, items);
        };

        let pagination = parse_page_count_pagination(
            pagination_value,
            "executionPlan.inventory.fetch.pagination",
        )?;
        let first_url = page_count_pagination_url(&fetch_url, &pagination, pagination.first_page);
        let first_body = self.fetch_inventory_text(first_url).await?;
        let (mut inventory_items, first_root) = select_json_items_with_root(&first_body, items)?;
        let total = resolve_json_u64(
            &first_root,
            &pagination.total_path,
            "executionPlan.inventory.fetch.pagination.totalPath",
        )?;
        let page_count = total.div_ceil(pagination.size);
        if page_count <= 1 {
            return Ok(inventory_items);
        }

        let last_page = pagination.first_page + page_count - 1;
        for page in (pagination.first_page + 1)..=last_page {
            let page_url = page_count_pagination_url(&fetch_url, &pagination, page);
            let page_body = self.fetch_inventory_text(page_url).await?;
            inventory_items.extend(select_json_items(&page_body, items)?);
        }

        Ok(inventory_items)
    }
}

#[derive(Clone, Debug)]
enum InventoryItem {
    Text(String),
    Json(Value),
}

impl InventoryItem {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Json(_) => None,
        }
    }

    fn json(&self) -> Option<&Value> {
        match self {
            Self::Text(_) => None,
            Self::Json(value) => Some(value),
        }
    }
}

fn select_xml_items(
    xml: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    let select = required_object_value(items, "select", "executionPlan.inventory.items.select")?;
    let element_name = required_string(
        select,
        "xmlText",
        "executionPlan.inventory.items.select.xmlText",
    )?;
    if element_name.trim().is_empty() {
        return Err(SourceExecutionError::Failed(
            "executionPlan.inventory.items.select.xmlText must not be empty".to_string(),
        ));
    }

    parse_xml_text_values(xml, element_name)
        .map(|values| values.into_iter().map(InventoryItem::Text).collect())
        .map_err(|error| {
            SourceExecutionError::Failed(format!("could not parse inventory XML: {error}"))
        })
}

fn select_json_items(
    json_text: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    select_json_items_with_root(json_text, items).map(|(items, _root)| items)
}

fn select_json_items_with_root(
    json_text: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<(Vec<InventoryItem>, Value), SourceExecutionError> {
    let root = serde_json::from_str::<Value>(json_text).map_err(|error| {
        SourceExecutionError::Failed(format!("could not parse inventory JSON: {error}"))
    })?;
    let inventory_items = select_json_items_from_root(&root, items)?;
    Ok((inventory_items, root))
}

fn select_json_items_from_root(
    root: &Value,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    let select = required_object_value(items, "select", "executionPlan.inventory.items.select")?;
    let json_path = required_string(
        select,
        "jsonPath",
        "executionPlan.inventory.items.select.jsonPath",
    )?;
    let selected = resolve_simple_json_path(root, json_path)
        .map_err(|error| simple_json_path_execution_error("executionPlan.inventory.items.select.jsonPath", error))?
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "executionPlan.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but no value was found"
            ))
        })?;
    let array = selected.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "executionPlan.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but resolved to {}",
            json_type_label(selected)
        ))
    })?;

    Ok(array.iter().cloned().map(InventoryItem::Json).collect())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PageCountPagination {
    page_param: String,
    size_param: String,
    size: u64,
    first_page: u64,
    total_path: String,
}

fn parse_page_count_pagination(
    value: &Value,
    path: &str,
) -> Result<PageCountPagination, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    let pagination_type = required_string(object, "type", &format!("{path}.type"))?;
    if pagination_type != "page_count" {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.type `{pagination_type}` is not supported by this executor slice"
        )));
    }

    let page_param = required_string(object, "pageParam", &format!("{path}.pageParam"))?;
    let size_param = required_string(object, "sizeParam", &format!("{path}.sizeParam"))?;
    let size = required_u64(object, "size", &format!("{path}.size"))?;
    if size == 0 {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.size must be greater than 0"
        )));
    }
    let first_page = optional_u64(object, "firstPage", &format!("{path}.firstPage"))?.unwrap_or(1);
    let total_path = required_string(object, "totalPath", &format!("{path}.totalPath"))?;

    Ok(PageCountPagination {
        page_param: page_param.to_string(),
        size_param: size_param.to_string(),
        size,
        first_page,
        total_path: total_path.to_string(),
    })
}

fn page_count_pagination_url(base_url: &Url, pagination: &PageCountPagination, page: u64) -> Url {
    let overrides = [
        (pagination.size_param.as_str(), pagination.size.to_string()),
        (pagination.page_param.as_str(), page.to_string()),
    ];
    query_param_override_url(base_url, &overrides)
}

fn query_param_override_url(base_url: &Url, overrides: &[(&str, String)]) -> Url {
    let mut url = base_url.clone();
    let existing_pairs = url
        .query_pairs()
        .filter(|(key, _value)| {
            !overrides
                .iter()
                .any(|(override_key, _)| key == *override_key)
        })
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    url.set_query(None);
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in existing_pairs {
            pairs.append_pair(&key, &value);
        }
        for (key, value) in overrides {
            pairs.append_pair(key, value);
        }
    }
    url
}

fn resolve_json_u64(
    root: &Value,
    json_path: &str,
    path: &str,
) -> Result<u64, SourceExecutionError> {
    let selected = resolve_simple_json_path(root, json_path)
        .map_err(|error| simple_json_path_execution_error(path, error))?
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path} `{json_path}` must resolve to a non-negative integer, but no value was found"
            ))
        })?;
    selected.as_u64().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "{path} `{json_path}` must resolve to a non-negative integer, but resolved to {}",
            json_type_label(selected)
        ))
    })
}

fn parse_xml_text_values(xml: &str, element_name: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let target = element_name.as_bytes();
    let mut selected_depth = 0_usize;
    let mut current_text = String::new();
    let mut values = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if selected_depth > 0 {
                    selected_depth += 1;
                } else if element.local_name().as_ref() == target {
                    selected_depth = 1;
                    current_text.clear();
                }
            }
            Ok(Event::Empty(element)) => {
                if selected_depth == 0 && element.local_name().as_ref() == target {
                    values.push(String::new());
                }
            }
            Ok(Event::Text(text)) if selected_depth > 0 => {
                let decoded = text
                    .xml10_content()
                    .map_err(|error| format!("text could not be decoded: {error}"))?;
                let unescaped = unescape(decoded.as_ref())
                    .map_err(|error| format!("text could not be unescaped: {error}"))?;
                current_text.push_str(unescaped.as_ref());
            }
            Ok(Event::GeneralRef(reference)) if selected_depth > 0 => {
                let decoded = reference
                    .xml10_content()
                    .map_err(|error| format!("entity could not be decoded: {error}"))?;
                let entity = format!("&{};", decoded.as_ref());
                let unescaped = unescape(&entity)
                    .map_err(|error| format!("entity could not be unescaped: {error}"))?;
                current_text.push_str(unescaped.as_ref());
            }
            Ok(Event::CData(cdata)) if selected_depth > 0 => {
                let decoded = cdata
                    .xml10_content()
                    .map_err(|error| format!("CDATA could not be decoded: {error}"))?;
                current_text.push_str(decoded.as_ref());
            }
            Ok(Event::End(_)) if selected_depth > 0 => {
                selected_depth -= 1;
                if selected_depth == 0 {
                    let value = current_text.trim();
                    if !value.is_empty() {
                        values.push(value.to_string());
                    }
                    current_text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(values)
}

fn simple_json_path_execution_error(
    path: &str,
    error: SimpleJsonPathError,
) -> SourceExecutionError {
    SourceExecutionError::Failed(format!("{path} {error}"))
}

fn json_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn compile_regex_list(
    value: Option<&Value>,
    path: &str,
) -> Result<Vec<Regex>, SourceExecutionError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be an array")))?;

    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry_path = format!("{path}[{index}]");
            let object = entry.as_object().ok_or_else(|| {
                SourceExecutionError::Failed(format!("{entry_path} must be a JSON object"))
            })?;
            let pattern = required_string(object, "regex", &format!("{entry_path}.regex"))?;
            Regex::new(pattern).map_err(|error| {
                SourceExecutionError::Failed(format!("{entry_path}.regex is invalid: {error}"))
            })
        })
        .collect()
}

fn capture_item(regexes: &[Regex], item_text: &str) -> Option<HashMap<String, String>> {
    let mut values = HashMap::new();
    for regex in regexes {
        let captures = regex.captures(item_text)?;
        for capture_name in regex.capture_names().flatten() {
            if let Some(value) = captures.name(capture_name) {
                values.insert(capture_name.to_string(), value.as_str().to_string());
            }
        }
    }
    Some(values)
}

fn render_required_field(
    fields: &serde_json::Map<String, Value>,
    field_name: &str,
    context: &InventoryTemplateContext<'_>,
) -> Result<String, SourceExecutionError> {
    let field = fields.get(field_name).ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "executionPlan.inventory.fields.{field_name} is required"
        ))
    })?;
    render_field_expression(
        field,
        context,
        &format!("executionPlan.inventory.fields.{field_name}"),
    )
}

fn render_locations(
    fields: &serde_json::Map<String, Value>,
    context: &InventoryTemplateContext<'_>,
) -> Result<Vec<String>, SourceExecutionError> {
    let locations = fields.get("locations").ok_or_else(|| {
        SourceExecutionError::Failed(
            "executionPlan.inventory.fields.locations is required".to_string(),
        )
    })?;
    let locations = locations.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(
            "executionPlan.inventory.fields.locations must be an array".to_string(),
        )
    })?;

    let mut rendered_locations = Vec::new();
    for (index, location) in locations.iter().enumerate() {
        rendered_locations.extend(render_location_expression(
            location,
            context,
            &format!("executionPlan.inventory.fields.locations[{index}]"),
        )?);
    }
    dedupe_preserving_order(&mut rendered_locations);
    Ok(rendered_locations)
}

fn render_location_expression(
    value: &Value,
    context: &InventoryTemplateContext<'_>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    let split = optional_location_split(object, path)?;

    if let Some(template) = object.get("template").and_then(Value::as_str) {
        let rendered = render_template(template, context).map_err(|error| {
            SourceExecutionError::Failed(format!("{path}.template is invalid: {error}"))
        })?;
        return Ok(location_string_values(&rendered, split));
    }

    if let Some(json_path) = object.get("jsonPath") {
        let json_path = json_path.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(format!("{path}.jsonPath must be a non-empty string"))
        })?;
        if json_path.trim().is_empty() {
            return Err(SourceExecutionError::Failed(format!(
                "{path}.jsonPath must be a non-empty string"
            )));
        }
        let item = context.item.and_then(InventoryItem::json).ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path}.jsonPath is only available for JSON inventory items"
            ))
        })?;
        let value = resolve_simple_json_path(item, json_path).map_err(|error| {
            simple_json_path_execution_error(&format!("{path}.jsonPath"), error)
        })?;
        return json_location_value_to_strings(value, split, path);
    }

    Err(SourceExecutionError::Failed(format!(
        "{path} must contain a template or jsonPath expression"
    )))
}

fn optional_location_split<'a>(
    object: &'a serde_json::Map<String, Value>,
    path: &str,
) -> Result<Option<&'a str>, SourceExecutionError> {
    let Some(split) = object.get("split") else {
        return Ok(None);
    };
    let delimiter = split
        .as_str()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path}.split must be a string")))?;
    if delimiter.is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.split must not be empty"
        )));
    }
    Ok(Some(delimiter))
}

fn json_location_value_to_strings(
    value: Option<&Value>,
    split: Option<&str>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(value)) => Ok(location_string_values(value, split)),
        Some(Value::Bool(value)) => Ok(location_string_values(&value.to_string(), None)),
        Some(Value::Number(value)) => Ok(location_string_values(&value.to_string(), None)),
        Some(Value::Array(values)) => {
            let mut locations = Vec::new();
            for (index, value) in values.iter().enumerate() {
                match value {
                    Value::Null => {}
                    Value::String(value) => locations.extend(location_string_values(value, split)),
                    Value::Bool(value) => locations.extend(location_string_values(&value.to_string(), None)),
                    Value::Number(value) => {
                        locations.extend(location_string_values(&value.to_string(), None))
                    }
                    Value::Array(_) | Value::Object(_) => {
                        return Err(SourceExecutionError::Failed(format!(
                            "{path}.jsonPath array item {index} must resolve to a string, number, boolean, or null"
                        )));
                    }
                }
            }
            Ok(locations)
        }
        Some(Value::Object(_)) => Err(SourceExecutionError::Failed(format!(
            "{path}.jsonPath must resolve to a string, number, boolean, null, or an array of those values"
        ))),
    }
}

fn location_string_values(value: &str, split: Option<&str>) -> Vec<String> {
    match split {
        Some(delimiter) => value
            .split(delimiter)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        None => {
            let value = value.trim();
            if value.is_empty() {
                Vec::new()
            } else {
                vec![value.to_string()]
            }
        }
    }
}

fn dedupe_preserving_order(values: &mut Vec<String>) {
    let mut seen = Vec::<String>::new();
    values.retain(|value| {
        if seen.iter().any(|seen_value| seen_value == value) {
            false
        } else {
            seen.push(value.clone());
            true
        }
    });
}

fn render_field_expression(
    value: &Value,
    context: &InventoryTemplateContext<'_>,
    path: &str,
) -> Result<String, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    if let Some(template) = object.get("template").and_then(Value::as_str) {
        return render_template(template, context).map_err(|error| {
            SourceExecutionError::Failed(format!("{path}.template is invalid: {error}"))
        });
    }

    if let Some(json_path) = object.get("jsonPath") {
        let json_path = json_path.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(format!("{path}.jsonPath must be a non-empty string"))
        })?;
        if json_path.trim().is_empty() {
            return Err(SourceExecutionError::Failed(format!(
                "{path}.jsonPath must be a non-empty string"
            )));
        }
        let item = context.item.and_then(InventoryItem::json).ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path}.jsonPath is only available for JSON inventory items"
            ))
        })?;
        let value = resolve_simple_json_path(item, json_path).map_err(|error| {
            simple_json_path_execution_error(&format!("{path}.jsonPath"), error)
        })?;
        return json_field_value_to_string(value, path);
    }

    Err(SourceExecutionError::Failed(format!(
        "{path} must contain a template or jsonPath expression"
    )))
}

fn json_field_value_to_string(
    value: Option<&Value>,
    path: &str,
) -> Result<String, SourceExecutionError> {
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Array(_) | Value::Object(_)) => Err(SourceExecutionError::Failed(format!(
            "{path}.jsonPath must resolve to a string, number, boolean, or null"
        ))),
    }
}

struct InventoryTemplateContext<'a> {
    source: &'a SourceExecutionSource,
    item: Option<&'a InventoryItem>,
    captures: &'a HashMap<String, String>,
}

impl TemplateContext for InventoryTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "sourceName" {
            Ok(Some(self.source.name.clone()))
        } else if variable == "sourceKey" {
            Ok(Some(self.source.key.clone()))
        } else if variable == "itemText" {
            self.item
                .and_then(InventoryItem::text)
                .map(str::to_string)
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(
                        "itemText is not available in this template context".to_string(),
                    )
                })
        } else if let Some(config_key) = variable.strip_prefix("sourceConfig:") {
            if config_key.is_empty() {
                return Err(TemplateError::Invalid(
                    "sourceConfig template variable must include a key".to_string(),
                ));
            }
            source_config_value_as_string(&self.source.source_config, config_key)
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(format!("sourceConfig.{config_key} is not available"))
                })
        } else if let Some(capture_key) = variable.strip_prefix("capture:") {
            if capture_key.is_empty() {
                return Err(TemplateError::Invalid(
                    "capture template variable must include a capture name".to_string(),
                ));
            }
            self.captures
                .get(capture_key)
                .cloned()
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(format!("capture `{capture_key}` is not available"))
                })
        } else {
            Err(TemplateError::Invalid(format!(
                "unsupported template variable `{variable}`"
            )))
        }
    }
}

fn source_config_value_as_string(source_config: &Value, key: &str) -> Option<String> {
    let value = source_config.get(key)?;
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn required_object_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, SourceExecutionError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, SourceExecutionError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-empty string"))
    })?;
    if value.trim().is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "{path} must be a non-empty string"
        )));
    }
    Ok(value)
}

fn required_u64(
    object: &serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<u64, SourceExecutionError> {
    object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-negative integer"))
    })
}

fn optional_u64(
    object: &serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<u64>, SourceExecutionError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    value.as_u64().map(Some).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-negative integer"))
    })
}

fn resolve_http_candidate_url(raw_url: &str, base_url: &Url) -> Option<String> {
    let raw_url = raw_url.trim();
    if raw_url.is_empty() {
        return None;
    }
    let url = base_url.join(raw_url).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}

fn parse_http_url(value: &str, field: &str) -> Result<Url, SourceExecutionError> {
    let url = Url::parse(value.trim()).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "{field} must be an absolute http or https URL: {error}"
        ))
    })?;

    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err(SourceExecutionError::Failed(format!(
            "{field} must be an absolute http or https URL"
        )))
    }
}

type BoxedTextFuture<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait InventoryHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_>;
}

pub(crate) struct ReqwestInventoryHttpClient;

impl InventoryHttpClient for ReqwestInventoryHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("JobRadarDeclarativeInventoryExecutor/0.1")
                .build()
                .map_err(|error| error.to_string())?;
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if !response.status().is_success() {
                return Err(format!("HTTP {}", response.status()));
            }
            response.text().await.map_err(|error| error.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        search_request_model::{
            CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
            SearchRequestStatus, SearchRuleInput,
        },
        search_run_model::{
            DefaultSourceExecutor, SearchRunService, SearchRunStatus, SourceRunStatus,
        },
        source_registry::ResolvedSelectedAccessPath,
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::{path::Path, sync::Mutex};

    struct FixtureInventoryHttpClient {
        responses: HashMap<String, Result<String, String>>,
        requested_urls: Mutex<Vec<String>>,
    }

    impl FixtureInventoryHttpClient {
        fn new(
            responses: impl IntoIterator<Item = (&'static str, Result<&'static str, &'static str>)>,
        ) -> Self {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(url, response)| {
                        (
                            url.to_string(),
                            response.map(str::to_string).map_err(str::to_string),
                        )
                    })
                    .collect(),
                requested_urls: Mutex::new(Vec::new()),
            }
        }

        fn requested_urls(&self) -> Vec<String> {
            self.requested_urls.lock().unwrap().clone()
        }
    }

    impl InventoryHttpClient for FixtureInventoryHttpClient {
        fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
            Box::pin(async move {
                self.requested_urls
                    .lock()
                    .unwrap()
                    .push(url.as_str().to_string());
                self.responses
                    .get(url.as_str())
                    .cloned()
                    .unwrap_or_else(|| Err(format!("{} not found", url.as_str())))
            })
        }
    }

    #[test]
    fn inventory_template_context_uses_shared_renderer_and_filters() {
        let source = SourceExecutionSource {
            key: "focused_energy".to_string(),
            adapter_key: DECLARATIVE_HTTP_ADAPTER_KEY.to_string(),
            name: "Focused Energy".to_string(),
            source_config: json!({
                "startUrl": "https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true"
            }),
            effective_source_config_schema: json!({ "type": "object" }),
            selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
                query: None,
                inventory: None,
                interactions: None,
                manual_release: None,
            },
        };
        let item = InventoryItem::Text(
            "https://example.com/job/Berlin-Senior+Rust%2DEngineer-123/".to_string(),
        );
        let captures = HashMap::from([
            ("location".to_string(), "berlin".to_string()),
            ("title".to_string(), "senior+rust%2Dengineer".to_string()),
        ]);
        let context = InventoryTemplateContext {
            source: &source,
            item: Some(&item),
            captures: &captures,
        };

        let rendered = render_template(
            "{{sourceKey}}|{{sourceConfig:startUrl}}|{{itemText}}|{{capture:title|urlDecode|slugToTitle}}|{{sourceName}}",
            &context,
        )
        .unwrap();

        assert_eq!(
            rendered,
            "focused_energy|https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true|https://example.com/job/Berlin-Senior+Rust%2DEngineer-123/|Senior Rust Engineer|Focused Energy"
        );
    }

    #[test]
    fn json_inventory_executes_from_resolved_execution_plan() {
        tauri::async_runtime::block_on(async {
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.test/jobs.json",
                Ok(r#"{
                  "jobs": [
                    {
                      "title": "Laser Engineer",
                      "jobUrl": "https://example.test/jobs/laser",
                      "location": "Mainz"
                    }
                  ]
                }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let search_request = search_request();
            let source = source_with_inventory(
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json!({ "startUrl": "https://example.test/jobs.json" }),
                json_jobs_inventory("{{sourceConfig:startUrl}}"),
            );

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .unwrap();

            assert_eq!(
                candidates,
                vec![SourceCandidate {
                    title: "Laser Engineer".to_string(),
                    company: "Fixture Careers".to_string(),
                    url: "https://example.test/jobs/laser".to_string(),
                    locations: vec!["Mainz".to_string()],
                }]
            );
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://example.test/jobs.json"]
            );
        });
    }

    #[test]
    fn json_inventory_paginates_endpoint_and_resolves_relative_urls() {
        tauri::async_runtime::block_on(async {
            let fixture_client = FixtureInventoryHttpClient::new([
                (
                    "https://example.test/.search?index=job&size=2&page=1",
                    Ok(r#"{
                      "total": 3,
                      "searchResults": [
                        {
                          "title": "Backend Engineer",
                          "url": "/jobs/backend",
                          "location": "Berlin"
                        },
                        {
                          "title": "Frontend Engineer",
                          "url": "/jobs/frontend",
                          "location": "Hamburg"
                        }
                      ]
                    }"#),
                ),
                (
                    "https://example.test/.search?index=job&size=2&page=2",
                    Ok(r#"{
                      "total": 3,
                      "searchResults": [
                        {
                          "title": "Platform Engineer",
                          "url": "/jobs/platform",
                          "location": "Mainz"
                        }
                      ]
                    }"#),
                ),
            ]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let search_request = search_request();
            let source = source_with_inventory(
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json!({ "endpointUrl": "https://example.test/.search?index=job" }),
                json!({
                    "fetch": {
                        "url": "{{sourceConfig:endpointUrl}}",
                        "pagination": {
                            "type": "page_count",
                            "pageParam": "page",
                            "sizeParam": "size",
                            "size": 2,
                            "firstPage": 1,
                            "totalPath": "$.total"
                        }
                    },
                    "parse": { "as": "json" },
                    "items": {
                        "select": { "jsonPath": "$.searchResults" }
                    },
                    "fields": {
                        "title": { "jsonPath": "$.title" },
                        "url": { "jsonPath": "$.url" },
                        "company": { "template": "{{sourceName}}" },
                        "locations": [
                            { "jsonPath": "$.location" }
                        ]
                    }
                }),
            );

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .unwrap();

            assert_eq!(
                candidates,
                vec![
                    SourceCandidate {
                        title: "Backend Engineer".to_string(),
                        company: "Fixture Careers".to_string(),
                        url: "https://example.test/jobs/backend".to_string(),
                        locations: vec!["Berlin".to_string()],
                    },
                    SourceCandidate {
                        title: "Frontend Engineer".to_string(),
                        company: "Fixture Careers".to_string(),
                        url: "https://example.test/jobs/frontend".to_string(),
                        locations: vec!["Hamburg".to_string()],
                    },
                    SourceCandidate {
                        title: "Platform Engineer".to_string(),
                        company: "Fixture Careers".to_string(),
                        url: "https://example.test/jobs/platform".to_string(),
                        locations: vec!["Mainz".to_string()],
                    },
                ]
            );
            assert_eq!(
                executor.client.requested_urls(),
                vec![
                    "https://example.test/.search?index=job&size=2&page=1",
                    "https://example.test/.search?index=job&size=2&page=2",
                ]
            );
        });
    }

    #[test]
    fn json_inventory_locations_expand_arrays_split_strings_and_dedupe() {
        tauri::async_runtime::block_on(async {
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.test/jobs.json",
                Ok(r#"{
                  "jobs": [
                    {
                      "title": "Platform Engineer",
                      "jobUrl": "https://example.test/jobs/platform",
                      "locations": ["Berlin, Germany", "Munich, Germany"],
                      "fallbackLocations": "Munich, Germany; Hamburg, Germany; "
                    }
                  ]
                }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let search_request = search_request();
            let source = source_with_inventory(
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json!({ "startUrl": "https://example.test/jobs.json" }),
                json!({
                    "fetch": { "url": "{{sourceConfig:startUrl}}" },
                    "parse": { "as": "json" },
                    "items": {
                        "select": { "jsonPath": "$.jobs" }
                    },
                    "fields": {
                        "title": { "jsonPath": "$.title" },
                        "url": { "jsonPath": "$.jobUrl" },
                        "company": { "template": "{{sourceName}}" },
                        "locations": [
                            { "jsonPath": "$.locations" },
                            { "jsonPath": "$.fallbackLocations", "split": ";" }
                        ]
                    }
                }),
            );

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .unwrap();

            assert_eq!(
                candidates,
                vec![SourceCandidate {
                    title: "Platform Engineer".to_string(),
                    company: "Fixture Careers".to_string(),
                    url: "https://example.test/jobs/platform".to_string(),
                    locations: vec![
                        "Berlin, Germany".to_string(),
                        "Munich, Germany".to_string(),
                        "Hamburg, Germany".to_string(),
                    ],
                }]
            );
        });
    }

    #[test]
    fn xml_inventory_source_runs_through_search_run_with_source_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_profile_backed_source(
                temp_dir.path(),
                "example",
                "Example",
                DECLARATIVE_SITEMAP_ADAPTER_KEY,
                xml_loc_inventory(),
                inventory_source_config_schema(DECLARATIVE_SITEMAP_ADAPTER_KEY),
                json!({ "url": "https://example.com/sitemap.xml" }),
            );
            let search_request =
                create_search_request(&pool, vec!["example".to_string()], "laser").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.com/sitemap.xml",
                Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <url>
                    <loc>https://example.com/job/Mainz-Laser-Engineer-123/</loc>
                  </url>
                  <url>
                    <loc>https://example.com/about</loc>
                  </url>
                </urlset>"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 1);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Laser Engineer");
            assert_eq!(posting.company, "Example");
            assert_eq!(
                posting.url,
                "https://example.com/job/Mainz-Laser-Engineer-123/"
            );
            assert_eq!(posting.locations, vec!["Mainz"]);
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://example.com/sitemap.xml"]
            );
        });
    }

    #[test]
    fn successfactors_builtin_inventory_runs_schott_sitemap_fixture_through_central_runtime() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_builtin_profile_source(
                temp_dir.path(),
                "schott_ag",
                "SCHOTT AG",
                "successfactors",
                "sitemap_inventory",
                json!({
                    "url": "https://join.schott.com/sitemap.xml",
                    "recursive": false
                }),
            );
            let search_request =
                create_search_request(&pool, vec!["schott_ag".to_string()], "laser").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://join.schott.com/sitemap.xml",
                Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <url>
                    <loc>https://join.schott.com/job/Mainz-Laser-Engineer-55122/</loc>
                  </url>
                  <url>
                    <loc>https://join.schott.com/about-schott/</loc>
                  </url>
                </urlset>"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 1);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Laser Engineer");
            assert_eq!(posting.company, "SCHOTT AG");
            assert_eq!(
                posting.url,
                "https://join.schott.com/job/Mainz-Laser-Engineer-55122/"
            );
            assert_eq!(posting.locations, vec!["Mainz"]);
            assert_eq!(posting.sources[0].source_key, "schott_ag");
            assert_eq!(posting.sources[0].source_name, "SCHOTT AG");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://join.schott.com/sitemap.xml"]
            );
        });
    }

    #[test]
    fn ashby_json_inventory_source_runs_through_search_run_with_source_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_builtin_profile_source(
                temp_dir.path(),
                "focused_energy",
                "Focused Energy",
                "ashby",
                "endpoint_inventory",
                json!({
                    "boardSlug": "focused",
                    "companyWebsite": "https://focused-energy.co"
                }),
            );
            let search_request =
                create_search_request(&pool, vec!["focused_energy".to_string()], "photonics").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true",
                Ok(r#"{
                  "jobs": [
                    {
                      "title": "Photonics Engineer",
                      "jobUrl": "https://jobs.ashbyhq.com/focused/abc",
                      "location": "Darmstadt"
                    }
                  ]
                }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 1);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Photonics Engineer");
            assert_eq!(posting.company, "Focused Energy");
            assert_eq!(posting.url, "https://jobs.ashbyhq.com/focused/abc");
            assert_eq!(posting.locations, vec!["Darmstadt"]);
            assert_eq!(posting.sources[0].source_key, "focused_energy");
            assert_eq!(posting.sources[0].source_name, "Focused Energy");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true"]
            );
        });
    }

    #[test]
    fn greenhouse_json_inventory_source_runs_through_search_run_with_source_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_builtin_profile_source(
                temp_dir.path(),
                "d3",
                "D3",
                "greenhouse",
                "endpoint_inventory",
                json!({
                    "boardSlug": "d3"
                }),
            );
            let search_request =
                create_search_request(&pool, vec!["d3".to_string()], "backend").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://boards-api.greenhouse.io/v1/boards/d3/jobs",
                Ok(r#"{
                  "jobs": [
                    {
                      "title": "Backend Engineer - New Grad",
                      "absolute_url": "https://job-boards.greenhouse.io/d3/jobs/4915295008",
                      "location": { "name": "Los Angeles, CA" }
                    }
                  ]
                }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 1);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Backend Engineer - New Grad");
            assert_eq!(posting.company, "D3");
            assert_eq!(
                posting.url,
                "https://job-boards.greenhouse.io/d3/jobs/4915295008"
            );
            assert_eq!(posting.locations, vec!["Los Angeles, CA"]);
            assert_eq!(posting.sources[0].source_key, "d3");
            assert_eq!(posting.sources[0].source_name, "D3");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://boards-api.greenhouse.io/v1/boards/d3/jobs"]
            );
        });
    }

    #[test]
    fn lever_json_inventory_source_runs_through_search_run_with_source_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_builtin_profile_source(
                temp_dir.path(),
                "leverdemo",
                "Lever Demo",
                "lever",
                "endpoint_inventory",
                json!({
                    "boardSlug": "leverdemo"
                }),
            );
            let search_request =
                create_search_request(&pool, vec!["leverdemo".to_string()], "backend").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://api.lever.co/v0/postings/leverdemo?mode=json",
                Ok(r#"[
                  {
                    "text": "Backend Engineer",
                    "hostedUrl": "https://jobs.lever.co/leverdemo/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd",
                    "categories": {
                      "location": "Berlin, Germany",
                      "allLocations": ["Berlin, Germany", "Munich, Germany"]
                    }
                  }
                ]"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 1);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Backend Engineer");
            assert_eq!(posting.company, "Lever Demo");
            assert_eq!(
                posting.url,
                "https://jobs.lever.co/leverdemo/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd"
            );
            assert_eq!(
                posting.locations,
                vec!["Berlin, Germany", "Munich, Germany"]
            );
            assert_eq!(posting.sources[0].source_key, "leverdemo");
            assert_eq!(posting.sources[0].source_name, "Lever Demo");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://api.lever.co/v0/postings/leverdemo?mode=json"]
            );
        });
    }

    #[test]
    fn magnolia_esmp_job_search_inventory_paginates_relative_urls() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_builtin_profile_source(
                temp_dir.path(),
                "example_magnolia",
                "Example Magnolia",
                "magnolia_esmp_job_search",
                "endpoint_inventory",
                json!({
                    "startUrl": "https://example.test/karriere/jobsuche",
                    "endpointUrl": "https://example.test/.search?index=job"
                }),
            );
            let search_request =
                create_search_request(&pool, vec!["example_magnolia".to_string()], "engineer")
                    .await;
            let fixture_client = FixtureInventoryHttpClient::new([
                (
                    "https://example.test/.search?index=job&size=1000&page=1",
                    Ok(r#"{
                      "page": 1,
                      "pageSize": 1000,
                      "total": 1001,
                      "searchResults": [
                        {
                          "title": "Backend Engineer",
                          "url": "/karriere/stellenanzeigen/backend",
                          "location": "Berlin"
                        },
                        {
                          "title": "Frontend Engineer",
                          "url": "/karriere/stellenanzeigen/frontend",
                          "location": "Hamburg"
                        }
                      ]
                    }"#),
                ),
                (
                    "https://example.test/.search?index=job&size=1000&page=2",
                    Ok(r#"{
                      "page": 2,
                      "pageSize": 1000,
                      "total": 1001,
                      "searchResults": [
                        {
                          "title": "Platform Engineer",
                          "url": "/karriere/stellenanzeigen/platform",
                          "location": "Mainz"
                        }
                      ]
                    }"#),
                ),
            ]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Completed);
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 3);
            assert_eq!(result.source_runs[0].matched_count, 3);
            assert_eq!(result.postings.len(), 3);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Backend Engineer");
            assert_eq!(posting.company, "Example Magnolia");
            assert_eq!(
                posting.url,
                "https://example.test/karriere/stellenanzeigen/backend"
            );
            assert_eq!(posting.locations, vec!["Berlin"]);
            assert_eq!(posting.sources[0].source_key, "example_magnolia");
            assert_eq!(posting.sources[0].source_name, "Example Magnolia");
            assert_eq!(
                executor.client.requested_urls(),
                vec![
                    "https://example.test/.search?index=job&size=1000&page=1",
                    "https://example.test/.search?index=job&size=1000&page=2",
                ]
            );
        });
    }

    #[test]
    fn json_inventory_reports_profile_author_error_when_items_path_is_not_array() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_profile_backed_source(
                temp_dir.path(),
                "focused_energy",
                "Focused Energy",
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json_jobs_inventory("{{sourceConfig:startUrl}}"),
                inventory_source_config_schema(DECLARATIVE_HTTP_ADAPTER_KEY),
                json!({ "startUrl": "https://example.com/jobs.json" }),
            );
            let search_request =
                create_search_request(&pool, vec!["focused_energy".to_string()], "photonics").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.com/jobs.json",
                Ok(r#"{
                  "jobs": {
                    "title": "Photonics Engineer",
                    "jobUrl": "https://jobs.ashbyhq.com/focused/abc",
                    "location": "Darmstadt"
                  }
                }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            assert!(result.postings.is_empty());
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            assert_eq!(result.source_runs[0].candidate_count, 0);
            assert_eq!(result.source_runs[0].matched_count, 0);
            let error = result.source_runs[0].error.as_deref().unwrap();
            assert!(error.contains(
                "executionPlan.inventory.items.select.jsonPath `$.jobs` must resolve to an array"
            ));
            assert!(error.contains("resolved to object"));
        });
    }

    #[test]
    fn json_inventory_execution_rejects_wildcards_to_document_simple_dot_jsonpath_scope() {
        tauri::async_runtime::block_on(async {
            let mut inventory = json_jobs_inventory("{{sourceConfig:startUrl}}");
            inventory["items"]["select"]["jsonPath"] = json!("$.jobs[*]");
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.com/jobs.json",
                Ok(r#"{ "jobs": [] }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let search_request = search_request();
            let source = source_with_inventory(
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json!({ "startUrl": "https://example.com/jobs.json" }),
                inventory,
            );

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .unwrap_err();

            let SourceExecutionError::Failed(message) = error else {
                panic!("expected failed source execution");
            };
            assert!(message.contains(
                "executionPlan.inventory.items.select.jsonPath `$.jobs[*]` is not supported"
            ));
            assert!(message.contains("simple dot JSONPath only"));
            assert!(message.contains("filters and wildcards are not supported"));
        });
    }

    #[test]
    fn xml_inventory_fetch_errors_become_source_run_errors() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_profile_backed_source(
                temp_dir.path(),
                "broken",
                "Broken",
                DECLARATIVE_SITEMAP_ADAPTER_KEY,
                xml_loc_inventory(),
                inventory_source_config_schema(DECLARATIVE_SITEMAP_ADAPTER_KEY),
                json!({ "url": "https://broken.example/sitemap.xml" }),
            );
            let search_request =
                create_search_request(&pool, vec!["broken".to_string()], "engineer").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://broken.example/sitemap.xml",
                Err("connection refused"),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            assert!(result.postings.is_empty());
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            assert_eq!(result.source_runs[0].candidate_count, 0);
            assert_eq!(result.source_runs[0].matched_count, 0);
            assert!(result.source_runs[0]
                .error
                .as_deref()
                .unwrap()
                .contains("could not fetch inventory https://broken.example/sitemap.xml"));
            assert!(result.source_runs[0]
                .error
                .as_deref()
                .unwrap()
                .contains("connection refused"));
        });
    }

    #[test]
    fn declarative_source_without_inventory_fails_source_run_clearly() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let temp_dir = tempfile::tempdir().unwrap();
            write_profile_backed_source_without_inventory(
                temp_dir.path(),
                "inventory_missing_source",
                "Inventory Missing",
                DECLARATIVE_HTTP_ADAPTER_KEY,
                inventory_source_config_schema(DECLARATIVE_HTTP_ADAPTER_KEY),
                json!({ "startUrl": "https://example.com/jobs.json" }),
            );
            let search_request = create_search_request(
                &pool,
                vec!["inventory_missing_source".to_string()],
                "engineer",
            )
            .await;
            let executor = DeclarativeInventoryExecutor::new(FixtureInventoryHttpClient::new([]));
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            assert!(result.postings.is_empty());
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            assert_eq!(result.source_runs[0].candidate_count, 0);
            assert_eq!(result.source_runs[0].matched_count, 0);
            assert_eq!(
                result.source_runs[0].error.as_deref(),
                Some("executionPlan.inventory must be a JSON object for source inventory_missing_source")
            );
            assert!(executor.client.requested_urls().is_empty());
        });
    }

    #[test]
    fn default_source_executor_routes_declarative_adapters_to_inventory_runtime() {
        tauri::async_runtime::block_on(async {
            let executor = DefaultSourceExecutor::new(
                tempfile::tempdir().unwrap().path().join("browser-runtime"),
            );
            let search_request = search_request();

            for adapter_key in [
                DECLARATIVE_HTTP_ADAPTER_KEY,
                DECLARATIVE_SITEMAP_ADAPTER_KEY,
            ] {
                let source = source(adapter_key);
                let error = executor
                    .execute(SourceExecutionInput {
                        search_request: &search_request,
                        source: &source,
                    })
                    .await
                    .unwrap_err();

                match error {
                    SourceExecutionError::Failed(message) => {
                        assert!(message.contains("executionPlan.inventory"));
                        assert!(!message.contains("has no search-run executor yet"));
                    }
                    SourceExecutionError::Cancelled(message) => {
                        panic!("expected failed source execution, got cancellation: {message}")
                    }
                }
            }
        });
    }

    fn xml_loc_inventory() -> Value {
        json!({
            "fetch": { "url": "{{sourceConfig:url}}" },
            "parse": { "as": "xml" },
            "items": {
                "select": { "xmlText": "loc" },
                "where": [{ "regex": "(?i)/job/" }],
                "captures": [{
                    "regex": "(?i)/job/(?P<location>[^/-]+)-(?P<title>.+?)(?:-\\d+)?(?:-\\d+)?/?$"
                }]
            },
            "fields": {
                "title": { "template": "{{capture:title|urlDecode|slugToTitle}}" },
                "url": { "template": "{{itemText}}" },
                "company": { "template": "{{sourceName}}" },
                "locations": [
                    { "template": "{{capture:location|urlDecode|slugToTitle}}" }
                ]
            }
        })
    }

    fn json_jobs_inventory(fetch_url_template: &str) -> Value {
        json!({
            "fetch": { "url": fetch_url_template },
            "parse": { "as": "json" },
            "items": {
                "select": { "jsonPath": "$.jobs" }
            },
            "fields": {
                "title": { "jsonPath": "$.title" },
                "url": { "jsonPath": "$.jobUrl" },
                "company": { "template": "{{sourceName}}" },
                "locations": [
                    { "jsonPath": "$.location" }
                ]
            }
        })
    }

    fn inventory_source_config_schema(adapter_key: &str) -> Value {
        if adapter_key == DECLARATIVE_HTTP_ADAPTER_KEY {
            json!({
                "type": "object",
                "required": ["startUrl"],
                "properties": {
                    "startUrl": { "type": "string", "format": "uri" }
                }
            })
        } else {
            json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string", "format": "uri" }
                }
            })
        }
    }

    fn write_profile_backed_source(
        app_data_dir: &Path,
        source_key: &str,
        source_name: &str,
        adapter_key: &str,
        inventory: Value,
        source_config_schema: Value,
        source_config: Value,
    ) {
        write_profile_backed_source_inner(
            app_data_dir,
            source_key,
            source_name,
            adapter_key,
            Some(inventory),
            source_config_schema,
            source_config,
        );
    }

    fn write_profile_backed_source_without_inventory(
        app_data_dir: &Path,
        source_key: &str,
        source_name: &str,
        adapter_key: &str,
        source_config_schema: Value,
        source_config: Value,
    ) {
        write_profile_backed_source_inner(
            app_data_dir,
            source_key,
            source_name,
            adapter_key,
            None,
            source_config_schema,
            source_config,
        );
    }

    fn write_profile_backed_source_inner(
        app_data_dir: &Path,
        source_key: &str,
        source_name: &str,
        adapter_key: &str,
        inventory: Option<Value>,
        source_config_schema: Value,
        source_config: Value,
    ) {
        let profile_key = format!("{source_key}_profile");
        let mut access_path = json!({
            "key": "inventory",
            "adapterKey": adapter_key,
            "sourceConfigSchema": source_config_schema
        });
        if let Some(inventory) = inventory {
            access_path["inventory"] = inventory;
        }
        write_json(
            app_data_dir.join(format!("source-profiles/{profile_key}.json")),
            &json!({
                "schemaVersion": 1,
                "key": profile_key,
                "name": format!("{source_name} Profile"),
                "kind": "generic",
                "accessPaths": [access_path]
            })
            .to_string(),
        );
        write_builtin_profile_source(
            app_data_dir,
            source_key,
            source_name,
            &profile_key,
            "inventory",
            source_config,
        );
    }

    fn write_builtin_profile_source(
        app_data_dir: &Path,
        source_key: &str,
        source_name: &str,
        profile_key: &str,
        path_key: &str,
        source_config: Value,
    ) {
        write_json(
            app_data_dir.join(format!("sources/{source_key}.json")),
            &json!({
                "schemaVersion": 1,
                "key": source_key,
                "name": source_name,
                "status": "active",
                "sourceConfig": source_config,
                "selectedAccessPath": {
                    "type": "profile",
                    "profileKey": profile_key,
                    "pathKey": path_key
                }
            })
            .to_string(),
        );
    }

    async fn create_search_request(
        pool: &SqlitePool,
        source_keys: Vec<String>,
        include_text: &str,
    ) -> SearchRequest {
        let running_search_runs = RunningSearchRuns::default();
        SearchRequestService::new(pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule(include_text)],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys,
            })
            .await
            .unwrap()
    }

    fn text_rule(value: &str) -> SearchRuleInput {
        SearchRuleInput {
            target: "title".to_string(),
            kind: "text".to_string(),
            value: value.to_string(),
        }
    }

    fn search_request() -> SearchRequest {
        SearchRequest {
            id: 1,
            status: SearchRequestStatus::Active,
            include_rules: vec![],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: vec!["fixture_source".to_string()],
            validation_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn source(adapter_key: &str) -> SourceExecutionSource {
        source_with_inventory(adapter_key, json!({}), Value::Null)
    }

    fn source_with_inventory(
        adapter_key: &str,
        source_config: Value,
        inventory: Value,
    ) -> SourceExecutionSource {
        SourceExecutionSource {
            key: "fixture_source".to_string(),
            adapter_key: adapter_key.to_string(),
            name: "Fixture Careers".to_string(),
            source_config,
            effective_source_config_schema: json!({ "type": "object" }),
            selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
                query: None,
                inventory: if inventory.is_null() {
                    None
                } else {
                    Some(inventory)
                },
                interactions: None,
                manual_release: None,
            },
        }
    }

    fn write_json(path: impl AsRef<Path>, contents: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    async fn migrated_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        pool
    }
}
