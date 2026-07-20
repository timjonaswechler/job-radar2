use super::*;
use crate::{
    profile_dsl::runtime::{
        DetailFetchError, DetailFetchRequest, DetailFetchResponse, DetailFetcher,
        ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchRequest,
        ProfileBrowserFetchResponse, UnavailableProfileBrowserClient,
    },
    search::run::{
        NormalizedPosting, PostingSource, SearchRunResult, SearchRunStatus, SourceRunResult,
    },
    source_profile::registry::SourceProfileRegistrySnapshot,
};
use serde_json::{from_str, json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

mod detail_loading;
mod import_and_merge;
mod listing_and_queues;
mod state_updates;

#[derive(Clone)]
struct FixtureDetailHttpClient {
    responses: Arc<Mutex<BTreeMap<String, Result<String, String>>>>,
    requested_urls: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Default)]
struct FixtureProfileBrowserClient {
    responses: Arc<Mutex<BTreeMap<String, Result<String, ProfileBrowserFetchError>>>>,
    requested_urls: Arc<Mutex<Vec<String>>>,
}

impl FixtureDetailHttpClient {
    fn new(responses: impl IntoIterator<Item = (String, Result<String, String>)>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            requested_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls.lock().unwrap().clone()
    }
}

impl FixtureProfileBrowserClient {
    fn new(
        responses: impl IntoIterator<Item = (String, Result<String, ProfileBrowserFetchError>)>,
    ) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            requested_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls.lock().unwrap().clone()
    }
}

impl ProfileBrowserClient for FixtureProfileBrowserClient {
    fn render<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        let url = request.url;
        self.requested_urls.lock().unwrap().push(url.clone());
        let result = self
            .responses
            .lock()
            .unwrap()
            .get(&url)
            .cloned()
            .unwrap_or_else(|| {
                Err(ProfileBrowserFetchError::new(
                    crate::profile_dsl::runtime::ProfileBrowserFetchErrorKind::NavigationFailed,
                    format!("unexpected browser detail URL: {url}"),
                ))
            });
        Box::pin(async move { result.map(|body| ProfileBrowserFetchResponse { body }) })
    }
}

impl DetailFetcher for FixtureDetailHttpClient {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        let url = request.url;
        self.requested_urls.lock().unwrap().push(url.clone());
        let result = self
            .responses
            .lock()
            .unwrap()
            .get(&url)
            .cloned()
            .unwrap_or_else(|| Err(format!("unexpected detail URL: {url}")));
        Box::pin(async move {
            result
                .map(|body| DetailFetchResponse { body })
                .map_err(DetailFetchError::new)
        })
    }
}

fn test_snapshot(
    profile_documents: Vec<String>,
    source_documents: Vec<String>,
) -> SourceProfileRegistrySnapshot {
    let snapshot = test_snapshot_with_diagnostics(profile_documents, source_documents);
    assert_eq!(snapshot.diagnostics, Vec::new());
    snapshot
}

fn test_snapshot_with_diagnostics(
    profile_documents: Vec<String>,
    source_documents: Vec<String>,
) -> SourceProfileRegistrySnapshot {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile_dir = temp_dir.path().join("source-profiles");
    let source_dir = temp_dir.path().join("sources");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::create_dir_all(&source_dir).unwrap();

    for document in &profile_documents {
        std::fs::write(
            profile_dir.join(format!("{}.json", document_key(document))),
            document,
        )
        .unwrap();
    }
    for document in &source_documents {
        std::fs::write(
            source_dir.join(format!("{}.json", document_key(document))),
            document,
        )
        .unwrap();
    }

    crate::source_profile::registry::load_snapshot(temp_dir.path())
}

fn document_key(document: &str) -> String {
    serde_json::from_str::<Value>(document).unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string()
}

fn detail_profile_json(profile_key: &str, path_key: &str, fetch_url: &str) -> String {
    profile_json(
        profile_key,
        path_key,
        Some(json!({
            "fetch": { "url": fetch_url },
            "parse": { "as": "html" },
            "fields": {
                "descriptionText": { "selectorText": ".description" }
            }
        })),
    )
}

fn profile_without_detail_json(profile_key: &str, path_key: &str) -> String {
    profile_json(profile_key, path_key, None)
}

fn profile_json(profile_key: &str, path_key: &str, detail: Option<Value>) -> String {
    profile_json_with_detail_step(profile_key, path_key, detail.map(posting_json_detail_step))
}

fn profile_json_with_detail_step(
    profile_key: &str,
    path_key: &str,
    detail: Option<Value>,
) -> String {
    let mut access_path = json!({
        "key": path_key,
        "name": path_key,
        "sourceConfigSchema": {
            "type": "object",
            "properties": {
                "token": { "type": "string" }
            },
            "additionalProperties": false
        },
        "discovery": {
            "policy": { "type": "first_accepted" },
            "strategies": [{
                "key": "fixture_discovery",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs.json",
                    "timeoutMs": 1000
                },
                "parse": { "type": "json" },
                "select": { "type": "json_path", "jsonPath": "$.jobs" },
                "extract": {
                    "fields": {
                        "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                        "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
                        "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" },
                        "postingMeta": {
                            "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "optional" }
                        }
                    }
                }
            }]
        }
    });
    if let Some(mut detail) = detail {
        detail["policy"] = json!({ "type": "first_accepted" });
        access_path["detail"] = detail;
    }

    json!({
        "schemaVersion": 3,
        "key": profile_key,
        "name": profile_key,
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "JobPostingService detail-loading fixture."
        },
        "accessPaths": [access_path]
    })
    .to_string()
}

fn posting_json_detail_step(legacy_detail: Value) -> Value {
    let fetch_url = legacy_detail["fetch"]["url"]
        .as_str()
        .expect("detail fixture has fetch.url");
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [{
            "key": "fixture_detail",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": fetch_url,
                "timeoutMs": 1000
            },
            "parse": { "type": "html" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "descriptionText": {
                        "type": "css_text",
                        "selector": ".description",
                        "cardinality": "first"
                    }
                }
            }
        }]
    })
}

fn profile_source_json(
    source_key: &str,
    profile_key: &str,
    path_key: &str,
    source_config: Value,
) -> String {
    json!({
        "schemaVersion": 3,
        "key": source_key,
        "name": source_key,
        "status": "active",
        "sourceConfig": source_config,
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": profile_key,
            "pathKey": path_key
        }
    })
    .to_string()
}

fn search_run_result(postings: Vec<NormalizedPosting>) -> SearchRunResult {
    SearchRunResult {
        search_request_id: 1,
        status: SearchRunStatus::Completed,
        generated_at: "2026-06-23T21:41:36.000Z".to_string(),
        diagnostics: Vec::new(),
        source_runs: Vec::<SourceRunResult>::new(),
        postings,
    }
}

fn posting(
    title: &str,
    company: &str,
    locations: &[&str],
    sources: Vec<PostingSource>,
) -> NormalizedPosting {
    NormalizedPosting {
        title: title.to_string(),
        company: company.to_string(),
        url: sources
            .first()
            .map(|source| source.url.clone())
            .unwrap_or_default(),
        locations: locations
            .iter()
            .map(|location| (*location).to_string())
            .collect(),
        sources,
    }
}

fn source(source_key: &str, source_name: &str, url: &str) -> PostingSource {
    source_with_meta(source_key, source_name, url, [])
}

fn source_with_meta(
    source_key: &str,
    source_name: &str,
    url: &str,
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> PostingSource {
    PostingSource {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        url: url.to_string(),
        posting_meta: posting_meta
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}

fn locations_from_row(row: &sqlx::sqlite::SqliteRow) -> Vec<String> {
    from_str::<Vec<String>>(&row.get::<String, _>("locations_json")).unwrap()
}

struct ExistingPosting<'a> {
    title: &'a str,
    company: &'a str,
    locations: &'a [&'a str],
    read_state: &'a str,
    interest_state: &'a str,
    preparation_state: &'a str,
    application_state: &'a str,
    first_seen_at: &'a str,
    last_seen_at: &'a str,
}

async fn insert_existing_posting(pool: &SqlitePool, posting: ExistingPosting<'_>) -> i64 {
    let locations_json = serde_json::to_string(&posting.locations).unwrap();
    sqlx::query(
        "INSERT INTO job_postings (
           title, company, locations_json,
           read_state, interest_state, preparation_state, application_state,
           first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(posting.title)
    .bind(posting.company)
    .bind(locations_json)
    .bind(posting.read_state)
    .bind(posting.interest_state)
    .bind(posting.preparation_state)
    .bind(posting.application_state)
    .bind(posting.first_seen_at)
    .bind(posting.last_seen_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_existing_source(
    pool: &SqlitePool,
    posting_id: i64,
    source_key: &str,
    source_name_snapshot: &str,
    url: &str,
    seen_at: &str,
) -> i64 {
    insert_existing_source_with_meta(
        pool,
        posting_id,
        source_key,
        source_name_snapshot,
        url,
        [],
        seen_at,
    )
    .await
}

async fn insert_existing_source_with_meta(
    pool: &SqlitePool,
    posting_id: i64,
    source_key: &str,
    source_name_snapshot: &str,
    url: &str,
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
    seen_at: &str,
) -> i64 {
    let posting_meta_json = serde_json::to_string(
        &posting_meta
            .into_iter()
            .collect::<BTreeMap<&'static str, &'static str>>(),
    )
    .unwrap();
    sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, source_name_snapshot, url, posting_meta_json,
           first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
    )
    .bind(posting_id)
    .bind(source_key)
    .bind(source_name_snapshot)
    .bind(url)
    .bind(posting_meta_json)
    .bind(seen_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn persist_description_text(pool: &SqlitePool, posting_id: i64, description_text: &str) {
    sqlx::query("UPDATE job_postings SET description_text = ?1 WHERE id = ?2")
        .bind(description_text)
        .bind(posting_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn persisted_description_text(pool: &SqlitePool, posting_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT description_text FROM job_postings WHERE id = ?1")
        .bind(posting_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn set_primary_source(pool: &SqlitePool, posting_id: i64, source_id: i64) {
    sqlx::query("UPDATE job_postings SET primary_source_id = ?1 WHERE id = ?2")
        .bind(source_id)
        .bind(posting_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn table_count(pool: &SqlitePool, table_name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table_name}"))
        .fetch_one(pool)
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
