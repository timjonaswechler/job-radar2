use regex::Regex;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{collections::HashSet, fmt, future::Future, path::PathBuf, pin::Pin, time::Duration};

use crate::{
    search_request_model::{SearchRequest, SearchRuleKind, SearchRuleTarget},
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutor,
    },
    source_model::Source,
};

const ADAPTER_KEY: &str = "stepstone_search";
const DEFAULT_BASE_URL: &str = "https://www.stepstone.de";

pub(crate) struct StepstoneSearchExecutor<
    B = ManagedStepstoneBrowserClient,
    H = ReqwestStepstoneHttpClient,
> {
    browser: B,
    http: H,
}

impl StepstoneSearchExecutor<ManagedStepstoneBrowserClient, ReqwestStepstoneHttpClient> {
    pub(crate) fn new_managed(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser: ManagedStepstoneBrowserClient {
                runtime_dir: browser_runtime_dir.into(),
            },
            http: ReqwestStepstoneHttpClient,
        }
    }
}

impl<B, H> StepstoneSearchExecutor<B, H> {
    #[cfg(test)]
    fn new(browser: B, http: H) -> Self {
        Self { browser, http }
    }
}

impl<B, H> SourceExecutor for StepstoneSearchExecutor<B, H>
where
    B: StepstoneBrowserClient + Send + Sync,
    H: StepstoneHttpClient + Send + Sync,
{
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            self.execute_source(input.search_request, input.source)
                .await
        })
    }
}

impl<B, H> StepstoneSearchExecutor<B, H>
where
    B: StepstoneBrowserClient + Send + Sync,
    H: StepstoneHttpClient + Send + Sync,
{
    async fn execute_source(
        &self,
        search_request: &SearchRequest,
        source: &Source,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        if source.adapter_key != ADAPTER_KEY {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by {ADAPTER_KEY}",
                source.adapter_key
            )));
        }

        let config = stepstone_source_config(source)?;
        let search_url = build_stepstone_search_url(search_request, &config)?;

        match self.browser.render_html(search_url.clone()).await {
            Ok(rendered_html) => parse_stepstone_candidates(&rendered_html, &search_url).map_err(
                |error| {
                    SourceExecutionError::Failed(format!(
                        "stepstone parse error after browser execution for {}: {error}",
                        search_url.as_str()
                    ))
                },
            ),
            Err(browser_error) => match self.http.get_text(search_url.clone()).await {
                Ok(html) => parse_stepstone_candidates(&html, &search_url).map_err(|error| {
                    SourceExecutionError::Failed(format!(
                        "stepstone parse error after HTTP fallback for {}: {error}; browser attempt: {browser_error}",
                        search_url.as_str()
                    ))
                }),
                Err(http_error) => Err(SourceExecutionError::Failed(format!(
                    "stepstone browser attempt failed for {}: {browser_error}; HTTP fallback failed: {http_error}",
                    search_url.as_str()
                ))),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepstoneSourceConfig {
    #[serde(default = "default_base_url")]
    base_url: String,
    #[allow(dead_code)]
    manual_release_start_url: Option<String>,
    #[allow(dead_code)]
    max_pages: Option<usize>,
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}

fn stepstone_source_config(source: &Source) -> Result<StepstoneSourceConfig, SourceExecutionError> {
    serde_json::from_value(source.source_config.clone()).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "sourceConfig is invalid for {ADAPTER_KEY}: {error}"
        ))
    })
}

fn build_stepstone_search_url(
    search_request: &SearchRequest,
    config: &StepstoneSourceConfig,
) -> Result<Url, SourceExecutionError> {
    let base_url = parse_http_url(&config.base_url, "sourceConfig.baseUrl")?;
    let mut url = base_url.join("/jobs").map_err(|error| {
        SourceExecutionError::Failed(format!(
            "sourceConfig.baseUrl could not be used to build StepStone jobs URL: {error}"
        ))
    })?;

    let query_text = stepstone_query_text(search_request);
    let first_location = search_request
        .locations
        .iter()
        .map(|location| collapse_whitespace(location))
        .find(|location| !location.is_empty());
    let radius_km = search_request
        .radius_km
        .map(|radius_km| radius_km.to_string());

    {
        let mut query = url.query_pairs_mut();
        if !query_text.is_empty() {
            query.append_pair("what", &query_text);
        }
        if let Some(location) = first_location.as_deref() {
            query.append_pair("where", location);
        }
        if let Some(radius_km) = radius_km.as_deref() {
            query.append_pair("radius", radius_km);
        }
    }

    Ok(url)
}

fn stepstone_query_text(search_request: &SearchRequest) -> String {
    search_request
        .include_rules
        .iter()
        .filter(|rule| rule.target == SearchRuleTarget::Title && rule.kind == SearchRuleKind::Text)
        .map(|rule| collapse_whitespace(&rule.value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
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

type BoxedStepstoneFetchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, StepstoneFetchError>> + Send + 'a>>;

pub(crate) trait StepstoneBrowserClient {
    fn render_html(&self, url: Url) -> BoxedStepstoneFetchFuture<'_>;
}

pub(crate) trait StepstoneHttpClient {
    fn get_text(&self, url: Url) -> BoxedStepstoneFetchFuture<'_>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StepstoneFetchError {
    Unavailable(String),
    Failed(String),
}

impl StepstoneFetchError {
    fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable(message.into())
    }

    fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }
}

impl fmt::Display for StepstoneFetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => {
                write!(formatter, "browser runtime unavailable: {message}")
            }
            Self::Failed(message) => write!(formatter, "{message}"),
        }
    }
}

pub(crate) struct ManagedStepstoneBrowserClient {
    runtime_dir: PathBuf,
}

impl StepstoneBrowserClient for ManagedStepstoneBrowserClient {
    fn render_html(&self, url: Url) -> BoxedStepstoneFetchFuture<'_> {
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
                return Err(StepstoneFetchError::unavailable(format!(
                    "status {:?}: {status_detail}",
                    status.status
                )));
            }

            let executable_path = status.executable_path.as_deref().ok_or_else(|| {
                StepstoneFetchError::unavailable(
                    "installed managed browser runtime has no executable path",
                )
            })?;

            let executable_path = PathBuf::from(executable_path);
            crate::browser_runtime::render_page_html(
                &executable_path,
                &self.runtime_dir,
                url.as_str(),
            )
            .await
            .map_err(|error| {
                StepstoneFetchError::failed(format!(
                    "browser execution failed for {}: {error}",
                    url.as_str()
                ))
            })
        })
    }
}

pub(crate) struct ReqwestStepstoneHttpClient;

impl StepstoneHttpClient for ReqwestStepstoneHttpClient {
    fn get_text(&self, url: Url) -> BoxedStepstoneFetchFuture<'_> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("JobRadarStepstoneExecutor/0.1")
                .build()
                .map_err(|error| StepstoneFetchError::failed(error.to_string()))?;
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|error| StepstoneFetchError::failed(error.to_string()))?;
            if !response.status().is_success() {
                return Err(StepstoneFetchError::failed(format!(
                    "HTTP {} from {}",
                    response.status(),
                    url.as_str()
                )));
            }
            response
                .text()
                .await
                .map_err(|error| StepstoneFetchError::failed(error.to_string()))
        })
    }
}

fn parse_stepstone_candidates(html: &str, page_url: &Url) -> Result<Vec<SourceCandidate>, String> {
    let mut candidates = preloaded_state_candidates(html, page_url);
    let mut json_errors = Vec::new();

    if candidates.is_empty() {
        for json_source in embedded_result_json(html) {
            match serde_json::from_str::<Value>(&json_source) {
                Ok(value) => collect_json_candidates(&value, page_url, &mut candidates),
                Err(error) => json_errors.push(error.to_string()),
            }
        }

        candidates.extend(parse_html_card_candidates(html, page_url));
    }
    let candidates = dedupe_candidates(candidates);

    if !candidates.is_empty() {
        return Ok(candidates);
    }

    if has_no_results_marker(html) {
        return Ok(Vec::new());
    }

    if !json_errors.is_empty() {
        return Err(format!(
            "could not parse embedded StepStone result JSON: {}",
            json_errors.join("; ")
        ));
    }

    Err("could not find StepStone job result data in HTML".to_string())
}

fn preloaded_state_candidates(html: &str, page_url: &Url) -> Vec<SourceCandidate> {
    let script_re = Regex::new(r#"(?is)<script\b[^>]*>(?P<body>.*?)</script>"#)
        .expect("script regex should compile");
    let mut candidates = Vec::new();

    for captures in script_re.captures_iter(html) {
        let Some(body) = captures.name("body").map(|match_| match_.as_str()) else {
            continue;
        };
        if !(body.contains("items") && body.contains("companyName") && body.contains("title")) {
            continue;
        }
        for item in preloaded_stepstone_item_objects(body) {
            if let Some(candidate) = preloaded_item_candidate(item, page_url) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

fn preloaded_stepstone_item_objects(script_body: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut search_offset = 0;

    while let Some(relative_items_index) = script_body[search_offset..].find("items") {
        let items_index = search_offset + relative_items_index;
        let context_start = items_index.saturating_sub(5_000);
        let context = &script_body[context_start..items_index];
        if !(context.contains("categorization") || context.contains("jobAdsData")) {
            search_offset = items_index + "items".len();
            continue;
        }

        let Some(relative_array_index) = script_body[items_index..].find('[') else {
            search_offset = items_index + "items".len();
            continue;
        };
        let array_index = items_index + relative_array_index;
        let Some((array_body, array_end)) = balanced_slice(script_body, array_index, '[', ']')
        else {
            search_offset = array_index + 1;
            continue;
        };
        objects.extend(top_level_object_slices(array_body));
        search_offset = array_end;
    }

    objects
}

fn preloaded_item_candidate(item: &str, page_url: &Url) -> Option<SourceCandidate> {
    let title = js_object_string_field(item, "title")?;
    let company = js_object_string_field(item, "companyName")?;
    let raw_url = js_object_string_field(item, "url")?;
    let url = absolute_http_url(&raw_url, page_url)?;
    let locations = js_object_string_field(item, "location")
        .into_iter()
        .collect::<Vec<_>>();

    normalized_candidate(title, company, url, locations)
}

fn js_object_string_field(object: &str, field: &str) -> Option<String> {
    let bytes = object.as_bytes();
    let mut index = object.find('{')? + 1;

    while index < bytes.len() {
        index = skip_js_ws_and_commas(bytes, index);
        if index >= bytes.len() || bytes[index] == b'}' {
            break;
        }

        let (key, key_end) = parse_js_object_key(object, index)?;
        index = skip_js_ws(bytes, key_end);
        if bytes.get(index) != Some(&b':') {
            break;
        }
        index = skip_js_ws(bytes, index + 1);

        if key == field {
            if matches!(bytes.get(index), Some(b'"' | b'\'')) {
                let (raw_value, _) = parse_js_string_raw(object, index)?;
                let value = collapse_whitespace(&decode_js_string_literal(raw_value));
                return (!value.is_empty()).then_some(value);
            }
            return None;
        }

        index = skip_js_value(object, index);
    }

    None
}

fn parse_js_object_key(object: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = object.as_bytes();
    match bytes.get(start) {
        Some(b'"' | b'\'') => parse_js_string_raw(object, start),
        Some(byte) if is_js_ident_start(*byte) => {
            let mut end = start + 1;
            while bytes
                .get(end)
                .is_some_and(|byte| is_js_ident_continue(*byte))
            {
                end += 1;
            }
            Some((&object[start..end], end))
        }
        _ => None,
    }
}

fn parse_js_string_raw(object: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = object.as_bytes();
    let quote = *bytes.get(start)?;
    if !matches!(quote, b'"' | b'\'') {
        return None;
    }

    let mut index = start + 1;
    let content_start = index;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some((&object[content_start..index], index + 1));
        }
        index += 1;
    }

    None
}

fn skip_js_ws_and_commas(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b',')
    {
        index += 1;
    }
    index
}

fn skip_js_ws(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn skip_js_value(object: &str, start: usize) -> usize {
    let bytes = object.as_bytes();
    let mut index = start;
    let mut nested_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote_byte) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote_byte {
                quote = None;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' | b'\'' | b'`' => quote = Some(byte),
            b'{' | b'[' | b'(' => nested_depth += 1,
            b'}' if nested_depth == 0 => return index,
            b'}' | b']' | b')' => nested_depth = nested_depth.saturating_sub(1),
            b',' if nested_depth == 0 => return index + 1,
            _ => {}
        }
        index += 1;
    }

    index
}

fn is_js_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

fn is_js_ident_continue(byte: u8) -> bool {
    is_js_ident_start(byte) || byte.is_ascii_digit()
}

fn decode_js_string_literal(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }

        match chars.next() {
            Some('"') => decoded.push('"'),
            Some('\\') => decoded.push('\\'),
            Some('/') => decoded.push('/'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('b') => decoded.push('\u{0008}'),
            Some('f') => decoded.push('\u{000c}'),
            Some('u') => {
                let hex = chars.by_ref().take(4).collect::<String>();
                if hex.len() == 4 {
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(character) = char::from_u32(code) {
                            decoded.push(character);
                        }
                    }
                }
            }
            Some(other) => decoded.push(other),
            None => decoded.push('\\'),
        }
    }

    decode_html_entities(&decoded)
}

fn balanced_slice(
    input: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<(&str, usize)> {
    let mut depth = 0usize;
    let mut content_start = None;
    let mut quote = None;
    let mut escaped = false;

    for (relative_index, character) in input[open_index..].char_indices() {
        let index = open_index + relative_index;
        if let Some(quote_character) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote_character {
                quote = None;
            }
            continue;
        }

        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if character == open {
            depth += 1;
            if depth == 1 {
                content_start = Some(index + character.len_utf8());
            }
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return content_start
                    .map(|start| (&input[start..index], index + character.len_utf8()));
            }
        }
    }

    None
}

fn top_level_object_slices(array_body: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut object_start = None;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in array_body.char_indices() {
        if let Some(quote_character) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote_character {
                quote = None;
            }
            continue;
        }

        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if character == '{' {
            if depth == 0 {
                object_start = Some(index);
            }
            depth += 1;
        } else if character == '}' {
            depth = match depth.checked_sub(1) {
                Some(depth) => depth,
                None => return objects,
            };
            if depth == 0 {
                if let Some(start) = object_start.take() {
                    objects.push(&array_body[start..index + character.len_utf8()]);
                }
            }
        }
    }

    objects
}

fn embedded_result_json(html: &str) -> Vec<String> {
    let script_re = Regex::new(r#"(?is)<script\b(?P<attrs>[^>]*)>(?P<body>.*?)</script>"#)
        .expect("script regex should compile");

    script_re
        .captures_iter(html)
        .filter_map(|captures| {
            let attrs = captures.name("attrs")?.as_str();
            let body = captures.name("body")?.as_str().trim();
            if body.is_empty() || !script_contains_stepstone_result_json(attrs, body) {
                return None;
            }
            Some(decode_html_entities(body))
        })
        .collect()
}

fn script_contains_stepstone_result_json(attrs: &str, body: &str) -> bool {
    let attrs = attrs.to_lowercase();
    attrs.contains("application/ld+json")
        || attrs.contains("__next_data__")
        || (attrs.contains("application/json") && body.to_lowercase().contains("job"))
}

fn collect_json_candidates(value: &Value, page_url: &Url, candidates: &mut Vec<SourceCandidate>) {
    match value {
        Value::Object(object) => {
            if let Some(candidate) = json_candidate_from_object(object, page_url) {
                candidates.push(candidate);
            }
            for child in object.values() {
                collect_json_candidates(child, page_url, candidates);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_json_candidates(child, page_url, candidates);
            }
        }
        _ => {}
    }
}

fn json_candidate_from_object(
    object: &Map<String, Value>,
    page_url: &Url,
) -> Option<SourceCandidate> {
    let title = first_json_string(object, &["title", "jobTitle", "name"])?;
    let raw_url = first_json_string(object, &["url", "jobUrl", "canonicalUrl", "targetUrl"])
        .or_else(|| object.get("link").and_then(json_url_like))?;
    let company = first_json_string(object, &["companyName", "employerName"])
        .or_else(|| object.get("company").and_then(json_name_like))
        .or_else(|| object.get("hiringOrganization").and_then(json_name_like))
        .or_else(|| object.get("employer").and_then(json_name_like))?;
    let url = absolute_http_url(&raw_url, page_url)?;

    normalized_candidate(title, company, url, collect_locations_from_object(object))
}

fn first_json_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_string_like))
}

fn json_string_like(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(collapse_whitespace(value)),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn json_url_like(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(collapse_whitespace(value)),
        Value::Object(object) => first_json_string(object, &["href", "url"]),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn json_name_like(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(collapse_whitespace(value)),
        Value::Object(object) => first_json_string(object, &["name", "title", "companyName"]),
        Value::Array(values) => values.iter().find_map(json_name_like),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn collect_locations_from_object(object: &Map<String, Value>) -> Vec<String> {
    let mut locations = Vec::new();

    for key in [
        "locations",
        "location",
        "jobLocation",
        "jobLocations",
        "workplace",
        "workplaces",
    ] {
        if let Some(value) = object.get(key) {
            collect_location_values(value, &mut locations);
        }
    }

    if locations.is_empty() {
        if let Some(address) = object.get("address") {
            collect_location_values(address, &mut locations);
        }
    }

    normalize_locations(locations)
}

fn collect_location_values(value: &Value, locations: &mut Vec<String>) {
    match value {
        Value::String(value) => locations.push(value.clone()),
        Value::Array(values) => {
            for value in values {
                collect_location_values(value, locations);
            }
        }
        Value::Object(object) => {
            for key in [
                "addressLocality",
                "city",
                "name",
                "formatted",
                "label",
                "text",
            ] {
                if let Some(value) = object.get(key).and_then(Value::as_str) {
                    locations.push(value.to_string());
                }
            }
            for key in ["address", "location", "locations", "jobLocation"] {
                if let Some(value) = object.get(key) {
                    collect_location_values(value, locations);
                }
            }
        }
        _ => {}
    }
}

fn parse_html_card_candidates(html: &str, page_url: &Url) -> Vec<SourceCandidate> {
    let anchor_re = Regex::new(r#"(?is)<a\b(?P<attrs>[^>]*)>(?P<body>.*?)</a>"#)
        .expect("anchor regex should compile");
    let mut candidates = Vec::new();

    for captures in anchor_re.captures_iter(html) {
        let anchor = captures.get(0).expect("whole anchor match");
        let attrs = captures
            .name("attrs")
            .map(|match_| match_.as_str())
            .unwrap_or("");
        let href = html_attr(attrs, "href");
        let Some(href) = href else {
            continue;
        };
        if !looks_like_stepstone_job_anchor(attrs, &href) {
            continue;
        }

        let title = html_fragment_text(
            captures
                .name("body")
                .map(|match_| match_.as_str())
                .unwrap_or(""),
        );
        if title.is_empty() || looks_like_css_garbage(&title) {
            continue;
        }

        let window_end = html.len().min(anchor.end() + 5_000);
        let window = &html[anchor.end()..window_end];
        let locations = extract_stepstone_marker_text(
            window,
            "job-item-location",
            &[
                "job-item-work-from-home",
                "job-item-middle",
                "job-item-badges",
            ],
        )
        .into_iter()
        .collect::<Vec<_>>();
        if let Some(url) = absolute_http_url(&href, page_url) {
            let company = extract_stepstone_marker_text(
                window,
                "job-item-company-name",
                &[
                    "job-item-location",
                    "job-item-work-from-home",
                    "job-item-middle",
                    "job-item-badges",
                ],
            )
            .or_else(|| company_from_stepstone_url(&url, &locations));
            if let Some(company) = company {
                if let Some(candidate) = normalized_candidate(title, company, url, locations) {
                    candidates.push(candidate);
                }
            }
        }
    }

    candidates
}

fn looks_like_stepstone_job_anchor(attrs: &str, href: &str) -> bool {
    let attrs = attrs.to_lowercase();
    let href = href.to_lowercase();
    attrs.contains("job-item-title")
        || attrs.contains("job-title")
        || attrs.contains("jobcard")
        || href.contains("/stellenangebote")
        || href.contains("/job/")
        || href.contains("/jobs/")
}

fn html_attr(attrs: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*(?:\"([^\"]*)\"|'([^']*)')"#,
        regex::escape(name)
    );
    let attr_re = Regex::new(&pattern).ok()?;
    attr_re.captures(attrs).and_then(|captures| {
        captures
            .get(1)
            .or_else(|| captures.get(2))
            .map(|match_| decode_html_entities(match_.as_str()))
    })
}

fn extract_stepstone_marker_text(
    fragment: &str,
    marker: &str,
    stop_markers: &[&str],
) -> Option<String> {
    let marker_index = find_stepstone_marker(fragment, marker)?;
    let start = fragment[..marker_index].rfind('<').unwrap_or(marker_index);
    let mut end = fragment.len();

    for stop_marker in stop_markers {
        if let Some(relative_stop_index) =
            find_stepstone_marker(&fragment[marker_index + 1..], stop_marker)
        {
            let stop_index = marker_index + 1 + relative_stop_index;
            let stop_tag_start = fragment[..stop_index].rfind('<').unwrap_or(stop_index);
            end = end.min(stop_tag_start);
        }
    }
    if let Some(relative_article_end) = fragment[marker_index..].find("</article>") {
        end = end.min(marker_index + relative_article_end);
    }

    let value = html_fragment_text(&fragment[start..end]);
    (!value.is_empty() && !looks_like_css_garbage(&value)).then_some(value)
}

fn find_stepstone_marker(fragment: &str, marker: &str) -> Option<usize> {
    [
        format!("data-at=\"{marker}\""),
        format!("data-testid=\"{marker}\""),
        format!("data-at='{marker}'"),
        format!("data-testid='{marker}'"),
    ]
    .into_iter()
    .filter_map(|pattern| fragment.find(&pattern))
    .min()
}

fn html_fragment_text(fragment: &str) -> String {
    let mut without_non_content = fragment.to_string();
    for tag in ["script", "style", "svg", "noscript"] {
        let pattern = format!(r#"(?is)<{}\b[^>]*>.*?</{}>"#, tag, tag);
        let tag_re = Regex::new(&pattern).expect("non-content tag regex should compile");
        without_non_content = tag_re.replace_all(&without_non_content, " ").into_owned();
    }
    let tag_re = Regex::new(r#"(?is)<[^>]+>"#).expect("strip-tag regex should compile");
    collapse_whitespace(&decode_html_entities(
        &tag_re.replace_all(&without_non_content, " "),
    ))
}

fn absolute_http_url(raw_url: &str, page_url: &Url) -> Option<String> {
    let raw_url = decode_html_entities(raw_url.trim());
    let url = page_url.join(&raw_url).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}

fn company_from_stepstone_url(url: &str, locations: &[String]) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let slug = url
        .path_segments()?
        .find(|segment| segment.starts_with("stellenangebote--"))?
        .strip_prefix("stellenangebote--")?
        .split("--")
        .next()?;
    let tokens = slug
        .split('-')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }

    let normalized_tokens = tokens
        .iter()
        .map(|token| normalized_slug_token(token))
        .collect::<Vec<_>>();
    for location in locations {
        let location_tokens = slug_tokens_from_text(location);
        if location_tokens.is_empty() {
            continue;
        }
        for start in 0..normalized_tokens.len() {
            let end = start + location_tokens.len();
            if end >= normalized_tokens.len() || end > normalized_tokens.len() {
                continue;
            }
            if normalized_tokens[start..end] == location_tokens {
                return format_company_slug_tokens(&tokens[end..]);
            }
        }
    }

    None
}

fn slug_tokens_from_text(value: &str) -> Vec<String> {
    normalize_german_slug_text(value)
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalized_slug_token(value: &str) -> String {
    normalize_german_slug_text(value)
}

fn normalize_german_slug_text(value: &str) -> String {
    value
        .replace('Ä', "Ae")
        .replace('Ö', "Oe")
        .replace('Ü', "Ue")
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss")
        .to_lowercase()
}

fn format_company_slug_tokens(tokens: &[&str]) -> Option<String> {
    let company = tokens
        .iter()
        .map(|token| match token.to_ascii_lowercase().as_str() {
            "ag" => "AG".to_string(),
            "gmbh" => "GmbH".to_string(),
            "kg" => "KG".to_string(),
            "se" => "SE".to_string(),
            "ug" => "UG".to_string(),
            "gbr" => "GbR".to_string(),
            "kgaa" => "KGaA".to_string(),
            "llc" => "LLC".to_string(),
            "inc" => "Inc".to_string(),
            _ => (*token).to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    let company = collapse_whitespace(&company);
    (!company.is_empty()).then_some(company)
}

fn normalized_candidate(
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
) -> Option<SourceCandidate> {
    let title = collapse_whitespace(&title);
    let company = collapse_whitespace(&company);
    let url = url.trim().to_string();
    if title.is_empty()
        || company.is_empty()
        || url.is_empty()
        || looks_like_css_garbage(&title)
        || looks_like_css_garbage(&company)
    {
        return None;
    }

    Some(SourceCandidate {
        title,
        company,
        url,
        locations: normalize_locations(locations),
    })
}

fn normalize_locations(locations: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for location in locations {
        let location = collapse_whitespace(&location);
        if location.is_empty() || looks_like_css_garbage(&location) {
            continue;
        }
        if seen.insert(location.to_lowercase()) {
            normalized.push(location);
        }
    }
    normalized
}

fn dedupe_candidates(candidates: Vec<SourceCandidate>) -> Vec<SourceCandidate> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for candidate in candidates {
        let key = candidate.url.to_lowercase();
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }

    deduped
}

fn looks_like_css_garbage(value: &str) -> bool {
    let lower = value.to_lowercase();
    (lower.contains('{') && lower.contains('}'))
        || lower.starts_with(".res-")
        || lower.starts_with("<path")
        || lower.contains("box-sizing")
        || lower.contains("data-genesis-element")
        || lower.contains("fill=\"currentcolor\"")
        || lower.contains("viewbox")
}

fn has_no_results_marker(html: &str) -> bool {
    let lower = html.to_lowercase();
    lower.contains("data-at=\"no-results\"")
        || lower.contains("data-testid=\"no-results\"")
        || lower.contains("keine passenden stellenangebote")
        || lower.contains("keine jobs gefunden")
        || lower.contains("no jobs found")
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_html_entities(value: &str) -> String {
    let decoded = value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    decode_numeric_html_entities(&decoded)
}

fn decode_numeric_html_entities(value: &str) -> String {
    let numeric_re =
        Regex::new(r#"(?i)&#(x[0-9a-f]+|\d+);"#).expect("numeric entity regex should compile");
    let mut decoded = String::with_capacity(value.len());
    let mut last_end = 0;

    for captures in numeric_re.captures_iter(value) {
        let entity = captures.get(0).expect("whole entity match");
        decoded.push_str(&value[last_end..entity.start()]);
        let code = captures
            .get(1)
            .and_then(|match_| parse_html_entity_code(match_.as_str()));
        if let Some(character) = code.and_then(char::from_u32) {
            decoded.push(character);
        } else {
            decoded.push_str(entity.as_str());
        }
        last_end = entity.end();
    }

    decoded.push_str(&value[last_end..]);
    decoded
}

fn parse_html_entity_code(value: &str) -> Option<u32> {
    if let Some(hex) = value.strip_prefix('x').or_else(|| value.strip_prefix('X')) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        value.parse::<u32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        search_request_model::{
            CreateSearchRequestInput, RunningSearchRuns, SearchRequestService, SearchRequestStatus,
            SearchRule, SearchRuleInput,
        },
        search_run_model::{SearchRunService, SearchRunStatus, SourceRunStatus},
        source_model::{
            create_browser_profile, create_source, CreateBrowserProfileInput, CreateSourceInput,
            SourceStatus,
        },
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::{collections::VecDeque, sync::Mutex};

    struct FixtureStepstoneBrowserClient {
        responses: Mutex<VecDeque<Result<String, StepstoneFetchError>>>,
        requested_urls: Mutex<Vec<String>>,
    }

    impl FixtureStepstoneBrowserClient {
        fn new(responses: Vec<Result<&'static str, StepstoneFetchError>>) -> Self {
            Self {
                responses: Mutex::new(
                    responses
                        .into_iter()
                        .map(|response| response.map(str::to_string))
                        .collect(),
                ),
                requested_urls: Mutex::new(Vec::new()),
            }
        }

        fn requested_urls(&self) -> Vec<String> {
            self.requested_urls.lock().unwrap().clone()
        }
    }

    impl StepstoneBrowserClient for FixtureStepstoneBrowserClient {
        fn render_html(&self, url: Url) -> BoxedStepstoneFetchFuture<'_> {
            Box::pin(async move {
                self.requested_urls
                    .lock()
                    .unwrap()
                    .push(url.as_str().to_string());
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or_else(|| Err(StepstoneFetchError::failed("missing browser fixture")))
            })
        }
    }

    struct FixtureStepstoneHttpClient {
        responses: Mutex<VecDeque<Result<String, StepstoneFetchError>>>,
        requested_urls: Mutex<Vec<String>>,
    }

    impl FixtureStepstoneHttpClient {
        fn new(responses: Vec<Result<&'static str, StepstoneFetchError>>) -> Self {
            Self {
                responses: Mutex::new(
                    responses
                        .into_iter()
                        .map(|response| response.map(str::to_string))
                        .collect(),
                ),
                requested_urls: Mutex::new(Vec::new()),
            }
        }

        fn requested_urls(&self) -> Vec<String> {
            self.requested_urls.lock().unwrap().clone()
        }
    }

    impl StepstoneHttpClient for FixtureStepstoneHttpClient {
        fn get_text(&self, url: Url) -> BoxedStepstoneFetchFuture<'_> {
            Box::pin(async move {
                self.requested_urls
                    .lock()
                    .unwrap()
                    .push(url.as_str().to_string());
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or_else(|| Err(StepstoneFetchError::failed("missing HTTP fixture")))
            })
        }
    }

    #[test]
    fn adapter_builds_url_from_text_location_and_radius_and_uses_browser_first() {
        tauri::async_runtime::block_on(async {
            let browser = FixtureStepstoneBrowserClient::new(vec![Ok(json_ld_html())]);
            let http = FixtureStepstoneHttpClient::new(vec![Err(StepstoneFetchError::failed(
                "HTTP should not be used",
            ))]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let search_request = search_request(
                vec![
                    text_rule(" Rust  Engineer "),
                    regex_rule("Senior\\s+Developer"),
                    text_rule(" Data "),
                ],
                vec![regex_rule("Intern")],
                vec![" Berlin ", "München"],
                Some(50),
            );
            let source = source(json!({ "baseUrl": "https://stepstone.example" }));

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect("browser fixture should produce candidates");

            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].title, "Rust Engineer");
            assert_eq!(candidates[0].company, "ACME GmbH");
            assert_eq!(candidates[0].locations, vec!["Berlin"]);
            assert_eq!(
                candidates[0].url,
                "https://stepstone.example/stellenangebote--Rust-Engineer-Berlin-ACME--123.html"
            );

            let browser_urls = executor.browser.requested_urls();
            assert_eq!(browser_urls.len(), 1);
            let requested_url = Url::parse(&browser_urls[0]).unwrap();
            assert_eq!(
                requested_url.as_str(),
                "https://stepstone.example/jobs?what=Rust+Engineer+Data&where=Berlin&radius=50"
            );
            assert!(!requested_url.as_str().contains("Senior"));
            assert!(!requested_url.as_str().contains("Intern"));
            assert!(!requested_url.as_str().contains("M%C3%BCnchen"));
            assert!(executor.http.requested_urls().is_empty());
        });
    }

    #[test]
    fn adapter_falls_back_to_http_when_browser_is_unavailable() {
        tauri::async_runtime::block_on(async {
            let browser = FixtureStepstoneBrowserClient::new(vec![Err(
                StepstoneFetchError::unavailable("managed browser runtime is not installed"),
            )]);
            let http = FixtureStepstoneHttpClient::new(vec![Ok(html_card_results())]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let search_request = search_request(
                vec![text_rule("Developer")],
                vec![],
                vec!["Hamburg"],
                Some(30),
            );
            let source = source(json!({ "baseUrl": "https://stepstone.example" }));

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect("HTTP fallback should produce candidates");

            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].title, "Senior Developer");
            assert_eq!(candidates[1].title, "Senior Intern Developer");
            assert_eq!(
                executor.browser.requested_urls(),
                executor.http.requested_urls()
            );
        });
    }

    #[test]
    fn adapter_extracts_preloaded_search_result_items_before_html_fallback() {
        tauri::async_runtime::block_on(async {
            let browser =
                FixtureStepstoneBrowserClient::new(vec![Ok(preloaded_search_results_html())]);
            let http = FixtureStepstoneHttpClient::new(vec![Err(StepstoneFetchError::failed(
                "HTTP should not be used",
            ))]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let search_request =
                search_request(vec![text_rule("Laser")], vec![], vec!["Mainz"], Some(30));
            let source = source(json!({ "baseUrl": "https://stepstone.example" }));

            let candidates = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect("preloaded search-result items should produce candidates");

            assert_eq!(candidates.len(), 2);
            assert_eq!(
                candidates[0].title,
                "Masterthesis Laser-/ Materialbearbeitung (m/w/d)*"
            );
            assert_eq!(candidates[0].company, "SCHOTT AG");
            assert_eq!(candidates[0].locations, vec!["Mainz"]);
            assert_eq!(
                candidates[0].url,
                "https://stepstone.example/stellenangebote--Masterthesis-Laser-Materialbearbeitung-m-w-d-Mainz-SCHOTT-AG--14098611-inline.html"
            );
            assert_eq!(
                candidates[1].title,
                "Techniker für Laser- & LED-Anlagen (m/w/d)"
            );
            assert_eq!(candidates[1].company, "Schmoll Maschinen GmbH");
            assert_eq!(candidates[1].locations, vec!["Rödermark"]);
            assert!(executor.http.requested_urls().is_empty());
        });
    }

    #[test]
    fn html_fallback_uses_exact_stepstone_markers_without_css_noise() {
        let page_url = Url::parse("https://stepstone.example/jobs?what=Physik+Laser").unwrap();
        let candidates = parse_stepstone_candidates(nested_html_card_with_style_noise(), &page_url)
            .expect("nested HTML card should parse");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].title,
            "Mathematiker/Mathematikerin / Physiker/Physikerin für die Unternehmensberatung (w/m/d)"
        );
        assert_eq!(
            candidates[0].company,
            "KPMG AG Wirtschaftsprüfungsgesellschaft"
        );
        assert_eq!(
            candidates[0].locations,
            vec!["Frankfurt am Main, Düsseldorf, Köln, Essen, Dortmund, Münster, Mainz"]
        );
        assert_eq!(
            candidates[0].url,
            "https://www.stepstone.de/stellenangebote--Mathematiker-Mathematikerin-Physiker-Physikerin-fuer-die-Unternehmensberatung-w-m-d-Frankfurt-am-Main-Duesseldorf-Koeln-Essen-Dortmund-Muenster-Mainz-Saarbruecken-Aachen-KPMG-AG-Wirtschaftspruefungsgesellschaft--13253573-inline.html"
        );
    }

    #[test]
    fn adapter_maps_browser_and_http_fetch_failures_explicitly() {
        tauri::async_runtime::block_on(async {
            let browser = FixtureStepstoneBrowserClient::new(vec![Err(
                StepstoneFetchError::failed("chromium crashed"),
            )]);
            let http =
                FixtureStepstoneHttpClient::new(vec![Err(StepstoneFetchError::failed("HTTP 503"))]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let search_request = search_request(vec![text_rule("Developer")], vec![], vec![], None);
            let source = source(json!({ "baseUrl": "https://stepstone.example" }));

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect_err("both fetch paths should fail");

            assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "stepstone browser attempt failed for https://stepstone.example/jobs?what=Developer: chromium crashed; HTTP fallback failed: HTTP 503"
                        .to_string()
                )
            );
        });
    }

    #[test]
    fn adapter_maps_parse_failure_to_source_error_without_http_fallback() {
        tauri::async_runtime::block_on(async {
            let browser = FixtureStepstoneBrowserClient::new(vec![Ok(
                "<html><body>unexpected layout</body></html>",
            )]);
            let http = FixtureStepstoneHttpClient::new(vec![Err(StepstoneFetchError::failed(
                "HTTP should not be used after browser parse failure",
            ))]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let search_request = search_request(vec![text_rule("Developer")], vec![], vec![], None);
            let source = source(json!({ "baseUrl": "https://stepstone.example" }));

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect_err("unparseable browser content should fail explicitly");

            assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "stepstone parse error after browser execution for https://stepstone.example/jobs?what=Developer: could not find StepStone job result data in HTML"
                        .to_string()
                )
            );
            assert!(executor.http.requested_urls().is_empty());
        });
    }

    #[test]
    fn default_source_executor_routes_stepstone_adapter() {
        tauri::async_runtime::block_on(async {
            let executor = crate::search_run_model::DefaultSourceExecutor::new(
                tempfile::tempdir().unwrap().path().join("browser-runtime"),
            );
            let search_request = search_request(vec![text_rule("Developer")], vec![], vec![], None);
            let source = source(json!({ "baseUrl": "not a url" }));

            let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                    system_profile: None,
                })
                .await
                .expect_err(
                    "invalid StepStone source config should fail before any network access",
                );

            match error {
                SourceExecutionError::Failed(message) => {
                    assert!(message.contains("sourceConfig.baseUrl"));
                    assert!(!message.contains("has no search-run executor yet"));
                }
                SourceExecutionError::Cancelled(message) => {
                    panic!("expected failed source execution, got cancellation: {message}")
                }
            }
        });
    }

    #[test]
    fn stepstone_candidates_flow_through_common_regex_and_exclusion_pipeline_locally() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source =
                create_stepstone_source(&pool, json!({ "baseUrl": "https://stepstone.example" }))
                    .await;
            let running_search_runs = RunningSearchRuns::default();
            let search_request = SearchRequestService::new(&pool, &running_search_runs)
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![
                        text_rule("No Such Text"),
                        regex_rule("Senior\\s+Developer"),
                    ],
                    exclude_rules: vec![regex_rule("Intern")],
                    locations: vec!["Hamburg".to_string(), "Berlin".to_string()],
                    radius_km: Some(25),
                    source_ids: vec![source.id],
                })
                .await
                .unwrap();
            let browser = FixtureStepstoneBrowserClient::new(vec![Ok(html_card_results())]);
            let http = FixtureStepstoneHttpClient::new(vec![]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let temp_dir = tempfile::tempdir().unwrap();

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
            assert_eq!(result.source_runs[0].candidate_count, 2);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            assert_eq!(result.postings[0].title, "Senior Developer");
            assert_eq!(result.postings[0].sources.len(), 1);
            assert_eq!(
                result.postings[0].sources[0].source_key,
                "stepstone_fixture"
            );
            assert_eq!(
                result.postings[0].sources[0].url,
                "https://stepstone.example/stellenangebote--Senior-Developer-Hamburg-ACME--456.html"
            );

            let requested_urls = executor.browser.requested_urls();
            assert_eq!(requested_urls.len(), 1);
            let requested_url = Url::parse(&requested_urls[0]).unwrap();
            assert_eq!(
                requested_url.as_str(),
                "https://stepstone.example/jobs?what=No+Such+Text&where=Hamburg&radius=25"
            );
            assert!(!requested_url.as_str().contains("Senior"));
            assert!(!requested_url.as_str().contains("Intern"));
            assert!(!requested_url.as_str().contains("Berlin"));
        });
    }

    #[test]
    fn stepstone_parse_failure_becomes_partial_source_error_when_another_source_completes() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let browser_profile_id = create_stepstone_browser_profile(&pool).await;
            let broken_source = create_stepstone_source_with_key(
                &pool,
                browser_profile_id,
                "stepstone_broken",
                json!({ "baseUrl": "https://stepstone.example" }),
            )
            .await;
            let healthy_source = create_stepstone_source_with_key(
                &pool,
                browser_profile_id,
                "stepstone_healthy",
                json!({ "baseUrl": "https://stepstone.example" }),
            )
            .await;
            let running_search_runs = RunningSearchRuns::default();
            let search_request = SearchRequestService::new(&pool, &running_search_runs)
                .create(CreateSearchRequestInput {
                    status: SearchRequestStatus::Active,
                    include_rules: vec![text_rule("Developer")],
                    exclude_rules: vec![],
                    locations: vec!["Hamburg".to_string()],
                    radius_km: None,
                    source_ids: vec![broken_source.id, healthy_source.id],
                })
                .await
                .unwrap();
            let browser = FixtureStepstoneBrowserClient::new(vec![
                Ok("<html><body>unexpected layout</body></html>"),
                Ok(html_card_results()),
            ]);
            let http = FixtureStepstoneHttpClient::new(vec![]);
            let executor = StepstoneSearchExecutor::new(browser, http);
            let temp_dir = tempfile::tempdir().unwrap();

            let result = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .run(search_request.id)
            .await
            .unwrap();

            assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
            assert_eq!(result.source_runs[0].source_key, "stepstone_broken");
            assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
            assert!(result.source_runs[0]
                .error
                .as_deref()
                .unwrap()
                .contains("stepstone parse error after browser execution"));
            assert_eq!(result.source_runs[1].source_key, "stepstone_healthy");
            assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
            assert_eq!(result.postings.len(), 2);
        });
    }

    fn json_ld_html() -> &'static str {
        r#"
        <html>
          <head>
            <script type="application/ld+json">
              {
                "@context": "https://schema.org",
                "@type": "ItemList",
                "itemListElement": [
                  {
                    "@type": "ListItem",
                    "item": {
                      "@type": "JobPosting",
                      "title": "Rust Engineer",
                      "hiringOrganization": { "@type": "Organization", "name": "ACME GmbH" },
                      "jobLocation": [
                        { "@type": "Place", "address": { "addressLocality": "Berlin" } }
                      ],
                      "url": "/stellenangebote--Rust-Engineer-Berlin-ACME--123.html"
                    }
                  }
                ]
              }
            </script>
          </head>
          <body></body>
        </html>
        "#
    }

    fn html_card_results() -> &'static str {
        r#"
        <html>
          <body>
            <article data-at="job-item">
              <a data-at="job-item-title" href="/stellenangebote--Senior-Developer-Hamburg-ACME--456.html">
                Senior Developer
              </a>
              <span data-at="job-item-company-name">ACME GmbH</span>
              <span data-at="job-item-location">Hamburg</span>
            </article>
            <article data-at="job-item">
              <a data-at="job-item-title" href="/stellenangebote--Senior-Intern-Developer-Hamburg-ACME--789.html">
                Senior Intern Developer
              </a>
              <span data-at="job-item-company-name">ACME GmbH</span>
              <span data-at="job-item-location">Hamburg</span>
            </article>
          </body>
        </html>
        "#
    }

    fn preloaded_search_results_html() -> &'static str {
        r#"
        <html>
          <body>
            <script>
              window.__PRELOADED_STATE__ = window.__PRELOADED_STATE__ || {};
              window.__PRELOADED_STATE__.SearchResults = {
                props: {
                  searchResults: {
                    categorization: { what: "Physik Laser", where: "Mainz", radius: 30 },
                    items: [
                      {
                        score: 0.47262776,
                        id: 14098611,
                        title: "Masterthesis Laser-/ Materialbearbeitung (m/w/d)*",
                        normalizedTitle: "",
                        tracking: { location: "<path fill=\"currentColor\" d=\"M9.99984 16.157C11.5885\"></path>" },
                        location: "Mainz",
                        companyId: 56082,
                        companyName: "SCHOTT AG",
                        companyUrl: "https://www.stepstone.de/cmp/de/SCHOTT-AG-56082/jobs.html",
                        url: "/stellenangebote--Masterthesis-Laser-Materialbearbeitung-m-w-d-Mainz-SCHOTT-AG--14098611-inline.html",
                        metaData: { positionOnPage: 5 }
                      },
                      {
                        score: 0.6145367,
                        id: 13903683,
                        title: "Techniker für Laser- & LED-Anlagen (m/w/d)",
                        normalizedTitle: "Techniker",
                        location: "Rödermark",
                        companyId: 78687,
                        companyName: "Schmoll Maschinen GmbH",
                        companyUrl: "https://www.stepstone.de/cmp/de/Schmoll-Maschinen-GmbH-78687/jobs.html",
                        url: "/stellenangebote--Techniker-fuer-Laser-LED-Anlagen-m-w-d-Roedermark-Schmoll-Maschinen-GmbH--13903683-inline.html",
                        metaData: { positionOnPage: 9 }
                      }
                    ],
                    meta: { jobItemCount: 25 }
                  }
                }
              };
            </script>
            <article data-at="job-item">
              <a data-at="job-item-title" href="/stellenangebote--Masterthesis-Laser-Materialbearbeitung-m-w-d-Mainz-SCHOTT-AG--14098611-inline.html">
                <style>.res-146mwm8{box-sizing:border-box;margin:0;}</style>
                Masterthesis Laser-/ Materialbearbeitung (m/w/d)*
              </a>
              <span data-at="job-item-company-name"><style>.res-dhwsg9{box-sizing:border-box;}</style></span>
            </article>
          </body>
        </html>
        "#
    }

    fn nested_html_card_with_style_noise() -> &'static str {
        r#"
        <html>
          <body>
            <article data-at="job-item">
              <h2>
                <a data-testid="job-item-title" data-at="job-item-title" href="https://www.stepstone.de/stellenangebote--Mathematiker-Mathematikerin-Physiker-Physikerin-fuer-die-Unternehmensberatung-w-m-d-Frankfurt-am-Main-Duesseldorf-Koeln-Essen-Dortmund-Muenster-Mainz-Saarbruecken-Aachen-KPMG-AG-Wirtschaftspruefungsgesellschaft--13253573-inline.html">
                  <style>.res-146mwm8{box-sizing:border-box;margin:0;}</style>
                  <div><div>Mathematiker/Mathematikerin / Physiker/Physikerin für die Unternehmensberatung (w/m/d)</div></div>
                </a>
              </h2>
              <div>
                <span data-at="job-item-company-name">
                  <span><span><svg><path d="M3 16"></path></svg></span><span><div>KPMG AG Wirtschaftsprüfungsgesellschaft</div></span></span>
                </span>
                <span data-at="job-item-location">
                  <span><svg><path d="M9 16"></path></svg></span>
                  <span>Frankfurt am Main, Düsseldorf, Köln, Essen, Dortmund, Münster, Mainz</span>
                </span>
              </div>
            </article>
          </body>
        </html>
        "#
    }

    fn source(source_config: Value) -> Source {
        Source {
            id: 42,
            key: "stepstone_de".to_string(),
            adapter_key: ADAPTER_KEY.to_string(),
            system_profile_id: None,
            browser_profile_id: Some(1),
            name: "StepStone Deutschland".to_string(),
            description: None,
            source_config,
            status: SourceStatus::Active,
            validation_error: None,
            built_in: true,
            created_at: "2026-06-13T00:00:00.000Z".to_string(),
            updated_at: "2026-06-13T00:00:00.000Z".to_string(),
        }
    }

    fn search_request(
        include_rules: Vec<SearchRuleInput>,
        exclude_rules: Vec<SearchRuleInput>,
        locations: Vec<&str>,
        radius_km: Option<i64>,
    ) -> SearchRequest {
        SearchRequest {
            id: 1,
            status: SearchRequestStatus::Active,
            include_rules: include_rules
                .into_iter()
                .map(|rule| SearchRule {
                    target: SearchRuleTarget::try_from(rule.target.as_str()).unwrap(),
                    kind: SearchRuleKind::try_from(rule.kind.as_str()).unwrap(),
                    value: rule.value,
                })
                .collect(),
            exclude_rules: exclude_rules
                .into_iter()
                .map(|rule| SearchRule {
                    target: SearchRuleTarget::try_from(rule.target.as_str()).unwrap(),
                    kind: SearchRuleKind::try_from(rule.kind.as_str()).unwrap(),
                    value: rule.value,
                })
                .collect(),
            locations: locations.into_iter().map(str::to_string).collect(),
            radius_km,
            source_ids: vec![42],
            validation_error: None,
            created_at: "2026-06-13T00:00:00.000Z".to_string(),
            updated_at: "2026-06-13T00:00:00.000Z".to_string(),
        }
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

    async fn create_stepstone_source(pool: &SqlitePool, source_config: Value) -> Source {
        let browser_profile_id = create_stepstone_browser_profile(pool).await;
        create_stepstone_source_with_key(
            pool,
            browser_profile_id,
            "stepstone_fixture",
            source_config,
        )
        .await
    }

    async fn create_stepstone_browser_profile(pool: &SqlitePool) -> i64 {
        create_browser_profile(
            pool,
            CreateBrowserProfileInput {
                key: "manual_release".to_string(),
                name: "Manuelle Freigabe".to_string(),
                description: None,
                name_i18n_key: None,
                description_i18n_key: None,
                definition_path: None,
                definition_hash: None,
                definition_schema_version: 1,
                definition: json!({}),
                source_config_schema: json!({}),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn create_stepstone_source_with_key(
        pool: &SqlitePool,
        browser_profile_id: i64,
        key: &str,
        source_config: Value,
    ) -> Source {
        create_source(
            pool,
            CreateSourceInput {
                key: key.to_string(),
                adapter_key: ADAPTER_KEY.to_string(),
                system_profile_id: None,
                browser_profile_id: Some(browser_profile_id),
                name: format!("StepStone Fixture {key}"),
                description: None,
                source_config,
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
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
