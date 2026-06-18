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
use std::{future::Future, path::PathBuf, pin::Pin};

use crate::{
    browser_runtime::BrowserRuntimePageWait,
    declarative_template::{render_template, TemplateContext, TemplateError},
    search_request_model::{SearchRequest, SearchRuleKind, SearchRuleTarget},
    search_run::normalization::collapse_whitespace,
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutionSource, SourceExecutor,
    },
    source_registry::BrowserInteraction,
};

const ADAPTER_KEY: &str = "declarative_browser_inventory";
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 15_000;

pub(crate) struct DeclarativeBrowserInventoryExecutor<B = ManagedBrowserInventoryClient> {
    browser: B,
}

impl DeclarativeBrowserInventoryExecutor<ManagedBrowserInventoryClient> {
    pub(crate) fn new_managed(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser: ManagedBrowserInventoryClient {
                runtime_dir: browser_runtime_dir.into(),
            },
        }
    }
}

impl<B> DeclarativeBrowserInventoryExecutor<B> {
    #[cfg(test)]
    fn new(browser: B) -> Self {
        Self { browser }
    }
}

impl<B> SourceExecutor for DeclarativeBrowserInventoryExecutor<B>
where
    B: BrowserInventoryClient + Send + Sync,
{
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(input).await })
    }
}

impl<B> DeclarativeBrowserInventoryExecutor<B>
where
    B: BrowserInventoryClient + Send + Sync,
{
    async fn execute_source(
        &self,
        input: SourceExecutionInput<'_>,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        let source = input.source;
        if source.adapter_key != ADAPTER_KEY {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by {ADAPTER_KEY}",
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
        validate_allowed_keys(
            inventory,
            &["items", "fields"],
            &plan_path(source, "executionPlan.inventory"),
        )?;

        let query_url = render_query_url(&input)?;
        let navigate_url = match query_url {
            Some(query_url) => query_url,
            None => source_config_start_url(source)?,
        };
        let page_url = parse_http_url(
            &navigate_url,
            &plan_path(source, "executionPlan.navigate.url"),
        )?;

        let wait_for = parse_wait_for(source)?;
        let rendered_html = self
            .browser
            .render_html(page_url.clone(), wait_for.clone())
            .await
            .map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not render browser inventory {} for source {}: {error}",
                    page_url.as_str(),
                    source.key
                ))
            })?;

        extract_candidates(source, &rendered_html, &page_url)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrowserInventoryWait {
    pub selector: String,
    pub timeout_ms: u64,
}

type BoxedBrowserInventoryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait BrowserInventoryClient {
    fn render_html(
        &self,
        url: Url,
        wait_for: Option<BrowserInventoryWait>,
    ) -> BoxedBrowserInventoryFuture<'_>;
}

pub(crate) struct ManagedBrowserInventoryClient {
    runtime_dir: PathBuf,
}

impl BrowserInventoryClient for ManagedBrowserInventoryClient {
    fn render_html(
        &self,
        url: Url,
        wait_for: Option<BrowserInventoryWait>,
    ) -> BoxedBrowserInventoryFuture<'_> {
        Box::pin(async move {
            let spec = crate::browser_runtime::current_runtime_spec();
            let status = crate::browser_runtime::status_for_runtime_dir(
                &self.runtime_dir,
                spec.as_ref(),
                false,
            );
            if status.status != crate::browser_runtime::BrowserRuntimeState::Installed {
                let status_detail = status
                    .error
                    .as_deref()
                    .unwrap_or("managed browser runtime is not installed and ready");
                return Err(format!(
                    "browser runtime unavailable: status {:?}: {status_detail}",
                    status.status
                ));
            }

            let executable_path = status.executable_path.as_deref().ok_or_else(|| {
                "browser runtime unavailable: installed managed browser runtime has no executable path".to_string()
            })?;
            let executable_path = PathBuf::from(executable_path);
            let runtime_wait = wait_for.as_ref().map(|wait_for| BrowserRuntimePageWait {
                selector: wait_for.selector.clone(),
                timeout_ms: wait_for.timeout_ms,
            });

            crate::browser_runtime::render_page_html_with_wait(
                &executable_path,
                &self.runtime_dir,
                url.as_str(),
                runtime_wait.as_ref(),
            )
            .await
        })
    }
}

fn extract_candidates(
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

fn render_required_field(
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

fn render_locations(
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

fn render_field_values(
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

fn selector_text_values(
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

fn selector_attribute_values(
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

fn parse_wait_for(
    source: &SourceExecutionSource,
) -> Result<Option<BrowserInventoryWait>, SourceExecutionError> {
    let Some(interactions) = source.interactions() else {
        return Ok(None);
    };

    for (index, interaction) in interactions.iter().enumerate() {
        match interaction {
            BrowserInteraction::WaitFor {
                selector,
                timeout_ms,
            } => {
                let path = plan_path(source, &format!("executionPlan.interactions[{index}]"));
                compile_selector(selector, &format!("{path}.selector"))?;
                let timeout_ms = timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
                if timeout_ms == 0 {
                    return Err(SourceExecutionError::Failed(format!(
                        "{path}.timeoutMs must be a positive integer"
                    )));
                }

                return Ok(Some(BrowserInventoryWait {
                    selector: selector.to_string(),
                    timeout_ms,
                }));
            }
            BrowserInteraction::ClickIfVisible { .. } | BrowserInteraction::ClickUpToN { .. } => {
                return Err(SourceExecutionError::Failed(format!(
                    "{} is not supported by the browser inventory executor yet",
                    plan_path(source, &format!("executionPlan.interactions[{index}]"))
                )));
            }
        }
    }

    Ok(None)
}

fn render_query_url(
    input: &SourceExecutionInput<'_>,
) -> Result<Option<String>, SourceExecutionError> {
    let Some(query_value) = input.source.query() else {
        return Ok(None);
    };

    let query_path = plan_path(input.source, "executionPlan.query");
    let query = query_value.as_object().ok_or_else(|| {
        SourceExecutionError::Failed(format!("{query_path} must be a JSON object"))
    })?;
    validate_allowed_keys(query, &["baseUrl", "path", "params"], &query_path)?;

    let base_url_value = query
        .get("baseUrl")
        .ok_or_else(|| SourceExecutionError::Failed(format!("{query_path}.baseUrl is required")))?;
    let base_url = render_query_base_url(input, base_url_value, &format!("{query_path}.baseUrl"))?;
    let base_url = parse_http_url(&base_url, &format!("{query_path}.baseUrl"))?;

    let path = required_string(query, "path", &format!("{query_path}.path"))?;
    if !path.starts_with('/') || path.starts_with("//") || path.contains('\\') {
        return Err(SourceExecutionError::Failed(format!(
            "{query_path}.path must be an absolute path starting with one / and without a URL authority or backslashes"
        )));
    }

    let mut url = base_url.join(path).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "{query_path}.path could not be used to build URL: {error}"
        ))
    })?;

    let params_path = format!("{query_path}.params");
    let params = query
        .get("params")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{params_path} must be an array")))?;
    let template_context = BrowserInventoryTemplateContext {
        source: input.source,
        search_request: input.search_request,
        query_url: None,
    };
    {
        let mut query_pairs = url.query_pairs_mut();
        for (index, param) in params.iter().enumerate() {
            let param_path = format!("{params_path}[{index}]");
            let param = param.as_object().ok_or_else(|| {
                SourceExecutionError::Failed(format!("{param_path} must be a JSON object"))
            })?;
            validate_allowed_keys(param, &["name", "value"], &param_path)?;
            let param_name = required_string(param, "name", &format!("{param_path}.name"))?;
            let template = required_string(param, "value", &format!("{param_path}.value"))?;
            let value = render_template(template, &template_context).map_err(|error| {
                SourceExecutionError::Failed(format!("{param_path}.value is invalid: {error}"))
            })?;
            let value = value.trim();
            if !value.is_empty() {
                query_pairs.append_pair(param_name, value);
            }
        }
    }

    Ok(Some(url.to_string()))
}

fn render_query_base_url(
    input: &SourceExecutionInput<'_>,
    value: &Value,
    path: &str,
) -> Result<String, SourceExecutionError> {
    match value {
        Value::String(template) => {
            let template_context = BrowserInventoryTemplateContext {
                source: input.source,
                search_request: input.search_request,
                query_url: None,
            };
            let rendered = render_template(template, &template_context).map_err(|error| {
                SourceExecutionError::Failed(format!("{path} is invalid: {error}"))
            })?;
            let rendered = rendered.trim();
            if rendered.is_empty() {
                Err(SourceExecutionError::Failed(format!(
                    "{path} must render a non-empty URL"
                )))
            } else {
                Ok(rendered.to_string())
            }
        }
        Value::Object(object) => {
            validate_allowed_keys(object, &["sourceConfigKey", "default"], path)?;
            let source_config_key = required_string(
                object,
                "sourceConfigKey",
                &format!("{path}.sourceConfigKey"),
            )?;
            let default = required_string(object, "default", &format!("{path}.default"))?;
            let source_config = input.source.source_config.as_object().ok_or_else(|| {
                SourceExecutionError::Failed(format!(
                    "sourceConfig is invalid for source {}: expected a JSON object",
                    input.source.key
                ))
            })?;
            match source_config.get(source_config_key) {
                Some(Value::String(base_url)) => {
                    let base_url = base_url.trim();
                    if base_url.is_empty() {
                        Err(SourceExecutionError::Failed(format!(
                            "sourceConfig.{source_config_key} must not be empty"
                        )))
                    } else {
                        Ok(base_url.to_string())
                    }
                }
                Some(Value::Null) | None => Ok(default.to_string()),
                Some(_) => Err(SourceExecutionError::Failed(format!(
                    "sourceConfig.{source_config_key} must be a string"
                ))),
            }
        }
        _ => Err(SourceExecutionError::Failed(format!(
            "{path} must be either a string template or a JSON object"
        ))),
    }
}

fn resolve_search_request_variable(
    search_request: &SearchRequest,
    key: &str,
) -> Result<Option<String>, TemplateError> {
    match key {
        "titleText" => Ok(Some(search_request_title_text(search_request))),
        "firstLocation" => Ok(Some(
            first_search_request_location(search_request).unwrap_or_default(),
        )),
        "radiusKm" => Ok(Some(
            search_request
                .radius_km
                .map(|radius_km| radius_km.to_string())
                .unwrap_or_default(),
        )),
        "" => Err(TemplateError::Invalid(
            "searchRequest template variable must include a key".to_string(),
        )),
        _ => Err(TemplateError::Invalid(format!(
            "unsupported searchRequest template variable `{key}`"
        ))),
    }
}

fn search_request_title_text(search_request: &SearchRequest) -> String {
    search_request
        .include_rules
        .iter()
        .filter(|rule| rule.target == SearchRuleTarget::Title && rule.kind == SearchRuleKind::Text)
        .map(|rule| collapse_whitespace(&rule.value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_search_request_location(search_request: &SearchRequest) -> Option<String> {
    search_request
        .locations
        .iter()
        .map(|location| collapse_whitespace(location))
        .find(|location| !location.is_empty())
}

struct BrowserInventoryTemplateContext<'a> {
    source: &'a SourceExecutionSource,
    search_request: &'a SearchRequest,
    query_url: Option<&'a str>,
}

impl TemplateContext for BrowserInventoryTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "sourceName" {
            Ok(Some(self.source.name.clone()))
        } else if variable == "sourceKey" {
            Ok(Some(self.source.key.clone()))
        } else if variable == "query:url" {
            self.query_url
                .map(|url| Some(url.to_string()))
                .ok_or_else(|| TemplateError::Invalid("query.url is not available".to_string()))
        } else if let Some(search_request_key) = variable.strip_prefix("searchRequest:") {
            resolve_search_request_variable(self.search_request, search_request_key)
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

fn source_config_start_url(source: &SourceExecutionSource) -> Result<String, SourceExecutionError> {
    let source_config = source.source_config.as_object().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "sourceConfig is invalid for source {}: expected a JSON object",
            source.key
        ))
    })?;
    let start_url = source_config
        .get("startUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "source {} requires sourceConfig.startUrl when executionPlan.query is absent",
                source.key
            ))
        })?;
    if start_url.trim().is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "source {} sourceConfig.startUrl must be a non-empty string",
            source.key
        )));
    }
    Ok(start_url.to_string())
}

fn resolve_http_candidate_url(raw_url: &str, page_url: &Url) -> Option<String> {
    let raw_url = raw_url.trim();
    if raw_url.is_empty() {
        return None;
    }
    let url = page_url.join(raw_url).ok()?;
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

fn compile_selector(selector: &str, path: &str) -> Result<Matcher, SourceExecutionError> {
    Matcher::new(selector).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "{path} must be a valid CSS selector for the browser inventory language: {error:?}"
        ))
    })
}

fn required_object_value<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, SourceExecutionError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
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

fn validate_allowed_keys(
    object: &Map<String, Value>,
    allowed_keys: &[&str],
    path: &str,
) -> Result<(), SourceExecutionError> {
    for key in object.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(SourceExecutionError::Failed(format!(
                "{path}.{key} is not supported by the browser inventory language"
            )));
        }
    }
    Ok(())
}

fn plan_path(source: &SourceExecutionSource, path: &str) -> String {
    format!("source {} {path}", source.key)
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
            create_browser_profile, create_source, CreateBrowserProfileInput, CreateSourceInput,
            Source, SourceStatus,
        },
        source_registry::{BrowserInteraction, ResolvedSelectedAccessPath},
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::{collections::HashMap, sync::Mutex};

    struct FixtureBrowserInventoryClient {
        responses: HashMap<String, Result<String, String>>,
        rendered_requests: Mutex<Vec<(String, Option<BrowserInventoryWait>)>>,
    }

    impl FixtureBrowserInventoryClient {
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
                rendered_requests: Mutex::new(Vec::new()),
            }
        }

        fn rendered_requests(&self) -> Vec<(String, Option<BrowserInventoryWait>)> {
            self.rendered_requests.lock().unwrap().clone()
        }
    }

    impl BrowserInventoryClient for FixtureBrowserInventoryClient {
        fn render_html(
            &self,
            url: Url,
            wait_for: Option<BrowserInventoryWait>,
        ) -> BoxedBrowserInventoryFuture<'_> {
            Box::pin(async move {
                self.rendered_requests
                    .lock()
                    .unwrap()
                    .push((url.as_str().to_string(), wait_for));
                self.responses
                    .get(url.as_str())
                    .cloned()
                    .unwrap_or_else(|| Err(format!("{} not found", url.as_str())))
            })
        }
    }

    #[test]
    #[ignore = "DB-owned source/profile tables were removed by #38; registry-backed flow follows in #39-#41"]
    fn browser_inventory_source_runs_through_search_run_with_browser_profile() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile_id =
                create_browser_inventory_profile(&pool, browser_inventory_definition()).await;
            let source = create_browser_inventory_source(
                &pool,
                browser_profile_id,
                json!({
                    "startUrl": "https://example.test/jobs"
                }),
            )
            .await;
            let search_request =
                create_search_request(&pool, vec![source.id], "Senior Laser Engineer").await;
            let fixture_browser = FixtureBrowserInventoryClient::new([(
                "https://example.test/jobs",
                Ok(r#"
                <html><body>
                  <article data-job-card>
                    <a href="/jobs/laser">
                      <span data-job-title>  Senior
 Laser   Engineer  </span>
                    </a>
                    <span data-company>	ACME   GmbH
</span>
                    <span data-location> Mainz </span>
                    <span data-location>mainz</span>
                  </article>
                  <article data-job-card>
                    <a href="https://example.test/jobs/chemist">
                      <span data-job-title>Chemist</span>
                    </a>
                    <span data-company>ACME GmbH</span>
                    <span data-location>Berlin</span>
                  </article>
                </body></html>
                "#),
            )]);
            let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
            let temp_dir = tempfile::tempdir().unwrap();
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
            assert_eq!(result.source_runs[0].candidate_count, 2);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Senior Laser Engineer");
            assert_eq!(posting.company, "ACME GmbH");
            assert_eq!(posting.url, "https://example.test/jobs/laser");
            assert_eq!(posting.locations, vec!["Mainz"]);
            assert_eq!(posting.sources[0].source_key, "browser_inventory_fixture");
            assert_eq!(
                executor.browser.rendered_requests(),
                vec![(
                    "https://example.test/jobs".to_string(),
                    Some(BrowserInventoryWait {
                        selector: "[data-job-card]".to_string(),
                        timeout_ms: 15_000,
                    })
                )]
            );
        });
    }

    #[test]
    #[ignore = "DB-owned source/profile tables were removed by #38; registry-backed flow follows in #39-#41"]
    fn stepstone_browser_profile_builds_query_url_and_extracts_cards_through_search_run() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile_id = create_stepstone_browser_inventory_profile(&pool).await;
            let source = create_stepstone_browser_inventory_source(
                &pool,
                browser_profile_id,
                json!({
                    "baseUrl": "https://stepstone.example"
                }),
            )
            .await;
            let search_request = SearchRequestService::new(&pool, &RunningSearchRuns::default())
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![
                        text_rule("Rust Engineer"),
                        regex_rule("Senior\\s+Developer"),
                        text_rule(" Data "),
                    ],
                    exclude_rules: vec![],
                    locations: vec![" Berlin ".to_string(), "München".to_string()],
                    radius_km: Some(50),
                    source_keys: vec![source.key.clone()],
                })
                .await
                .unwrap();
            let fixture_browser = FixtureBrowserInventoryClient::new([(
                "https://stepstone.example/jobs?what=Rust+Engineer+Data&where=Berlin&radius=50",
                Ok(r#"
                <html><body>
                  <article data-at="job-item">
                    <a data-at="job-item-title" href="/stellenangebote--Rust-Engineer-Berlin-ACME--123.html">
                        Rust
                        Engineer
                    </a>
                    <span data-at="job-item-company-name"> ACME   GmbH </span>
                    <span data-at="job-item-location"> Berlin </span>
                    <span data-at="job-item-location">berlin</span>
                  </article>
                  <article data-at="job-item">
                    <a data-at="job-item-title" href="/stellenangebote--Chemist-Berlin-ACME--456.html">
                        Chemist
                    </a>
                    <span data-at="job-item-company-name">ACME GmbH</span>
                    <span data-at="job-item-location">Berlin</span>
                  </article>
                </body></html>
                "#),
            )]);
            let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
            let temp_dir = tempfile::tempdir().unwrap();
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
            assert_eq!(result.source_runs[0].source_key, "stepstone_de");
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
            assert_eq!(result.source_runs[0].candidate_count, 2);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(posting.title, "Rust Engineer");
            assert_eq!(posting.company, "ACME GmbH");
            assert_eq!(
                posting.url,
                "https://stepstone.example/stellenangebote--Rust-Engineer-Berlin-ACME--123.html"
            );
            assert_eq!(posting.locations, vec!["Berlin"]);
            assert_eq!(posting.sources[0].source_key, "stepstone_de");
            assert_eq!(
                executor.browser.rendered_requests(),
                vec![(
                    "https://stepstone.example/jobs?what=Rust+Engineer+Data&where=Berlin&radius=50"
                        .to_string(),
                    Some(BrowserInventoryWait {
                        selector: "article[data-at=\"job-item\"]".to_string(),
                        timeout_ms: 15_000,
                    })
                )]
            );
        });
    }

    #[test]
    fn browser_inventory_executes_from_resolved_execution_plan_without_browser_profile() {
        tauri::async_runtime::block_on(async {
            let fixture_browser = FixtureBrowserInventoryClient::new([(
                "https://example.test/jobs?what=Engineer",
                Ok(r#"
                <html><body>
                  <article data-job-card>
                    <a href="/jobs/laser">
                      <span data-job-title>Laser Engineer</span>
                    </a>
                    <span data-company>ACME GmbH</span>
                    <span data-location>Mainz</span>
                  </article>
                </body></html>
                "#),
            )]);
            let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
            let search_request = search_request();
            let source = source_with_browser_plan(
                json!({ "baseUrl": "https://example.test" }),
                Some(json!({
                    "baseUrl": {
                        "sourceConfigKey": "baseUrl",
                        "default": "https://www.example.test"
                    },
                    "path": "/jobs",
                    "params": [
                        { "name": "what", "value": "{{searchRequest:titleText}}" }
                    ]
                })),
                browser_inventory_without_navigate_definition(),
                Some(vec![BrowserInteraction::WaitFor {
                    selector: "[data-job-card]".to_string(),
                    timeout_ms: Some(15_000),
                }]),
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
                    company: "ACME GmbH".to_string(),
                    url: "https://example.test/jobs/laser".to_string(),
                    locations: vec!["Mainz".to_string()],
                }]
            );
            assert_eq!(
                executor.browser.rendered_requests(),
                vec![(
                    "https://example.test/jobs?what=Engineer".to_string(),
                    Some(BrowserInventoryWait {
                        selector: "[data-job-card]".to_string(),
                        timeout_ms: 15_000,
                    })
                )]
            );
        });
    }

    #[test]
    fn adapter_requires_inventory_plan_before_rendering() {
        tauri::async_runtime::block_on(async {
            let executor =
                DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
            let search_request = search_request();
            let source =
                source_without_inventory(json!({ "startUrl": "https://example.test/jobs" }));

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .expect_err("browser inventory must require a resolved inventory plan");

            assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "executionPlan.inventory must be a JSON object for source browser_inventory_fixture"
                        .to_string()
                )
            );
            assert!(executor.browser.rendered_requests().is_empty());
        });
    }

    #[test]
    fn source_without_query_requires_start_url_before_rendering() {
        tauri::async_runtime::block_on(async {
            let executor =
                DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
            let search_request = search_request();
            let source = source_with_browser_plan(
                json!({}),
                None,
                browser_inventory_without_navigate_definition(),
                None,
            );

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .expect_err("source-specific browser inventory without query must need startUrl");

            assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "source browser_inventory_fixture requires sourceConfig.startUrl when executionPlan.query is absent"
                        .to_string()
                )
            );
            assert!(executor.browser.rendered_requests().is_empty());
        });
    }

    #[test]
    #[ignore = "DB-owned source/profile tables were removed by #38; registry-backed flow follows in #39-#41"]
    fn missing_inventory_definition_becomes_failed_source_run() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile_id = create_browser_inventory_profile(&pool, json!({})).await;
            let source = create_browser_inventory_source(
                &pool,
                browser_profile_id,
                json!({
                    "startUrl": "https://example.test/jobs"
                }),
            )
            .await;
            let search_request = create_search_request(&pool, vec![source.id], "Engineer").await;
            let executor =
                DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
            let temp_dir = tempfile::tempdir().unwrap();
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
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            assert_eq!(
                result.source_runs[0].error.as_deref(),
                Some("Browserprofil browser_inventory_profile definition.inventory must be a JSON object")
            );
            assert!(result.postings.is_empty());
            assert!(executor.browser.rendered_requests().is_empty());
        });
    }

    #[test]
    fn default_source_executor_routes_browser_inventory_adapter() {
        tauri::async_runtime::block_on(async {
            let executor = DefaultSourceExecutor::new(
                tempfile::tempdir().unwrap().path().join("browser-runtime"),
            );
            let search_request = search_request();
            let source =
                source_without_inventory(json!({ "startUrl": "https://example.test/jobs" }));

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .expect_err(
                    "missing execution-plan inventory should fail before managed browser runtime access",
                );

            match error {
                SourceExecutionError::Failed(message) => {
                    assert!(message.contains("executionPlan.inventory"));
                    assert!(!message.contains("has no search-run executor yet"));
                }
                SourceExecutionError::Cancelled(message) => {
                    panic!("expected failed source execution, got cancellation: {message}")
                }
            }
        });
    }

    async fn create_stepstone_browser_inventory_profile(pool: &SqlitePool) -> i64 {
        create_browser_profile(
            pool,
            CreateBrowserProfileInput {
                key: "stepstone_de_browser_profile".to_string(),
                name: "StepStone Deutschland Browserprofil".to_string(),
                description: None,
                name_i18n_key: None,
                description_i18n_key: None,
                definition_path: None,
                definition_hash: None,
                definition_schema_version: 1,
                definition: stepstone_browser_inventory_definition(),
                source_config_schema: json!({
                    "type": "object",
                    "properties": {
                        "baseUrl": { "type": "string", "format": "uri" },
                        "manualReleaseStartUrl": { "type": "string", "format": "uri" },
                        "maxPages": { "type": "number", "minimum": 1, "default": 1 }
                    }
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_stepstone_browser_inventory_source(
        pool: &SqlitePool,
        browser_profile_id: i64,
        source_config: Value,
    ) -> Source {
        create_source(
            pool,
            CreateSourceInput {
                key: "stepstone_de".to_string(),
                adapter_key: ADAPTER_KEY.to_string(),
                system_profile_id: None,
                browser_profile_id: Some(browser_profile_id),
                name: "StepStone Deutschland".to_string(),
                description: None,
                source_config,
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
    }

    async fn create_browser_inventory_profile(pool: &SqlitePool, definition: Value) -> i64 {
        create_browser_profile(
            pool,
            CreateBrowserProfileInput {
                key: "browser_inventory_profile".to_string(),
                name: "Browser Inventory Profile".to_string(),
                description: None,
                name_i18n_key: None,
                description_i18n_key: None,
                definition_path: None,
                definition_hash: None,
                definition_schema_version: 1,
                definition,
                source_config_schema: json!({
                    "type": "object",
                    "required": ["startUrl"],
                    "properties": {
                        "startUrl": { "type": "string", "format": "uri" }
                    }
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_browser_inventory_source(
        pool: &SqlitePool,
        browser_profile_id: i64,
        source_config: Value,
    ) -> Source {
        create_source(
            pool,
            CreateSourceInput {
                key: "browser_inventory_fixture".to_string(),
                adapter_key: ADAPTER_KEY.to_string(),
                system_profile_id: None,
                browser_profile_id: Some(browser_profile_id),
                name: "Browser Inventory Fixture".to_string(),
                description: None,
                source_config,
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
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
                source_keys: source_ids.into_iter().map(|id| id.to_string()).collect(),
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

    fn regex_rule(value: &str) -> SearchRuleInput {
        SearchRuleInput {
            target: "title".to_string(),
            kind: "regex".to_string(),
            value: value.to_string(),
        }
    }

    fn stepstone_browser_inventory_definition() -> Value {
        json!({
            "schemaVersion": 1,
            "query": {
                "baseUrl": {
                    "sourceConfigKey": "baseUrl",
                    "default": "https://www.stepstone.de"
                },
                "path": "/jobs",
                "params": [
                    { "name": "what", "value": "{{searchRequest:titleText}}" },
                    { "name": "where", "value": "{{searchRequest:firstLocation}}" },
                    { "name": "radius", "value": "{{searchRequest:radiusKm}}" }
                ]
            },
            "inventory": {
                "navigate": { "url": "{{query:url}}" },
                "waitFor": { "selector": "article[data-at=\"job-item\"]", "timeoutMs": 15000 },
                "items": { "select": "article[data-at=\"job-item\"]" },
                "fields": {
                    "title": { "selectorText": "[data-at=\"job-item-title\"]" },
                    "company": { "selectorText": "[data-at=\"job-item-company-name\"]" },
                    "url": {
                        "selectorAttribute": {
                            "selector": "a[data-at=\"job-item-title\"]",
                            "attribute": "href"
                        }
                    },
                    "locations": [
                        { "selectorText": "[data-at=\"job-item-location\"]" }
                    ]
                }
            }
        })
    }

    fn browser_inventory_definition() -> Value {
        json!({
            "schemaVersion": 1,
            "inventory": {
                "navigate": { "url": "{{sourceConfig:startUrl}}" },
                "waitFor": { "selector": "[data-job-card]", "timeoutMs": 15000 },
                "items": { "select": "[data-job-card]" },
                "fields": {
                    "title": { "selectorText": "[data-job-title]" },
                    "company": { "selectorText": "[data-company]" },
                    "url": {
                        "selectorAttribute": { "selector": "a", "attribute": "href" }
                    },
                    "locations": [
                        { "selectorText": "[data-location]" }
                    ]
                }
            }
        })
    }

    fn browser_inventory_without_navigate_definition() -> Value {
        json!({
            "items": { "select": "[data-job-card]" },
            "fields": {
                "title": { "selectorText": "[data-job-title]" },
                "company": { "selectorText": "[data-company]" },
                "url": {
                    "selectorAttribute": { "selector": "a", "attribute": "href" }
                },
                "locations": [
                    { "selectorText": "[data-location]" }
                ]
            }
        })
    }

    fn search_request() -> SearchRequest {
        SearchRequest {
            id: 1,
            status: SearchRequestStatus::Active,
            include_rules: vec![text_rule("Engineer")]
                .into_iter()
                .map(|rule| crate::search_request_model::SearchRule {
                    target: crate::search_request_model::SearchRuleTarget::try_from(
                        rule.target.as_str(),
                    )
                    .unwrap(),
                    kind: crate::search_request_model::SearchRuleKind::try_from(rule.kind.as_str())
                        .unwrap(),
                    value: rule.value,
                })
                .collect(),
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys: vec!["browser_inventory_fixture".to_string()],
            validation_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn source_with_browser_plan(
        source_config: Value,
        query: Option<Value>,
        inventory: Value,
        interactions: Option<Vec<BrowserInteraction>>,
    ) -> SourceExecutionSource {
        source_execution_source(source_config, query, Some(inventory), interactions)
    }

    fn source_without_inventory(source_config: Value) -> SourceExecutionSource {
        source_execution_source(source_config, None, None, None)
    }

    fn source_execution_source(
        source_config: Value,
        query: Option<Value>,
        inventory: Option<Value>,
        interactions: Option<Vec<BrowserInteraction>>,
    ) -> SourceExecutionSource {
        SourceExecutionSource {
            key: "browser_inventory_fixture".to_string(),
            adapter_key: ADAPTER_KEY.to_string(),
            name: "Browser Inventory Fixture".to_string(),
            source_config,
            effective_source_config_schema: json!({ "type": "object" }),
            selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
                query,
                inventory,
                interactions,
                manual_release: None,
            },
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
