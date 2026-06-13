use regex::Regex;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{collections::HashSet, fmt, future::Future, path::PathBuf, pin::Pin, time::Duration};

use crate::{
    search_request_model::{SearchRequest, SearchRuleKind, SearchRuleTarget},
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutor,
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
    fn execute<'a>(
        &'a self,
        search_request: &'a SearchRequest,
        source: &'a Source,
    ) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(search_request, source).await })
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
    let mut candidates = Vec::new();
    let mut json_errors = Vec::new();

    for json_source in embedded_result_json(html) {
        match serde_json::from_str::<Value>(&json_source) {
            Ok(value) => collect_json_candidates(&value, page_url, &mut candidates),
            Err(error) => json_errors.push(error.to_string()),
        }
    }

    candidates.extend(parse_html_card_candidates(html, page_url));
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
        if title.is_empty() {
            continue;
        }

        let window_start = anchor.start().saturating_sub(1_500);
        let window_end = html.len().min(anchor.end() + 5_000);
        let window = &html[window_start..window_end];
        let Some(company) =
            extract_first_tag_text_by_attr(window, &["company", "employer", "arbeitgeber"])
        else {
            continue;
        };
        let locations = extract_all_tag_text_by_attr(
            window,
            &["location", "standort", "arbeitsort", "job-location"],
        );
        if let Some(url) = absolute_http_url(&href, page_url) {
            if let Some(candidate) = normalized_candidate(title, company, url, locations) {
                candidates.push(candidate);
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

fn extract_first_tag_text_by_attr(fragment: &str, attr_needles: &[&str]) -> Option<String> {
    extract_all_tag_text_by_attr(fragment, attr_needles)
        .into_iter()
        .next()
}

fn extract_all_tag_text_by_attr(fragment: &str, attr_needles: &[&str]) -> Vec<String> {
    let tag_re = Regex::new(r#"(?is)<[a-z0-9]+\b(?P<attrs>[^>]*)>(?P<body>.*?)</[a-z0-9]+>"#)
        .expect("tag regex should compile");
    let mut values = Vec::new();

    for captures in tag_re.captures_iter(fragment) {
        let attrs = captures
            .name("attrs")
            .map(|match_| match_.as_str().to_lowercase())
            .unwrap_or_default();
        if !attr_needles.iter().any(|needle| attrs.contains(needle)) {
            continue;
        }
        let value = html_fragment_text(
            captures
                .name("body")
                .map(|match_| match_.as_str())
                .unwrap_or(""),
        );
        if !value.is_empty() {
            values.push(value);
        }
    }

    normalize_locations(values)
}

fn html_fragment_text(fragment: &str) -> String {
    let tag_re = Regex::new(r#"(?is)<[^>]+>"#).expect("strip-tag regex should compile");
    collapse_whitespace(&decode_html_entities(&tag_re.replace_all(fragment, " ")))
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

fn normalized_candidate(
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
) -> Option<SourceCandidate> {
    let title = collapse_whitespace(&title);
    let company = collapse_whitespace(&company);
    let url = url.trim().to_string();
    if title.is_empty() || company.is_empty() || url.is_empty() {
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
        if location.is_empty() {
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
        let key = format!(
            "{}\n{}\n{}",
            candidate.url.to_lowercase(),
            candidate.title.to_lowercase(),
            candidate.company.to_lowercase()
        );
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }

    deduped
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
                .execute(&search_request, &source)
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
                .execute(&search_request, &source)
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
                .execute(&search_request, &source)
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
                .execute(&search_request, &source)
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

            let error = executor.execute(&search_request, &source).await.expect_err(
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
