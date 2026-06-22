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

use reqwest::Url;
use serde_json::{Map, Value};

use crate::{
    declarative::template::{render_template, TemplateContext, TemplateError},
    search::normalization::collapse_whitespace,
    search::request::{SearchRequest, SearchRuleKind, SearchRuleTarget},
    search::run::{SourceExecutionError, SourceExecutionInput, SourceExecutionSource},
};

pub(super) fn render_query_url(
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

pub(super) fn render_query_base_url(
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

pub(super) fn resolve_search_request_variable(
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

pub(super) fn search_request_title_text(search_request: &SearchRequest) -> String {
    search_request
        .include_rules
        .iter()
        .filter(|rule| rule.target == SearchRuleTarget::Title && rule.kind == SearchRuleKind::Text)
        .map(|rule| collapse_whitespace(&rule.value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn first_search_request_location(search_request: &SearchRequest) -> Option<String> {
    search_request
        .locations
        .iter()
        .map(|location| collapse_whitespace(location))
        .find(|location| !location.is_empty())
}

pub(super) struct BrowserInventoryTemplateContext<'a> {
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

pub(super) fn source_config_value_as_string(source_config: &Value, key: &str) -> Option<String> {
    let value = source_config.get(key)?;
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn source_config_start_url(
    source: &SourceExecutionSource,
) -> Result<String, SourceExecutionError> {
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

pub(super) fn resolve_http_candidate_url(raw_url: &str, page_url: &Url) -> Option<String> {
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

pub(super) fn parse_http_url(value: &str, field: &str) -> Result<Url, SourceExecutionError> {
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

pub(super) fn required_object_value<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, SourceExecutionError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))
}

pub(super) fn required_string<'a>(
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

pub(super) fn validate_allowed_keys(
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

pub(super) fn plan_path(source: &SourceExecutionSource, path: &str) -> String {
    format!("source {} {path}", source.key)
}
