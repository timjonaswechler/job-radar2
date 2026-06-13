use quick_xml::{escape::unescape, events::Event, Reader};
use reqwest::Url;
use serde::Deserialize;
use std::{collections::HashSet, future::Future, pin::Pin, time::Duration};

use crate::{
    search_request_model::SearchRequest,
    search_run_model::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutor,
    },
    source_model::Source,
};

const ADAPTER_KEY: &str = "declarative_sitemap_jobboard";

pub(crate) struct DeclarativeSitemapJobboardExecutor<C = ReqwestSitemapHttpClient> {
    client: C,
}

impl DeclarativeSitemapJobboardExecutor<ReqwestSitemapHttpClient> {
    pub(crate) fn new_reqwest() -> Self {
        Self {
            client: ReqwestSitemapHttpClient,
        }
    }
}

impl<C> DeclarativeSitemapJobboardExecutor<C> {
    #[cfg(test)]
    fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C> SourceExecutor for DeclarativeSitemapJobboardExecutor<C>
where
    C: SitemapHttpClient + Send + Sync,
{
    fn execute<'a>(
        &'a self,
        _search_request: &'a SearchRequest,
        source: &'a Source,
    ) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move { self.execute_source(source).await })
    }
}

impl<C> DeclarativeSitemapJobboardExecutor<C>
where
    C: SitemapHttpClient + Send + Sync,
{
    async fn execute_source(
        &self,
        source: &Source,
    ) -> Result<Vec<SourceCandidate>, SourceExecutionError> {
        if source.adapter_key != ADAPTER_KEY {
            return Err(SourceExecutionError::Failed(format!(
                "adapterKey {} is not supported by {ADAPTER_KEY}",
                source.adapter_key
            )));
        }

        let config = sitemap_source_config(source)?;
        let sitemap_url = parse_http_url(&config.url, "sourceConfig.url")?;
        let locs = self
            .collect_sitemap_locs(sitemap_url, config.recursive, config.max_urls)
            .await?;
        let company = derive_company_from_source_name(&source.name);

        Ok(locs
            .into_iter()
            .filter_map(|loc| normalize_job_url(&loc, &company))
            .take(config.max_urls.unwrap_or(usize::MAX))
            .collect())
    }

    async fn collect_sitemap_locs(
        &self,
        initial_url: Url,
        recursive: bool,
        max_urls: Option<usize>,
    ) -> Result<Vec<String>, SourceExecutionError> {
        let mut pending = vec![initial_url];
        let mut visited = HashSet::new();
        let mut locs = Vec::new();

        while let Some(sitemap_url) = pending.pop() {
            if !visited.insert(sitemap_url.as_str().to_string()) {
                continue;
            }

            let xml = self
                .client
                .get_text(sitemap_url.clone())
                .await
                .map_err(|error| {
                    SourceExecutionError::Failed(format!(
                        "could not fetch sitemap {}: {error}",
                        sitemap_url.as_str()
                    ))
                })?;
            let document = parse_sitemap_document(&xml).map_err(|error| {
                SourceExecutionError::Failed(format!(
                    "could not parse sitemap XML from {}: {error}",
                    sitemap_url.as_str()
                ))
            })?;

            match document.kind {
                SitemapDocumentKind::UrlSet => {
                    for loc in document.locs {
                        locs.push(loc);
                        if max_urls.is_some_and(|max_urls| locs.len() >= max_urls) {
                            return Ok(locs);
                        }
                    }
                }
                SitemapDocumentKind::SitemapIndex if recursive => {
                    for loc in document.locs {
                        if let Ok(child_url) = parse_http_url(&loc, "sitemap loc") {
                            pending.push(child_url);
                        }
                    }
                }
                SitemapDocumentKind::SitemapIndex => {}
            }
        }

        Ok(locs)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SitemapSourceConfig {
    url: String,
    #[serde(default = "default_recursive")]
    recursive: bool,
    max_urls: Option<usize>,
}

fn default_recursive() -> bool {
    true
}

fn sitemap_source_config(source: &Source) -> Result<SitemapSourceConfig, SourceExecutionError> {
    serde_json::from_value(source.source_config.clone()).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "sourceConfig is invalid for {ADAPTER_KEY}: {error}"
        ))
    })
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SitemapDocumentKind {
    UrlSet,
    SitemapIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SitemapDocument {
    kind: SitemapDocumentKind,
    locs: Vec<String>,
}

fn parse_sitemap_document(xml: &str) -> Result<SitemapDocument, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut root_kind = None;
    let mut in_loc = false;
    let mut current_loc = String::new();
    let mut locs = Vec::new();
    let mut element_stack = Vec::<Vec<u8>>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = element.local_name().as_ref().to_vec();
                if root_kind.is_none() {
                    root_kind = Some(sitemap_document_kind(&name)?);
                }
                if name == b"loc" {
                    in_loc = true;
                    current_loc.clear();
                }
                element_stack.push(name);
            }
            Ok(Event::Empty(element)) => {
                let name = element.local_name().as_ref().to_vec();
                if root_kind.is_none() {
                    root_kind = Some(sitemap_document_kind(&name)?);
                }
            }
            Ok(Event::Text(text)) if in_loc => {
                let decoded = text
                    .xml10_content()
                    .map_err(|error| format!("loc text could not be decoded: {error}"))?;
                let unescaped = unescape(decoded.as_ref())
                    .map_err(|error| format!("loc text could not be unescaped: {error}"))?;
                current_loc.push_str(unescaped.as_ref());
            }
            Ok(Event::GeneralRef(reference)) if in_loc => {
                let decoded = reference
                    .xml10_content()
                    .map_err(|error| format!("loc entity could not be decoded: {error}"))?;
                let entity = format!("&{};", decoded.as_ref());
                let unescaped = unescape(&entity)
                    .map_err(|error| format!("loc entity could not be unescaped: {error}"))?;
                current_loc.push_str(unescaped.as_ref());
            }
            Ok(Event::CData(cdata)) if in_loc => {
                let decoded = cdata
                    .xml10_content()
                    .map_err(|error| format!("loc CDATA could not be decoded: {error}"))?;
                current_loc.push_str(decoded.as_ref());
            }
            Ok(Event::End(element)) => {
                let name = element.local_name().as_ref().to_vec();
                let opened = element_stack.pop().ok_or_else(|| {
                    format!(
                        "unexpected closing element {}",
                        String::from_utf8_lossy(&name)
                    )
                })?;
                if opened != name {
                    return Err(format!(
                        "expected closing element {}, found {}",
                        String::from_utf8_lossy(&opened),
                        String::from_utf8_lossy(&name)
                    ));
                }
                if name == b"loc" {
                    in_loc = false;
                    let loc = current_loc.trim();
                    if !loc.is_empty() {
                        locs.push(loc.to_string());
                    }
                    current_loc.clear();
                }
            }
            Ok(Event::Eof) => {
                if let Some(unclosed) = element_stack.last() {
                    return Err(format!(
                        "unexpected end of XML document inside {}",
                        String::from_utf8_lossy(unclosed)
                    ));
                }
                break;
            }
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(SitemapDocument {
        kind: root_kind.ok_or_else(|| "document has no root element".to_string())?,
        locs,
    })
}

fn sitemap_document_kind(name: &[u8]) -> Result<SitemapDocumentKind, String> {
    match name {
        b"urlset" => Ok(SitemapDocumentKind::UrlSet),
        b"sitemapindex" => Ok(SitemapDocumentKind::SitemapIndex),
        other => Err(format!(
            "expected urlset or sitemapindex root element, found {}",
            String::from_utf8_lossy(other)
        )),
    }
}

fn normalize_job_url(loc: &str, company: &str) -> Option<SourceCandidate> {
    let loc = loc.trim();
    let url = Url::parse(loc).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }

    let segments = url.path_segments()?.collect::<Vec<_>>();
    let job_segment = job_slug_segment(&segments)?;
    let decoded_slug = percent_decode_lossy(job_segment);
    let slug_parts = decoded_slug
        .split('-')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if slug_parts.len() < 2 {
        return None;
    }

    let location = slug_parts[0].to_string();
    let mut title_parts = slug_parts[1..].to_vec();
    while title_parts
        .last()
        .is_some_and(|part| part.chars().all(|character| character.is_ascii_digit()))
    {
        title_parts.pop();
    }
    let title = title_parts.join(" ");
    if title.trim().is_empty() {
        return None;
    }

    Some(SourceCandidate {
        title: collapse_whitespace(&title),
        company: company.to_string(),
        url: loc.to_string(),
        locations: vec![collapse_whitespace(&location)],
    })
}

fn job_slug_segment<'a>(segments: &'a [&'a str]) -> Option<&'a str> {
    segments
        .windows(2)
        .find(|window| window[0].eq_ignore_ascii_case("job"))
        .map(|window| window[1])
}

fn derive_company_from_source_name(source_name: &str) -> String {
    let source_name = collapse_whitespace(source_name);
    let lower = source_name.to_lowercase();
    for suffix in [
        " karriere",
        " careers",
        " career",
        " jobs",
        " stellenangebote",
    ] {
        if lower.ends_with(suffix) {
            let company = source_name[..source_name.len() - suffix.len()].trim();
            if !company.is_empty() {
                return company.to_string();
            }
        }
    }
    source_name
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn percent_decode_lossy(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

type BoxedTextFuture<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

pub(crate) trait SitemapHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_>;
}

pub(crate) struct ReqwestSitemapHttpClient;

impl SitemapHttpClient for ReqwestSitemapHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("JobRadarSitemapExecutor/0.1")
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
            DefaultSourceExecutor, SearchRunService, SearchRunStatus, SourceExecutionError,
            SourceRunStatus,
        },
        source_model::{
            create_source, create_system_profile, CreateSourceInput, CreateSystemProfileInput,
            Source, SourceStatus,
        },
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::{collections::HashMap, sync::Mutex};

    struct FixtureSitemapHttpClient {
        responses: HashMap<String, Result<String, String>>,
        requested_urls: Mutex<Vec<String>>,
    }

    impl FixtureSitemapHttpClient {
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

    impl SitemapHttpClient for FixtureSitemapHttpClient {
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
    fn schott_sitemap_source_runs_through_pipeline_without_fetching_job_detail_pages() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_id = create_sitemap_source(
                &pool,
                "schott_careers",
                "SCHOTT Karriere",
                json!({
                    "url": "https://join.schott.com/sitemap.xml",
                    "recursive": false
                }),
            )
            .await;
            let search_request = create_search_request(&pool, vec![source_id], "physik").await;
            let fixture_client = FixtureSitemapHttpClient::new([(
                "https://join.schott.com/sitemap.xml",
                Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <url>
                    <loc>https://join.schott.com/job/Mainz-StudentIn-Physik-Technik-oder-vergleichbar-%28mwd%29-55122/</loc>
                  </url>
                  <url>
                    <loc>https://join.schott.com/job/Jena-ChemielaborantIn-Analytik-07745/</loc>
                  </url>
                  <url>
                    <loc>https://join.schott.com/about-schott/</loc>
                  </url>
                </urlset>"#),
            )]);
            let executor = DeclarativeSitemapJobboardExecutor::new(fixture_client);
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
            assert_eq!(result.source_runs[0].candidate_count, 2);
            assert_eq!(result.source_runs[0].matched_count, 1);
            assert_eq!(result.postings.len(), 1);
            let posting = &result.postings[0];
            assert_eq!(
                posting.title,
                "StudentIn Physik Technik oder vergleichbar (mwd)"
            );
            assert_eq!(posting.company, "SCHOTT");
            assert_eq!(posting.locations, vec!["Mainz"]);
            assert_eq!(
                posting.url,
                "https://join.schott.com/job/Mainz-StudentIn-Physik-Technik-oder-vergleichbar-%28mwd%29-55122/"
            );
            assert_eq!(posting.sources.len(), 1);
            assert_eq!(posting.sources[0].source_key, "schott_careers");
            assert_eq!(posting.sources[0].source_name, "SCHOTT Karriere");
            assert_eq!(
                executor.client.requested_urls(),
                vec!["https://join.schott.com/sitemap.xml"]
            );
        });
    }

    #[test]
    fn sitemap_fetch_and_parse_errors_become_source_run_errors_without_stopping_pipeline() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let source_ids = vec![
                create_sitemap_source(
                    &pool,
                    "working_sitemap",
                    "Working Karriere",
                    json!({
                        "url": "https://working.example/sitemap.xml",
                        "recursive": false
                    }),
                )
                .await,
                create_sitemap_source(
                    &pool,
                    "broken_fetch",
                    "Broken Fetch Karriere",
                    json!({
                        "url": "https://broken.example/sitemap.xml",
                        "recursive": false
                    }),
                )
                .await,
                create_sitemap_source(
                    &pool,
                    "broken_xml",
                    "Broken XML Karriere",
                    json!({
                        "url": "https://broken-xml.example/sitemap.xml",
                        "recursive": false
                    }),
                )
                .await,
            ];
            let search_request = create_search_request(&pool, source_ids, "engineer").await;
            let fixture_client = FixtureSitemapHttpClient::new([
                (
                    "https://working.example/sitemap.xml",
                    Ok(r#"<urlset>
                      <url><loc>https://working.example/job/Berlin-Software-Engineer-123/</loc></url>
                    </urlset>"#),
                ),
                (
                    "https://broken.example/sitemap.xml",
                    Err("connection refused"),
                ),
                (
                    "https://broken-xml.example/sitemap.xml",
                    Ok("<urlset><url>"),
                ),
            ]);
            let executor = DeclarativeSitemapJobboardExecutor::new(fixture_client);
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

            assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
            assert_eq!(result.postings.len(), 1);
            assert_eq!(result.postings[0].title, "Software Engineer");
            let fetch_error = result
                .source_runs
                .iter()
                .find(|source_run| source_run.source_key == "broken_fetch")
                .unwrap();
            assert_eq!(fetch_error.status, SourceRunStatus::Failed);
            assert!(fetch_error
                .error
                .as_deref()
                .unwrap()
                .contains("could not fetch sitemap https://broken.example/sitemap.xml"));
            assert!(fetch_error
                .error
                .as_deref()
                .unwrap()
                .contains("connection refused"));
            let parse_error = result
                .source_runs
                .iter()
                .find(|source_run| source_run.source_key == "broken_xml")
                .unwrap();
            assert_eq!(parse_error.status, SourceRunStatus::Failed);
            assert!(parse_error.error.as_deref().unwrap().contains(
                "could not parse sitemap XML from https://broken-xml.example/sitemap.xml"
            ));
        });
    }

    #[test]
    fn default_source_executor_routes_declarative_sitemap_adapter() {
        tauri::async_runtime::block_on(async {
            let executor = DefaultSourceExecutor::new(
                tempfile::tempdir().unwrap().path().join("browser-runtime"),
            );
            let search_request = SearchRequest {
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
            };
            let source = Source {
                id: 1,
                key: "schott_careers".to_string(),
                adapter_key: ADAPTER_KEY.to_string(),
                system_profile_id: Some(1),
                browser_profile_id: None,
                name: "SCHOTT Karriere".to_string(),
                description: None,
                source_config: json!({}),
                status: SourceStatus::Active,
                validation_error: None,
                built_in: false,
                created_at: String::new(),
                updated_at: String::new(),
            };

            let error = executor
                .execute(&search_request, &source)
                .await
                .unwrap_err();

            match error {
                SourceExecutionError::Failed(message) => {
                    assert!(message.contains("sourceConfig is invalid"));
                    assert!(!message.contains("has no search-run executor yet"));
                }
                SourceExecutionError::Cancelled(message) => {
                    panic!("expected failed source execution, got cancellation: {message}")
                }
            }
        });
    }

    #[test]
    fn parses_sitemap_xml_loc_values_with_namespaces_and_entities() {
        let document = parse_sitemap_document(
            r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
              <url><loc>https://example.com/job/Mainz-Research-&amp;-Development-1/</loc></url>
            </urlset>"#,
        )
        .unwrap();

        assert_eq!(document.kind, SitemapDocumentKind::UrlSet);
        assert_eq!(
            document.locs,
            vec!["https://example.com/job/Mainz-Research-&-Development-1/"]
        );
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

    async fn create_sitemap_source(
        pool: &SqlitePool,
        key: &str,
        name: &str,
        source_config: Value,
    ) -> i64 {
        let profile = create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: format!("{key}_profile"),
                name: format!("{name} Profil"),
                description: None,
                adapter_key: ADAPTER_KEY.to_string(),
                definition_schema_version: 1,
                definition: json!({}),
                source_config_schema: json!({
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": { "type": "string", "format": "uri" },
                        "recursive": { "type": "boolean" },
                        "maxUrls": { "type": "number", "minimum": 1 }
                    }
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        create_source(
            pool,
            CreateSourceInput {
                key: key.to_string(),
                adapter_key: ADAPTER_KEY.to_string(),
                system_profile_id: Some(profile.id),
                browser_profile_id: None,
                name: name.to_string(),
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

    fn text_rule(value: &str) -> SearchRuleInput {
        SearchRuleInput {
            target: "title".to_string(),
            kind: "text".to_string(),
            value: value.to_string(),
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
