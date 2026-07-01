use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::{
            extract::{Cardinality, CombinePart, FieldExpression, ListFieldExpression},
            transform::Transform,
            HttpMethod, ParseType, Select,
        },
        execution_plan::{
            capabilities::ExecutionPlanFetch,
            posting_discovery::{
                ExecutionPlanPostingDiscoveryFields, ExecutionPlanPostingDiscoveryStrategy,
            },
            SourceExecutionPlan,
        },
    },
    simple_json_path::resolve_simple_json_path,
    source::documents::SourceConfig,
};

use super::transform::{apply_transform_pipeline, normalize_whitespace};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDiscoveryExecutionResult {
    pub candidates: Vec<PostingDiscoveryCandidate>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDiscoveryCandidate {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub posting_meta: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchError {
    pub message: String,
}

impl PostingDiscoveryFetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait PostingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone, Debug)]
pub struct ReqwestPostingDiscoveryFetcher {
    client: reqwest::Client,
}

impl ReqwestPostingDiscoveryFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestPostingDiscoveryFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PostingDiscoveryFetcher for ReqwestPostingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let method = match request.method {
                HttpMethod::Get => reqwest::Method::GET,
                HttpMethod::Post => reqwest::Method::POST,
            };
            let mut builder = self
                .client
                .request(method, &request.url)
                .timeout(Duration::from_millis(request.timeout_ms));
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            let response = builder
                .send()
                .await
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?
                .error_for_status()
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?;
            Ok(PostingDiscoveryFetchResponse { body })
        })
    }
}

pub async fn execute_posting_discovery(
    plan: &SourceExecutionPlan,
) -> PostingDiscoveryExecutionResult {
    execute_posting_discovery_with_fetcher(plan, &ReqwestPostingDiscoveryFetcher::new()).await
}

pub async fn execute_posting_discovery_with_fetcher<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    let Some((strategy_index, strategy)) =
        plan.posting_discovery.strategies.iter().enumerate().next()
    else {
        return PostingDiscoveryExecutionResult {
            candidates: Vec::new(),
            diagnostics: vec![runtime_error(
                "posting_discovery_strategy_missing",
                "postingDiscovery does not contain an executable strategy",
                "/postingDiscovery/strategies",
                None,
                json!({}),
            )],
        };
    };

    execute_strategy(plan, fetcher, strategy_index, strategy).await
}

async fn execute_strategy<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    let base_path = format!("/postingDiscovery/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let response = match fetch_strategy_document(
        fetcher,
        &strategy.fetch,
        &plan.source_config,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    )
    .await
    {
        Some(response) => response,
        None => {
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            }
        }
    };

    let document = match parse_json_document(
        &response.body,
        strategy,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => {
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            }
        }
    };

    let items = match select_json_items(
        &document,
        &strategy.select,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(items) => items,
        None => {
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            }
        }
    };

    let mut candidates = Vec::new();
    for (item_index, item) in items.into_iter().enumerate() {
        if let Some(candidate) = extract_candidate(
            item,
            &strategy.extract.fields,
            &plan.source_config,
            &base_path,
            strategy_key.as_deref(),
            item_index,
            &mut diagnostics,
        ) {
            candidates.push(candidate);
        }
    }

    PostingDiscoveryExecutionResult {
        candidates,
        diagnostics,
    }
}

async fn fetch_strategy_document<F>(
    fetcher: &F,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryFetchResponse>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    let ExecutionPlanFetch::Http {
        method,
        url,
        headers,
        timeout_ms,
        ..
    } = fetch
    else {
        diagnostics.push(runtime_error(
            "unsupported_fetch_mode",
            "postingDiscovery runtime slice supports only HTTP fetch",
            format!("{base_path}/fetch"),
            strategy_key,
            json!({ "supportedMode": "http" }),
        ));
        return None;
    };

    let method = method.unwrap_or(HttpMethod::Get);
    if method != HttpMethod::Get {
        diagnostics.push(runtime_error(
            "unsupported_http_method",
            "postingDiscovery runtime slice supports only HTTP GET",
            format!("{base_path}/fetch/method"),
            strategy_key,
            json!({ "supportedMethod": "GET" }),
        ));
        return None;
    }

    let rendered_url = match render_source_config_template(url, source_config) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "fetch_url_template_failed",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let request = PostingDiscoveryFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: headers.clone().unwrap_or_default(),
        timeout_ms: *timeout_ms,
    };

    match fetcher.fetch(request).await {
        Ok(response) => Some(response),
        Err(error) => {
            diagnostics.push(runtime_error(
                "fetch_failed",
                format!(
                    "HTTP GET fetch failed for {rendered_url}: {}",
                    error.message
                ),
                format!("{base_path}/fetch"),
                strategy_key,
                json!({ "url": rendered_url, "error": error.message }),
            ));
            None
        }
    }
}

fn parse_json_document(
    body: &str,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Value> {
    if strategy.parse.parse_type != ParseType::Json {
        diagnostics.push(runtime_error(
            "unsupported_parse_type",
            "postingDiscovery runtime slice supports only JSON parse",
            format!("{base_path}/parse/type"),
            strategy_key,
            json!({ "supportedType": "json" }),
        ));
        return None;
    }

    match serde_json::from_str(body) {
        Ok(document) => Some(document),
        Err(error) => {
            diagnostics.push(runtime_error(
                "json_parse_failed",
                format!("Fetched response could not be parsed as JSON: {error}"),
                format!("{base_path}/parse"),
                strategy_key,
                json!({ "error": error.to_string() }),
            ));
            None
        }
    }
}

fn select_json_items<'a>(
    document: &'a Value,
    select: &Select,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<&'a Value>> {
    match select {
        Select::JsonPath { json_path } => {
            let selected = match resolve_simple_json_path(document, json_path) {
                Ok(selected) => selected,
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_failed",
                        format!("JSONPath selector is invalid: {error}"),
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    return None;
                }
            };

            match selected {
                Some(Value::Array(items)) => Some(items.iter().collect()),
                Some(_) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_not_array",
                        "JSONPath selector must resolve to an array for postingDiscovery",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
                None => {
                    diagnostics.push(runtime_error(
                        "json_path_select_missing",
                        "JSONPath selector did not match a posting item collection",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
            }
        }
        Select::Document => match document {
            Value::Array(items) => Some(items.iter().collect()),
            value => Some(vec![value]),
        },
        _ => {
            diagnostics.push(runtime_error(
                "unsupported_select_type",
                "postingDiscovery runtime slice supports only JSONPath selection for JSON responses",
                format!("{base_path}/select"),
                strategy_key,
                json!({}),
            ));
            None
        }
    }
}

fn extract_candidate(
    item: &Value,
    fields: &ExecutionPlanPostingDiscoveryFields,
    source_config: &SourceConfig,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDiscoveryCandidate> {
    let title = extract_required_string_field(
        item,
        source_config,
        &fields.title,
        &format!("{base_path}/extract/fields/title"),
        strategy_key,
        item_index,
        diagnostics,
    );
    let company = extract_required_string_field(
        item,
        source_config,
        &fields.company,
        &format!("{base_path}/extract/fields/company"),
        strategy_key,
        item_index,
        diagnostics,
    );
    let url = extract_required_string_field(
        item,
        source_config,
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

fn extract_required_string_field(
    item: &Value,
    source_config: &SourceConfig,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    match evaluate_string_field(
        item,
        source_config,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldEvaluation {
    value: Option<String>,
    failed: bool,
}

fn evaluate_string_field(
    item: &Value,
    source_config: &SourceConfig,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let RawFieldValues {
        values,
        failed,
        cardinality,
        transforms,
    } = raw_field_values(
        item,
        source_config,
        expression,
        path,
        strategy_key,
        item_index,
        diagnostics,
    );
    if failed {
        return FieldEvaluation {
            value: None,
            failed: true,
        };
    }

    let values = match apply_transforms(
        values,
        transforms,
        path,
        strategy_key,
        item_index,
        diagnostics,
    ) {
        Some(values) => values,
        None => {
            return FieldEvaluation {
                value: None,
                failed: true,
            };
        }
    };

    let mut normalized_values = Vec::new();
    for value in values {
        let value = normalize_whitespace(value.trim());
        if !value.is_empty() {
            normalized_values.push(value);
        }
    }

    match cardinality.unwrap_or(Cardinality::One) {
        Cardinality::One => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => {
                cardinality_mismatch(path, strategy_key, item_index, count, "one", diagnostics)
            }
        },
        Cardinality::First => {
            if let Some(value) = normalized_values.into_iter().next() {
                FieldEvaluation {
                    value: Some(value),
                    failed: false,
                }
            } else {
                FieldEvaluation {
                    value: None,
                    failed: false,
                }
            }
        }
        Cardinality::Optional => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => cardinality_mismatch(
                path,
                strategy_key,
                item_index,
                count,
                "optional",
                diagnostics,
            ),
        },
        Cardinality::All => FieldEvaluation {
            value: normalized_values.into_iter().next(),
            failed: false,
        },
    }
}

fn cardinality_mismatch(
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    actual_count: usize,
    expected: &str,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    diagnostics.push(runtime_error(
        "field_cardinality_mismatch",
        format!("Field cardinality `{expected}` did not match {actual_count} resolved values"),
        path,
        strategy_key,
        json!({
            "itemIndex": item_index,
            "expectedCardinality": expected,
            "actualCount": actual_count,
        }),
    ));
    FieldEvaluation {
        value: None,
        failed: true,
    }
}

struct RawFieldValues<'a> {
    values: Vec<String>,
    failed: bool,
    cardinality: Option<Cardinality>,
    transforms: Option<&'a Vec<Transform>>,
}

fn raw_field_values<'a>(
    item: &Value,
    source_config: &SourceConfig,
    expression: &'a FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    match expression {
        FieldExpression::Const {
            value,
            cardinality,
            transforms,
        } => json_value_to_strings(value, path, strategy_key, item_index, diagnostics)
            .into_raw(*cardinality, transforms.as_ref()),
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => match resolve_simple_json_path(item, json_path) {
            Ok(Some(value)) => {
                json_value_to_strings(value, path, strategy_key, item_index, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            Ok(None) => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            Err(error) => {
                diagnostics.push(runtime_error(
                    "field_json_path_failed",
                    format!("Field JSONPath is invalid: {error}"),
                    path,
                    strategy_key,
                    json!({
                        "itemIndex": item_index,
                        "jsonPath": json_path,
                        "error": error.to_string(),
                    }),
                ));
                RawFieldValues {
                    values: Vec::new(),
                    failed: true,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                }
            }
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => match source_config.get(key) {
            Some(value) => {
                json_value_to_strings(value, path, strategy_key, item_index, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            None => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => match item.get(key) {
            Some(value) => {
                json_value_to_strings(value, path, strategy_key, item_index, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            None => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::Template {
            template,
            cardinality,
            transforms,
        } => match render_source_config_template(template, source_config) {
            Ok(value) => RawFieldValues {
                values: vec![value],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            Err(message) => {
                diagnostics.push(runtime_error(
                    "field_template_failed",
                    format!("Field template could not be rendered: {message}"),
                    path,
                    strategy_key,
                    json!({ "itemIndex": item_index, "template": template }),
                ));
                RawFieldValues {
                    values: Vec::new(),
                    failed: true,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                }
            }
        },
        FieldExpression::Combine {
            parts,
            join,
            cardinality,
            transforms,
        } => combine_field_values(
            item,
            source_config,
            parts,
            join.as_deref().unwrap_or_default(),
            path,
            strategy_key,
            item_index,
            diagnostics,
        )
        .into_raw(*cardinality, transforms.as_ref()),
        _ => {
            diagnostics.push(runtime_error(
                "unsupported_field_expression",
                "postingDiscovery runtime slice supports const, template, sourceConfig, itemField, JSONPath, and combine field expressions",
                path,
                strategy_key,
                json!({ "itemIndex": item_index }),
            ));
            RawFieldValues {
                values: Vec::new(),
                failed: true,
                cardinality: None,
                transforms: None,
            }
        }
    }
}

fn combine_field_values(
    item: &Value,
    source_config: &SourceConfig,
    parts: &[CombinePart],
    join: &str,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part_path = format!("{path}/parts/{index}/value");
        match evaluate_string_field(
            item,
            source_config,
            &part.value,
            &part_path,
            strategy_key,
            item_index,
            diagnostics,
        ) {
            FieldEvaluation {
                value: Some(value),
                failed: false,
            } => values.push(value),
            FieldEvaluation {
                value: None,
                failed: false,
            } if part.optional.unwrap_or(false) => {}
            FieldEvaluation {
                value: None,
                failed: false,
            } => {
                diagnostics.push(runtime_error(
                    "required_combine_part_missing",
                    "Required combine part did not resolve to a non-empty string",
                    &part_path,
                    strategy_key,
                    json!({ "itemIndex": item_index, "partIndex": index }),
                ));
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
            FieldEvaluation { failed: true, .. } => {
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
        }
    }

    JsonStringsResult {
        values: vec![values.join(join)],
        failed: false,
    }
}

struct JsonStringsResult {
    values: Vec<String>,
    failed: bool,
}

impl JsonStringsResult {
    fn into_raw(
        self,
        cardinality: Option<Cardinality>,
        transforms: Option<&Vec<Transform>>,
    ) -> RawFieldValues<'_> {
        RawFieldValues {
            values: self.values,
            failed: self.failed,
            cardinality,
            transforms,
        }
    }
}

fn json_value_to_strings(
    value: &Value,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    match value {
        Value::Null => JsonStringsResult {
            values: Vec::new(),
            failed: false,
        },
        Value::String(value) => JsonStringsResult {
            values: vec![value.clone()],
            failed: false,
        },
        Value::Number(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Bool(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Array(values) => {
            let mut strings = Vec::new();
            for (value_index, value) in values.iter().enumerate() {
                match value {
                    Value::Null => {}
                    Value::String(value) => strings.push(value.clone()),
                    Value::Number(value) => strings.push(value.to_string()),
                    Value::Bool(value) => strings.push(value.to_string()),
                    Value::Array(_) | Value::Object(_) => {
                        diagnostics.push(runtime_error(
                            "field_type_mismatch",
                            "Field array values must resolve to strings, numbers, booleans, or null",
                            path,
                            strategy_key,
                            json!({ "itemIndex": item_index, "valueIndex": value_index }),
                        ));
                        return JsonStringsResult {
                            values: Vec::new(),
                            failed: true,
                        };
                    }
                }
            }
            JsonStringsResult {
                values: strings,
                failed: false,
            }
        }
        Value::Object(_) => {
            diagnostics.push(runtime_error(
                "field_type_mismatch",
                "Field value must resolve to a string, number, boolean, null, or an array of scalar values",
                path,
                strategy_key,
                json!({ "itemIndex": item_index }),
            ));
            JsonStringsResult {
                values: Vec::new(),
                failed: true,
            }
        }
    }
}

fn apply_transforms(
    values: Vec<String>,
    transforms: Option<&Vec<Transform>>,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    match apply_transform_pipeline(values, transforms) {
        Ok(values) => Some(values),
        Err(error) => {
            diagnostics.push(runtime_error(
                error.code,
                error.message,
                path,
                strategy_key,
                json!({
                    "itemIndex": item_index,
                    "transform": error.transform,
                }),
            ));
            None
        }
    }
}

fn extract_locations_field(
    item: &Value,
    source_config: &SourceConfig,
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

fn render_source_config_template(
    template: &str,
    source_config: &SourceConfig,
) -> Result<String, String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered = placeholder_regex
        .replace_all(template, |captures: &regex::Captures<'_>| {
            let variable = captures[1].trim();
            match render_source_config_variable(variable, source_config) {
                Ok(value) => value,
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    String::new()
                }
            }
        })
        .to_string();

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(rendered)
    }
}

fn render_source_config_variable(
    variable: &str,
    source_config: &SourceConfig,
) -> Result<String, String> {
    let Some(key) = variable.strip_prefix("sourceConfig:") else {
        return Err(format!("unsupported template variable `{variable}`"));
    };
    let value = source_config
        .get(key)
        .ok_or_else(|| format!("sourceConfig `{key}` is missing"))?;
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Err(format!("sourceConfig `{key}` is null")),
        Value::Array(_) | Value::Object(_) => Err(format!(
            "sourceConfig `{key}` must be a string, number, or boolean for template rendering"
        )),
    }
}

fn runtime_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(details),
    }
}
