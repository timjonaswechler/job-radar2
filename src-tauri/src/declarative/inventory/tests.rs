use super::*;
use crate::{
    declarative::template::render_template,
    search::request::{
        CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
        SearchRequestStatus, SearchRuleInput,
    },
    search::run::{
        DefaultSourceExecutor, SearchRunService, SearchRunStatus, SourceCandidate,
        SourceExecutionError, SourceExecutionInput, SourceExecutionSource, SourceExecutor,
        SourceRunStatus,
    },
    source::registry::ResolvedSelectedAccessPath,
};
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Mutex,
};

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
fn xml_text_selection_uses_local_names_and_keeps_empty_text_values() {
    let values = parse_xml_text_values(
        r#"<urlset xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9">
              <s:loc>https://example.test/a?x=1&amp;y=2</s:loc>
              <loc><![CDATA[https://example.test/cdata]]></loc>
              <loc></loc>
            </urlset>"#,
        "loc",
    )
    .unwrap();

    assert_eq!(
        values,
        vec![
            "https://example.test/a?x=1&y=2".to_string(),
            "https://example.test/cdata".to_string(),
            String::new(),
        ]
    );
}

#[test]
fn xml_element_selection_maps_dom_to_structured_json_values() {
    let values = parse_xml_element_values(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <feed xmlns:j="urn:jobs">
              <j:item ignored="attribute">
                ignored mixed parent text
                <title lang="en">Senior &amp; Staff <![CDATA[Engineer]]></title>
                <details>
                  <team>Platform</team>
                </details>
                <tag>Rust</tag>
                <tag>XML</tag>
                <empty></empty>
                <selfClosing />
                trailing mixed parent text
              </j:item>
            </feed>"#,
        "item",
    )
    .unwrap();

    assert_eq!(
        values,
        vec![json!({
            "title": "Senior & Staff Engineer",
            "details": {
                "team": "Platform"
            },
            "tag": ["Rust", "XML"],
            "empty": "",
            "selfClosing": ""
        })]
    );
}

#[test]
fn xml_element_inventory_uses_json_path_and_item_json_template_fields() {
    tauri::async_runtime::block_on(async {
        let fixture_client = FixtureInventoryHttpClient::new([(
            "https://example.test/jobs.xml",
            Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <feed xmlns:j="urn:jobs">
                  <j:job>
                    <id>runtime-42</id>
                    <title>Platform Engineer</title>
                    <details>
                      <team>Runtime Team</team>
                    </details>
                    <locations>
                      <location>Berlin</location>
                      <location>Munich</location>
                    </locations>
                  </j:job>
                </feed>"#),
        )]);
        let executor = DeclarativeInventoryExecutor::new(fixture_client);
        let search_request = search_request();
        let source = source_with_inventory(
            DECLARATIVE_HTTP_ADAPTER_KEY,
            json!({ "startUrl": "https://example.test/jobs.xml" }),
            json!({
                "fetch": { "url": "{{sourceConfig:startUrl}}" },
                "parse": { "as": "xml" },
                "items": {
                    "select": { "xmlElement": "job" }
                },
                "fields": {
                    "title": { "jsonPath": "$.title" },
                    "url": { "template": "https://example.test/jobs/{{itemJson:$.id}}" },
                    "company": { "jsonPath": "$.details.team" },
                    "locations": [
                        { "jsonPath": "$.locations.location" }
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
                company: "Runtime Team".to_string(),
                url: "https://example.test/jobs/runtime-42".to_string(),
                locations: vec!["Berlin".to_string(), "Munich".to_string()],
                posting_meta: Default::default(),
            }]
        );
        assert_eq!(
            executor.client.requested_urls(),
            vec!["https://example.test/jobs.xml"]
        );
    });
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
                posting_meta: Default::default(),
            }]
        );
        assert_eq!(
            executor.client.requested_urls(),
            vec!["https://example.test/jobs.json"]
        );
    });
}

#[test]
fn json_inventory_extracts_reserved_posting_meta_for_candidates() {
    tauri::async_runtime::block_on(async {
        let fixture_client = FixtureInventoryHttpClient::new([(
            "https://example.test/jobs.json",
            Ok(r#"{
                  "jobs": [
                    {
                      "id": 4242,
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
                        { "jsonPath": "$.location" }
                    ],
                    "postingMeta": {
                        "jobId": { "jsonPath": "$.id" }
                    }
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

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].posting_meta,
            BTreeMap::from([("jobId".to_string(), "4242".to_string())])
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
                    posting_meta: Default::default(),
                },
                SourceCandidate {
                    title: "Frontend Engineer".to_string(),
                    company: "Fixture Careers".to_string(),
                    url: "https://example.test/jobs/frontend".to_string(),
                    locations: vec!["Hamburg".to_string()],
                    posting_meta: Default::default(),
                },
                SourceCandidate {
                    title: "Platform Engineer".to_string(),
                    company: "Fixture Careers".to_string(),
                    url: "https://example.test/jobs/platform".to_string(),
                    locations: vec!["Mainz".to_string()],
                    posting_meta: Default::default(),
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
                posting_meta: Default::default(),
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
fn personio_xml_inventory_source_runs_through_search_run_with_source_profile() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_builtin_profile_source(
            temp_dir.path(),
            "demo_ag",
            "Demo AG",
            "personio",
            "endpoint_inventory",
            json!({
                "boardSlug": "demo",
                "personioHost": "demo.jobs.personio.de",
                "language": "en",
                "startUrl": "https://demo.jobs.personio.de/"
            }),
        );
        let search_request =
            create_search_request(&pool, vec!["demo_ag".to_string()], "engineer").await;
        let fixture_client = FixtureInventoryHttpClient::new([(
            "https://demo.jobs.personio.de/xml?language=en",
            Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <workzag-jobs>
                  <position>
                    <id>4103</id>
                    <subcompany>Demo AG</subcompany>
                    <office>Munich</office>
                    <additionalOffices>
                      <office>Berlin</office>
                      <office>Hamburg</office>
                    </additionalOffices>
                    <department>Engineering</department>
                    <recruitingCategory>Engineering</recruitingCategory>
                    <name>Senior Rust Engineer</name>
                    <jobDescriptions>
                      <jobDescription>
                        <name>Description</name>
                        <value><![CDATA[Build reliable systems.]]></value>
                      </jobDescription>
                    </jobDescriptions>
                  </position>
                  <position>
                    <id>4104</id>
                    <office>Cologne</office>
                    <name>Sales Manager</name>
                  </position>
                </workzag-jobs>"#),
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
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Senior Rust Engineer");
        assert_eq!(posting.company, "Demo AG");
        assert_eq!(posting.url, "https://demo.jobs.personio.de/job/4103");
        assert_eq!(posting.locations, vec!["Munich", "Berlin", "Hamburg"]);
        assert_eq!(posting.sources[0].source_key, "demo_ag");
        assert_eq!(posting.sources[0].source_name, "Demo AG");
        assert_eq!(
            posting.sources[0].posting_meta,
            BTreeMap::from([("jobId".to_string(), "4103".to_string())])
        );
        assert_eq!(
            executor.client.requested_urls(),
            vec!["https://demo.jobs.personio.de/xml?language=en"]
        );
    });
}

#[test]
fn workday_sitemap_inventory_source_runs_through_search_run_with_source_profile() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_builtin_profile_source(
            temp_dir.path(),
            "acme_careers",
            "Acme Careers",
            "workday",
            "sitemap_inventory",
            json!({
                "startUrl": "https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers",
                "workdayHost": "acme.wd1.myworkdayjobs.com",
                "tenant": "acme",
                "site": "AcmeCareers",
                "sitemapUrl": "https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/siteMap.xml"
            }),
        );
        let search_request =
            create_search_request(&pool, vec!["acme_careers".to_string()], "engineer").await;
        let fixture_client = FixtureInventoryHttpClient::new([(
            "https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/siteMap.xml",
            Ok(r#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                  <url>
                    <loc>https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/job/Berlin/Senior-Engineer_R-123</loc>
                  </url>
                  <url>
                    <loc>https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/job/Munich/Sales-Manager_R-456</loc>
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
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Senior Engineer");
        assert_eq!(posting.company, "Acme Careers");
        assert_eq!(
            posting.url,
            "https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/job/Berlin/Senior-Engineer_R-123"
        );
        assert!(posting.locations.is_empty());
        assert_eq!(posting.sources[0].source_key, "acme_careers");
        assert_eq!(
            posting.sources[0].posting_meta,
            BTreeMap::from([(
                "jobId".to_string(),
                "/job/Berlin/Senior-Engineer_R-123".to_string()
            )])
        );
        assert_eq!(
            executor.client.requested_urls(),
            vec!["https://acme.wd1.myworkdayjobs.com/en-US/AcmeCareers/siteMap.xml"]
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
            create_search_request(&pool, vec!["example_magnolia".to_string()], "engineer").await;
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
fn muz_global_jobboard_inventory_runs_endpoint_fixture_through_central_runtime() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_builtin_profile_source(
            temp_dir.path(),
            "commerzbank",
            "Commerzbank",
            "muz_global_jobboard",
            "endpoint_inventory",
            json!({
                "startUrl": "https://jobs.commerzbank.com/index.php?ac=search_result",
                "apiBaseUrl": "https://api-jobs.commerzbank.com/",
                "configUrl": "https://jobs.commerzbank.com/assets/js/jobboard.config.json"
            }),
        );
        let search_request =
            create_search_request(&pool, vec!["commerzbank".to_string()], "praktikant").await;
        let fixture_client = FixtureInventoryHttpClient::new([(
            "https://api-jobs.commerzbank.com/search/",
            Ok(r#"{
                  "LanguageCode": "DE",
                  "SearchResult": {
                    "SearchResultCount": 1,
                    "SearchResultItems": [
                      {
                        "MatchedObjectId": "58810",
                        "MatchedObjectDescriptor": {
                          "PositionTitle": "Schülerpraktikant / Schülerpraktikantin in der Filiale",
                          "PositionURI": "https://jobs.commerzbank.com/index.php?ac=jobad&id=58810",
                          "PositionLocation": [
                            {
                              "CountryName": "Deutschland",
                              "CityName": "Hamburg",
                              "PostalCode": "22587"
                            }
                          ]
                        }
                      }
                    ]
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 1);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(
            posting.title,
            "Schülerpraktikant / Schülerpraktikantin in der Filiale"
        );
        assert_eq!(posting.company, "Commerzbank");
        assert_eq!(
            posting.url,
            "https://jobs.commerzbank.com/index.php?ac=jobad&id=58810"
        );
        assert_eq!(posting.locations, vec!["Hamburg, Deutschland"]);
        assert_eq!(posting.sources[0].source_key, "commerzbank");
        assert_eq!(posting.sources[0].source_name, "Commerzbank");
        assert_eq!(
            executor.client.requested_urls(),
            vec!["https://api-jobs.commerzbank.com/search/"]
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
            Some(
                "executionPlan.inventory must be a JSON object for source inventory_missing_source"
            )
        );
        assert!(executor.client.requested_urls().is_empty());
    });
}

#[test]
fn default_source_executor_routes_declarative_adapters_to_inventory_runtime() {
    tauri::async_runtime::block_on(async {
        let executor =
            DefaultSourceExecutor::new(tempfile::tempdir().unwrap().path().join("browser-runtime"));
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
        last_run_at: None,
        last_run_status: None,
        last_run_error: None,
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
