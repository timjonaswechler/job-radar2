#![allow(dead_code)]

use serde_json::{json, Value};
use source_engine::test_support::{ScriptedBrowserAcquisition, ScriptedProfileHttpClient};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{collections::BTreeMap, sync::Arc};

pub async fn pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    pool
}

pub struct Posting<'a> {
    pub title: &'a str,
    pub read: &'a str,
    pub interest: &'a str,
    pub preparation: &'a str,
    pub application: &'a str,
    pub last_seen: &'a str,
}

pub async fn insert_posting(pool: &SqlitePool, posting: Posting<'_>) -> (i64, i64) {
    let id = sqlx::query(
        "INSERT INTO job_postings (
           title, company, locations_json, read_state, interest_state,
           preparation_state, application_state, first_seen_at, last_seen_at
         ) VALUES (?1, 'ACME', '[\"Mainz\"]', ?2, ?3, ?4, ?5,
                   '2026-06-01T00:00:00.000Z', ?6)",
    )
    .bind(posting.title)
    .bind(posting.read)
    .bind(posting.interest)
    .bind(posting.preparation)
    .bind(posting.application)
    .bind(posting.last_seen)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let slug = posting
        .title
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let source_id = insert_source(
        pool,
        id,
        &format!("source_{slug}"),
        &format!("https://example.test/jobs/{slug}"),
        true,
        BTreeMap::new(),
    )
    .await;
    (id, source_id)
}

pub async fn insert_source(
    pool: &SqlitePool,
    posting_id: i64,
    source_key: &str,
    provider_url: &str,
    is_primary: bool,
    posting_meta: BTreeMap<String, String>,
) -> i64 {
    let posting_meta_json = serde_json::to_string(&posting_meta).unwrap();
    sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, identity_kind, identity_value, provider_url,
           source_name_snapshot, posting_meta_json, is_primary, first_seen_at, last_seen_at
         ) VALUES (?1, ?2, 'normalized_url', ?3, ?3, ?2, ?4, ?5,
                   '2026-06-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z')",
    )
    .bind(posting_id)
    .bind(source_key)
    .bind(provider_url)
    .bind(posting_meta_json)
    .bind(is_primary)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

pub fn detail_profile(profile_key: &str, path_key: &str, mode: &str) -> String {
    detail_profile_url(profile_key, path_key, mode, "{{posting:url}}")
}

pub fn detail_profile_url(profile_key: &str, path_key: &str, mode: &str, url: &str) -> String {
    let fetch = if mode == "browser" {
        json!({ "mode": "browser", "url": url, "timeoutMs": 1000 })
    } else {
        json!({ "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 })
    };
    json!({
        "schemaVersion": 3,
        "key": profile_key,
        "name": profile_key,
        "kind": "generic",
        "support": { "level": "experimental", "summary": "Detail fixture." },
        "accessPaths": [{
            "key": path_key,
            "name": path_key,
            "sourceConfigSchema": { "type": "object", "additionalProperties": false },
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "discovery",
                    "fetch": { "mode": "http", "method": "GET", "url": "https://example.test/jobs", "timeoutMs": 1000 },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "reference": { "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" } },
                        "postingMeta": { "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "optional" } }
                    }
                }]
            },
            "detail": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "detail",
                    "fetch": fetch,
                    "parse": { "type": "html" },
                    "select": { "type": "document" },
                    "extract": { "fields": { "descriptionText": {
                        "type": "css_text", "selector": ".description", "cardinality": "first"
                    } } }
                }]
            }
        }]
    }).to_string()
}

pub fn profile_without_detail(profile_key: &str, path_key: &str) -> String {
    let mut value: Value =
        serde_json::from_str(&detail_profile(profile_key, path_key, "http")).unwrap();
    value["accessPaths"][0]
        .as_object_mut()
        .unwrap()
        .remove("detail");
    value.to_string()
}

pub fn source_document(source_key: &str, profile_key: &str, path_key: &str) -> String {
    source_document_config(source_key, profile_key, path_key, json!({}))
}

pub fn source_document_config(
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

pub struct Installed {
    pub _temp: tempfile::TempDir,
    pub store: sources::installed::Store,
}

pub fn installed(profiles: &[String], sources: &[String]) -> Installed {
    let temp = tempfile::tempdir().unwrap();
    let profile_dir = temp.path().join("source-profiles");
    let source_dir = temp.path().join("sources");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::create_dir_all(&source_dir).unwrap();
    for document in profiles {
        std::fs::write(
            profile_dir.join(format!("{}.json", document_key(document))),
            document,
        )
        .unwrap();
    }
    for document in sources {
        std::fs::write(
            source_dir.join(format!("{}.json", document_key(document))),
            document,
        )
        .unwrap();
    }
    let store = sources::installed::Store::new(temp.path());
    Installed { _temp: temp, store }
}

fn document_key(document: &str) -> String {
    serde_json::from_str::<Value>(document).unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn no_browser() -> Arc<ScriptedBrowserAcquisition> {
    Arc::new(ScriptedBrowserAcquisition::new([]))
}

pub fn no_http() -> Arc<ScriptedProfileHttpClient> {
    Arc::new(ScriptedProfileHttpClient::new([]))
}
