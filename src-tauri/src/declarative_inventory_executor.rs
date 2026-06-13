use quick_xml::{escape::unescape, events::Event, Reader};
use regex::Regex;
use reqwest::Url;
use serde_json::Value;
use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use crate::{
    declarative_template::{render_template, TemplateContext, TemplateError},
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutor,
    },
    simple_json_path::{resolve_simple_json_path, SimpleJsonPathError},
    source_model::Source,
};

const DECLARATIVE_HTTP_ADAPTER_KEY: &str = "declarative_http_jobboard";
const DECLARATIVE_SITEMAP_ADAPTER_KEY: &str = "declarative_sitemap_jobboard";

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

        let system_profile = input.system_profile.ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "adapterKey {} requires an active SystemProfile for source {}",
                source.adapter_key, source.key
            ))
        })?;
        if system_profile.adapter_key != source.adapter_key {
            return Err(SourceExecutionError::Failed(format!(
                "source {} uses adapterKey {}, but SystemProfile {} uses adapterKey {}",
                source.key, source.adapter_key, system_profile.key, system_profile.adapter_key
            )));
        }

        let inventory = required_object(
            &system_profile.definition,
            "inventory",
            "definition.inventory",
        )?;
        let fetch = required_object_value(inventory, "fetch", "definition.inventory.fetch")?;
        let fetch_url_template = required_string(fetch, "url", "definition.inventory.fetch.url")?;
        let empty_captures = HashMap::new();
        let fetch_context = InventoryTemplateContext {
            source,
            item: None,
            captures: &empty_captures,
        };
        let fetch_url = render_template(fetch_url_template, &fetch_context).map_err(|error| {
            SourceExecutionError::Failed(format!(
                "definition.inventory.fetch.url is invalid: {error}"
            ))
        })?;
        let fetch_url = parse_http_url(&fetch_url, "definition.inventory.fetch.url")?;
        let body = self
            .client
            .get_text(fetch_url.clone())
            .await
            .map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not fetch inventory {}: {error}",
                    fetch_url.as_str()
                ))
            })?;

        let parse = required_object_value(inventory, "parse", "definition.inventory.parse")?;
        let parse_as = required_string(parse, "as", "definition.inventory.parse.as")?;
        let items = required_object_value(inventory, "items", "definition.inventory.items")?;
        let inventory_items = match parse_as {
            "xml" => select_xml_items(&body, items)?,
            "json" => select_json_items(&body, items)?,
            other => {
                return Err(SourceExecutionError::Failed(format!(
                    "definition.inventory.parse.as `{other}` is not supported by this executor slice"
                )));
            }
        };

        let where_regexes =
            compile_regex_list(items.get("where"), "definition.inventory.items.where")?;
        let capture_regexes =
            compile_regex_list(items.get("captures"), "definition.inventory.items.captures")?;
        let fields = required_object_value(inventory, "fields", "definition.inventory.fields")?;

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
                            "definition.inventory.items.where is only supported for text item selections"
                                .to_string(),
                        ));
                    }
                    if !capture_regexes.is_empty() {
                        return Err(SourceExecutionError::Failed(
                            "definition.inventory.items.captures is only supported for text item selections"
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
            let url = render_required_field(fields, "url", &context)?;
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
    let select = required_object_value(items, "select", "definition.inventory.items.select")?;
    let element_name = required_string(
        select,
        "xmlText",
        "definition.inventory.items.select.xmlText",
    )?;
    if element_name.trim().is_empty() {
        return Err(SourceExecutionError::Failed(
            "definition.inventory.items.select.xmlText must not be empty".to_string(),
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
    let root = serde_json::from_str::<Value>(json_text).map_err(|error| {
        SourceExecutionError::Failed(format!("could not parse inventory JSON: {error}"))
    })?;
    let select = required_object_value(items, "select", "definition.inventory.items.select")?;
    let json_path = required_string(
        select,
        "jsonPath",
        "definition.inventory.items.select.jsonPath",
    )?;
    let selected = resolve_simple_json_path(&root, json_path)
        .map_err(|error| simple_json_path_execution_error("definition.inventory.items.select.jsonPath", error))?
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "definition.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but no value was found"
            ))
        })?;
    let array = selected.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "definition.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but resolved to {}",
            json_type_label(selected)
        ))
    })?;

    Ok(array.iter().cloned().map(InventoryItem::Json).collect())
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
            "definition.inventory.fields.{field_name} is required"
        ))
    })?;
    render_field_expression(
        field,
        context,
        &format!("definition.inventory.fields.{field_name}"),
    )
}

fn render_locations(
    fields: &serde_json::Map<String, Value>,
    context: &InventoryTemplateContext<'_>,
) -> Result<Vec<String>, SourceExecutionError> {
    let locations = fields.get("locations").ok_or_else(|| {
        SourceExecutionError::Failed(
            "definition.inventory.fields.locations is required".to_string(),
        )
    })?;
    let locations = locations.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(
            "definition.inventory.fields.locations must be an array".to_string(),
        )
    })?;

    locations
        .iter()
        .enumerate()
        .map(|(index, location)| {
            render_field_expression(
                location,
                context,
                &format!("definition.inventory.fields.locations[{index}]"),
            )
        })
        .filter_map(|location| match location {
            Ok(location) if location.trim().is_empty() => None,
            other => Some(other),
        })
        .collect()
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
    source: &'a Source,
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

fn required_object<'a>(
    value: &'a Value,
    key: &str,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, SourceExecutionError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))
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
        source_model::{
            create_source, create_system_profile, CreateSourceInput, CreateSystemProfileInput,
            Source, SourceStatus,
        },
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::sync::Mutex;

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
        let source = Source {
            id: 1,
            key: "focused_energy_careers".to_string(),
            adapter_key: DECLARATIVE_HTTP_ADAPTER_KEY.to_string(),
            system_profile_id: Some(1),
            browser_profile_id: None,
            name: "Focused Energy Karriere".to_string(),
            description: None,
            source_config: json!({
                "startUrl": "https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true"
            }),
            status: SourceStatus::Active,
            validation_error: None,
            built_in: false,
            created_at: String::new(),
            updated_at: String::new(),
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
            "{{sourceKey}}|{{sourceConfig:startUrl}}|{{itemText}}|{{capture:title|urlDecode|slugToTitle}}|{{sourceName|stripCareerSuffix}}",
            &context,
        )
        .unwrap();

        assert_eq!(
            rendered,
            "focused_energy_careers|https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true|https://example.com/job/Berlin-Senior+Rust%2DEngineer-123/|Senior Rust Engineer|Focused Energy"
        );
    }

    #[test]
    fn xml_inventory_source_runs_through_search_run_with_system_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_id = create_inventory_source(
                &pool,
                DECLARATIVE_SITEMAP_ADAPTER_KEY,
                xml_loc_inventory(),
                json!({ "url": "https://example.com/sitemap.xml" }),
                "Example Careers",
            )
            .await;
            let search_request = create_search_request(&pool, vec![source_id], "laser").await;
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
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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
            let source_id = create_builtin_successfactors_source(&pool).await;
            let search_request = create_search_request(&pool, vec![source_id], "laser").await;
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
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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
            assert_eq!(posting.company, "SCHOTT");
            assert_eq!(
                posting.url,
                "https://join.schott.com/job/Mainz-Laser-Engineer-55122/"
            );
            assert_eq!(posting.locations, vec!["Mainz"]);
            assert_eq!(posting.sources[0].source_key, "schott_careers");
            assert_eq!(posting.sources[0].source_name, "SCHOTT Karriere");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://join.schott.com/sitemap.xml"]
            );
        });
    }

    #[test]
    fn ashby_json_inventory_source_runs_through_search_run_with_system_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_id = create_builtin_ashby_source(&pool).await;
            let search_request = create_search_request(&pool, vec![source_id], "photonics").await;
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
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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
            assert_eq!(posting.company, "Focused Energy Karriere");
            assert_eq!(posting.url, "https://jobs.ashbyhq.com/focused/abc");
            assert_eq!(posting.locations, vec!["Darmstadt"]);
            assert_eq!(posting.sources[0].source_key, "focused_energy_careers");
            assert_eq!(posting.sources[0].source_name, "Focused Energy Karriere");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true"]
            );
        });
    }

    #[test]
    fn json_inventory_reports_profile_author_error_when_items_path_is_not_array() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_id = create_inventory_source(
                &pool,
                DECLARATIVE_HTTP_ADAPTER_KEY,
                json_jobs_inventory("{{sourceConfig:startUrl}}"),
                json!({ "startUrl": "https://example.com/jobs.json" }),
                "Focused Energy Karriere",
            )
            .await;
            let search_request = create_search_request(&pool, vec![source_id], "photonics").await;
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
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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
                "definition.inventory.items.select.jsonPath `$.jobs` must resolve to an array"
            ));
            assert!(error.contains("resolved to object"));
        });
    }

    #[test]
    fn json_inventory_rejects_wildcards_to_document_simple_dot_jsonpath_scope() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let mut inventory = json_jobs_inventory("{{sourceConfig:startUrl}}");
            inventory["items"]["select"]["jsonPath"] = json!("$.jobs[*]");
            let source_id = create_inventory_source(
                &pool,
                DECLARATIVE_HTTP_ADAPTER_KEY,
                inventory,
                json!({ "startUrl": "https://example.com/jobs.json" }),
                "Focused Energy Karriere",
            )
            .await;
            let search_request = create_search_request(&pool, vec![source_id], "photonics").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://example.com/jobs.json",
                Ok(r#"{ "jobs": [] }"#),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::Failed);
            let error = result.source_runs[0].error.as_deref().unwrap();
            assert!(error.contains(
                "definition.inventory.items.select.jsonPath `$.jobs[*]` is not supported"
            ));
            assert!(error.contains("simple dot JSONPath only"));
            assert!(error.contains("filters and wildcards are not supported"));
        });
    }

    #[test]
    fn xml_inventory_fetch_errors_become_source_run_errors() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_id = create_inventory_source(
                &pool,
                DECLARATIVE_SITEMAP_ADAPTER_KEY,
                xml_loc_inventory(),
                json!({ "url": "https://broken.example/sitemap.xml" }),
                "Broken Careers",
            )
            .await;
            let search_request = create_search_request(&pool, vec![source_id], "engineer").await;
            let fixture_client = FixtureInventoryHttpClient::new([(
                "https://broken.example/sitemap.xml",
                Err("connection refused"),
            )]);
            let executor = DeclarativeInventoryExecutor::new(fixture_client);
            let temp_dir = tempfile::tempdir().unwrap();
            let running_search_runs = RunningSearchRuns::default();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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
                        system_profile: None,
                    })
                    .await
                    .unwrap_err();

                match error {
                    SourceExecutionError::Failed(message) => {
                        assert!(message.contains("requires an active SystemProfile"));
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
                "company": { "template": "{{sourceName|stripCareerSuffix}}" },
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

    async fn create_inventory_source(
        pool: &SqlitePool,
        adapter_key: &str,
        inventory: Value,
        source_config: Value,
        source_name: &str,
    ) -> i64 {
        let profile = create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: format!("{}_profile", adapter_key),
                name: format!("{source_name} Profil"),
                description: None,
                adapter_key: adapter_key.to_string(),
                definition_schema_version: 1,
                definition: json!({
                    "detect": { "required": [{ "htmlContains": "fixture" }] },
                    "inventory": inventory
                }),
                source_config_schema: inventory_source_config_schema(adapter_key),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        create_source(
            pool,
            CreateSourceInput {
                key: format!("{}_source", adapter_key),
                adapter_key: adapter_key.to_string(),
                system_profile_id: Some(profile.id),
                browser_profile_id: None,
                name: source_name.to_string(),
                description: None,
                source_config,
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_builtin_successfactors_source(pool: &SqlitePool) -> i64 {
        let document: Value = serde_json::from_str(include_str!(
            "../../system-profiles/builtin/successfactors.json"
        ))
        .unwrap();
        assert!(
            document.pointer("/definition/inventory").is_some(),
            "SuccessFactors built-in profile must define definition.inventory"
        );

        let profile = create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: document["key"].as_str().unwrap().to_string(),
                name: document["name"].as_str().unwrap().to_string(),
                description: document["description"].as_str().map(str::to_string),
                adapter_key: document["adapterKey"].as_str().unwrap().to_string(),
                definition_schema_version: document["definitionSchemaVersion"].as_i64().unwrap(),
                definition: document["definition"].clone(),
                source_config_schema: document["sourceConfigSchema"].clone(),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        create_source(
            pool,
            CreateSourceInput {
                key: "schott_careers".to_string(),
                adapter_key: DECLARATIVE_SITEMAP_ADAPTER_KEY.to_string(),
                system_profile_id: Some(profile.id),
                browser_profile_id: None,
                name: "SCHOTT Karriere".to_string(),
                description: None,
                source_config: json!({
                    "url": "https://join.schott.com/sitemap.xml",
                    "recursive": false
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_builtin_ashby_source(pool: &SqlitePool) -> i64 {
        let document: Value =
            serde_json::from_str(include_str!("../../system-profiles/builtin/ashby.json")).unwrap();
        assert!(
            document.pointer("/definition/inventory").is_some(),
            "Ashby built-in profile must define definition.inventory"
        );

        let profile = create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: document["key"].as_str().unwrap().to_string(),
                name: document["name"].as_str().unwrap().to_string(),
                description: document["description"].as_str().map(str::to_string),
                adapter_key: document["adapterKey"].as_str().unwrap().to_string(),
                definition_schema_version: document["definitionSchemaVersion"].as_i64().unwrap(),
                definition: document["definition"].clone(),
                source_config_schema: document["sourceConfigSchema"].clone(),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        create_source(
            pool,
            CreateSourceInput {
                key: "focused_energy_careers".to_string(),
                adapter_key: DECLARATIVE_HTTP_ADAPTER_KEY.to_string(),
                system_profile_id: Some(profile.id),
                browser_profile_id: None,
                name: "Focused Energy Karriere".to_string(),
                description: None,
                source_config: json!({
                    "startUrl": "https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true",
                    "companyWebsite": "https://focused-energy.co"
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_search_request(
        pool: &SqlitePool,
        source_ids: Vec<i64>,
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
                source_ids,
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
            source_ids: vec![1],
            validation_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn source(adapter_key: &str) -> Source {
        Source {
            id: 1,
            key: "fixture_source".to_string(),
            adapter_key: adapter_key.to_string(),
            system_profile_id: Some(1),
            browser_profile_id: None,
            name: "Fixture Careers".to_string(),
            description: None,
            source_config: json!({}),
            status: SourceStatus::Active,
            validation_error: None,
            built_in: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
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
