use super::*;

#[test]
fn compiled_discovery_runtime_executes_bounded_page_pagination() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "page",
                "pageParam": "page",
                "firstPage": 1,
                "pageSizeParam": "per_page",
                "pageSize": 2,
                "limits": { "maxRequests": 2 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/jobs.json?page=1&per_page=2",
            json!({
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?page=2&per_page=2",
            json!({
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs.json?page=1&per_page=2".to_string(),
            "https://example.test/jobs.json?page=2&per_page=2".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_reports_max_requests_limit() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "page",
                "pageParam": "page",
                "limits": { "maxRequests": 1 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json?page=1",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "pagination_max_requests_reached"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxRequests"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("json_api")
    );
}

#[test]
fn compiled_discovery_runtime_stops_page_pagination_when_total_path_is_exhausted() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "page",
                "pageParam": "page",
                "pageSize": 1,
                "totalPath": "$.total",
                "limits": { "maxRequests": 5 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/jobs.json?page=1",
            json!({
                "total": 2,
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?page=2",
            json!({
                "total": 2,
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(fetcher.requests().len(), 2);
}

#[test]
fn compiled_discovery_runtime_reports_max_items_limit() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "page",
                "pageParam": "page",
                "limits": { "maxRequests": 5, "maxItems": 2 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json?page=1",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" },
                { "title": "Platform Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/3" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "pagination_max_items_reached");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxItems"
    );
}

#[test]
fn compiled_discovery_runtime_stops_offset_limit_pagination_when_total_path_is_exhausted() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "offset_limit",
                "offsetParam": "offset",
                "limitParam": "limit",
                "limit": 2,
                "totalPath": "$.total",
                "limits": { "maxRequests": 5 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json?offset=0&limit=2",
        json!({
            "total": 2,
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(fetcher.requests().len(), 1);
}

#[test]
fn compiled_discovery_runtime_extracts_posting_urls_from_sitemap_xml() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "xml" }),
        json!({ "type": "document" }),
        json!({
            "title": { "type": "const", "value": "Discovered from sitemap" },
            "company": { "type": "const", "value": "Example GmbH" },
            "url": { "type": "item_field", "key": "value", "cardinality": "one" }
        }),
        "https://example.test/sitemap.xml",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "sitemap",
                "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "/jobs/" },
                "limits": { "maxRequests": 1, "maxItems": 2 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/sitemap.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset>
            <url><loc>https://example.test/jobs/1</loc></url>
            <url><loc>https://example.test/about</loc></url>
            <url><loc>https://example.test/jobs/2</loc></url>
        </urlset>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result
            .candidates
            .into_iter()
            .map(|candidate| candidate.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs/1".to_string(),
            "https://example.test/jobs/2".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_follows_child_sitemaps_within_max_depth() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "xml" }),
        json!({ "type": "document" }),
        json!({
            "title": { "type": "const", "value": "Discovered from sitemap" },
            "company": { "type": "const", "value": "Example GmbH" },
            "url": { "type": "item_field", "key": "value", "cardinality": "one" }
        }),
        "https://example.test/root-sitemap.xml",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "sitemap",
                "childSitemapSelector": { "type": "sitemap_urls", "urlPattern": "sitemap" },
                "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "/jobs/" },
                "limits": { "maxRequests": 2, "maxItems": 10, "maxDepth": 1 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/root-sitemap.xml",
            r#"<sitemapindex>
                <sitemap><loc>https://example.test/jobs-sitemap.xml</loc></sitemap>
            </sitemapindex>"#
                .to_string(),
        ),
        (
            "https://example.test/jobs-sitemap.xml",
            r#"<urlset>
                <url><loc>https://example.test/jobs/1</loc></url>
            </urlset>"#
                .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].url, "https://example.test/jobs/1");
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/root-sitemap.xml".to_string(),
            "https://example.test/jobs-sitemap.xml".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_reports_sitemap_max_depth_limit() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "xml" }),
        json!({ "type": "document" }),
        json!({
            "title": { "type": "const", "value": "Discovered from sitemap" },
            "company": { "type": "const", "value": "Example GmbH" },
            "url": { "type": "item_field", "key": "value", "cardinality": "one" }
        }),
        "https://example.test/root-sitemap.xml",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "sitemap",
                "childSitemapSelector": { "type": "sitemap_urls", "urlPattern": "sitemap" },
                "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "/jobs/" },
                "limits": { "maxRequests": 5, "maxDepth": 0 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/root-sitemap.xml",
        r#"<sitemapindex>
            <sitemap><loc>https://example.test/jobs-sitemap.xml</loc></sitemap>
        </sitemapindex>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "pagination_max_depth_reached");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxDepth"
    );
}

#[test]
fn compiled_discovery_runtime_reports_sitemap_max_requests_limit() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "xml" }),
        json!({ "type": "document" }),
        json!({
            "title": { "type": "const", "value": "Discovered from sitemap" },
            "company": { "type": "const", "value": "Example GmbH" },
            "url": { "type": "item_field", "key": "value", "cardinality": "one" }
        }),
        "https://example.test/root-sitemap.xml",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "sitemap",
                "childSitemapSelector": { "type": "sitemap_urls", "urlPattern": "sitemap" },
                "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "/jobs/" },
                "limits": { "maxRequests": 1, "maxDepth": 1 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/root-sitemap.xml",
        r#"<sitemapindex>
            <sitemap><loc>https://example.test/jobs-sitemap.xml</loc></sitemap>
        </sitemapindex>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "pagination_max_requests_reached"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxRequests"
    );
}

#[test]
fn compiled_discovery_runtime_executes_bounded_cursor_pagination() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json?tenant=acme",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "cursor",
                "cursorParam": "cursor",
                "nextCursorPath": "$.nextCursor",
                "limits": { "maxRequests": 2 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/jobs.json?tenant=acme",
            json!({
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ],
                "nextCursor": "page-2"
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?tenant=acme&cursor=page-2",
            json!({
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ],
                "nextCursor": "page-3"
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs.json?tenant=acme".to_string(),
            "https://example.test/jobs.json?tenant=acme&cursor=page-2".to_string(),
        ]
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "pagination_max_requests_reached"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxRequests"
    );
}

#[test]
fn compiled_discovery_runtime_stops_cursor_pagination_when_next_cursor_is_missing_or_empty() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "cursor",
                "cursorParam": "cursor",
                "nextCursorPath": "$.nextCursor",
                "limits": { "maxRequests": 5 }
            }),
        )]),
    );
    let missing_cursor_fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);
    let empty_cursor_fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ],
            "nextCursor": "   "
        })
        .to_string(),
    )]);

    let missing_result = block_on(execute_discovery_test(&plan, &missing_cursor_fetcher));
    let empty_result = block_on(execute_discovery_test(&plan, &empty_cursor_fetcher));

    assert_eq!(missing_result.diagnostics, Vec::new());
    assert_eq!(missing_result.candidates.len(), 1);
    assert_eq!(missing_cursor_fetcher.requests().len(), 1);
    assert_eq!(empty_result.diagnostics, Vec::new());
    assert_eq!(empty_result.candidates.len(), 1);
    assert_eq!(empty_cursor_fetcher.requests().len(), 1);
}

#[test]
fn compiled_discovery_runtime_reports_duplicate_cursor_loop() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "cursor",
                "cursorParam": "cursor",
                "nextCursorPath": "$.nextCursor",
                "limits": { "maxRequests": 5 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/jobs.json",
            json!({
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ],
                "nextCursor": "page-2"
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?cursor=page-2",
            json!({
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ],
                "nextCursor": "page-2"
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(fetcher.requests().len(), 2);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "pagination_duplicate_cursor");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/nextCursorPath"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("json_api")
    );
}

#[test]
fn compiled_discovery_runtime_reports_cursor_max_items_limit() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "cursor",
                "cursorParam": "cursor",
                "nextCursorPath": "$.nextCursor",
                "limits": { "maxRequests": 5, "maxItems": 1 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
            ],
            "nextCursor": "page-2"
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "pagination_max_items_reached");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/pagination/limits/maxItems"
    );
    assert_eq!(
        result.diagnostics[0].details.as_ref().unwrap()["paginationType"],
        "cursor"
    );
}

#[test]
fn compiled_discovery_runtime_executes_bounded_offset_limit_pagination() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json?tenant=acme",
        serde_json::Map::from_iter([(
            "pagination".to_string(),
            json!({
                "type": "offset_limit",
                "offsetParam": "offset",
                "limitParam": "limit",
                "startOffset": 0,
                "limit": 2,
                "limits": { "maxRequests": 2 }
            }),
        )]),
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/jobs.json?tenant=acme&offset=0&limit=2",
            json!({
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?tenant=acme&offset=2&limit=2",
            json!({
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs.json?tenant=acme&offset=0&limit=2".to_string(),
            "https://example.test/jobs.json?tenant=acme&offset=2&limit=2".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_can_place_offset_limit_pagination_in_json_body() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "fetch".to_string(),
        json!({
            "mode": "http",
            "method": "POST",
            "url": "{{sourceConfig:feedUrl}}",
            "headers": {
                "accept": "application/json",
                "content-type": "application/json"
            },
            "body": {
                "type": "json",
                "value": {
                    "appliedFacets": {},
                    "limit": 0,
                    "offset": 0
                }
            },
            "timeoutMs": 10000
        }),
    );
    extra.insert(
        "pagination".to_string(),
        json!({
            "type": "offset_limit",
            "offsetParam": "offset",
            "limitParam": "limit",
            "parameterLocation": "json_body",
            "startOffset": 0,
            "limit": 2,
            "totalPath": "$.total",
            "limits": { "maxRequests": 2 }
        }),
    );
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "total": 4,
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    let requests = fetcher.requests();
    assert_eq!(requests[0].url, "https://example.test/jobs.json");
    assert_eq!(requests[1].url, "https://example.test/jobs.json");
    assert_eq!(
        requests[0].body,
        Some(RequestBody::Json {
            value: serde_json::Map::from_iter([
                ("appliedFacets".to_string(), json!({})),
                ("limit".to_string(), json!(2)),
                ("offset".to_string(), json!(0)),
            ])
        })
    );
    assert_eq!(
        requests[1].body,
        Some(RequestBody::Json {
            value: serde_json::Map::from_iter([
                ("appliedFacets".to_string(), json!({})),
                ("limit".to_string(), json!(2)),
                ("offset".to_string(), json!(2)),
            ])
        })
    );
}
