use super::*;

fn pagination_source_config() -> serde_json::Map<String, Value> {
    serde_json::from_value(json!({ "feedUrl": "https://example.test/rendered?tenant=acme" }))
        .unwrap()
}

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
                "totalPath": "$.total",
                "limits": { "maxRequests": 2 }
            }),
        )]),
    );
    let fetcher = fake_fetcher([
        (
            "https://example.test/jobs.json?page=1&per_page=2",
            json!({
                "total": 3,
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?page=2&per_page=2",
            json!({
                "total": 3,
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[1]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Frontend Engineer"
    );
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
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json?page=1",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = budget_exhausted(block_on(execute_discovery(
        &plan,
        &pagination_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        fetcher.requests().len(),
        1,
        "one-over request must not start"
    );
    assert_eq!(result.report.usage.requests, 1);
    assert_eq!(result.report.usage.pages, 1);
    assert_eq!(result.report.usage.produced_items, 0);
    let PhaseCompletion::BudgetExhausted { exhaustion } = result.report.completion else {
        panic!("expected request exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
    assert_eq!(exhaustion.requested, 1);
    assert_eq!(exhaustion.remaining, 0);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "pagination_max_requests_reached"));
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
    let fetcher = fake_fetcher([
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
    assert_eq!(result.payload.candidates.len(), 2);
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
    let fetcher = fake_fetcher([(
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

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[1]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Frontend Engineer"
    );
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
fn max_items_exact_fit_stops_after_inserting_item_without_fetching_later_page() {
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
    let fetcher = fake_fetcher([
        (
            "https://example.test/jobs.json?page=1",
            json!({ "jobs": [
                { "title": "One", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ] })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?page=2",
            json!({ "jobs": [
                { "title": "Two", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
            ] })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(fetcher.requests().len(), 2);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "pagination_max_items_reached");
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
    let fetcher = fake_fetcher([(
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
    assert_eq!(result.payload.candidates.len(), 2);
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
    let fetcher = fake_fetcher([(
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

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "pagination_max_items_reached");
    assert_eq!(
        result
            .payload
            .candidates
            .into_iter()
            .map(|candidate| candidate.reference.provider_url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs/1".to_string(),
            "https://example.test/jobs/2".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_uses_all_root_locations_without_omitted_child_traversal() {
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
                "limits": { "maxRequests": 2, "maxItems": 10, "maxDepth": 1 }
            }),
        )]),
    );
    let fetcher = fake_fetcher([(
        "https://example.test/root-sitemap.xml",
        r#"<sitemapindex>
            <sitemap><loc> https://example.test/jobs-sitemap.xml </loc></sitemap>
            <url><loc>https://example.test/jobs/1</loc></url>
            <url><loc>   </loc></url>
        </sitemapindex>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result
            .payload
            .candidates
            .into_iter()
            .map(|candidate| candidate.reference.provider_url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs-sitemap.xml".to_string(),
            "https://example.test/jobs/1".to_string(),
        ]
    );
    assert_eq!(fetcher.request_count(), 1);
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
    let fetcher = fake_fetcher([
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
    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(
        result.payload.candidates[0].reference.provider_url,
        "https://example.test/jobs/1"
    );
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
    let fetcher = fake_fetcher([(
        "https://example.test/root-sitemap.xml",
        r#"<sitemapindex>
            <sitemap><loc>https://example.test/jobs-sitemap.xml</loc></sitemap>
        </sitemapindex>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.payload.candidates.is_empty());
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
    let fetcher = fake_fetcher([(
        "https://example.test/root-sitemap.xml",
        r#"<sitemapindex>
            <sitemap><loc>https://example.test/jobs-sitemap.xml</loc></sitemap>
        </sitemapindex>"#
            .to_string(),
    )]);

    let result = budget_exhausted(block_on(execute_discovery(
        &plan,
        &pagination_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        fetcher.requests().len(),
        1,
        "child fetch is denied before effect"
    );
    assert_eq!(result.report.usage.requests, 1);
    assert_eq!(result.report.usage.pages, 1);
    let PhaseCompletion::BudgetExhausted { exhaustion } = result.report.completion else {
        panic!("expected request exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "pagination_max_requests_reached"));
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
    let fetcher = fake_fetcher([
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
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[1]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Frontend Engineer"
    );
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
    assert!(result.diagnostics.is_empty());
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
    let missing_cursor_fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);
    let empty_cursor_fetcher = fake_fetcher([(
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
    assert_eq!(missing_result.payload.candidates.len(), 1);
    assert_eq!(missing_cursor_fetcher.requests().len(), 1);
    assert_eq!(empty_result.diagnostics, Vec::new());
    assert_eq!(empty_result.payload.candidates.len(), 1);
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
    let fetcher = fake_fetcher([
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

    assert_eq!(result.payload.candidates.len(), 2);
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
    let fetcher = fake_fetcher([(
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

    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
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
                "totalPath": "$.total",
                "limits": { "maxRequests": 2 }
            }),
        )]),
    );
    let fetcher = fake_fetcher([
        (
            "https://example.test/jobs.json?tenant=acme&offset=0&limit=2",
            json!({
                "total": 4,
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/jobs.json?tenant=acme&offset=2&limit=2",
            json!({
                "total": 4,
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[1]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Frontend Engineer"
    );
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
    let page = json!({
        "total": 4,
        "jobs": [
            { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
        ]
    })
    .to_string();
    let fetcher = fake_fetcher([
        ("https://example.test/jobs.json", page.clone()),
        ("https://example.test/jobs.json", page),
    ]);

    let source_config =
        serde_json::from_value(json!({ "feedUrl": "https://example.test/jobs.json" })).unwrap();
    let result = block_on(execute_discovery_test_with_config(
        &plan,
        &source_config,
        &fetcher,
    ));

    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(
        result.report.usage.produced_items, 1,
        "R02 charges only the final reduced occurrence, not duplicate selected candidates"
    );
    assert_eq!(
        result
            .payload
            .provenance
            .iter()
            .find(|evidence| matches!(
                evidence.responsibility,
                job_radar_lib::DiscoveryResponsibility::Url
            ))
            .expect("duplicate occurrence URL provenance")
            .contributors
            .len(),
        2
    );
    let requests = fetcher.requests();
    assert_eq!(requests[0].url, "https://example.test/jobs.json");
    assert_eq!(requests[1].url, "https://example.test/jobs.json");
    let first_body = requests[0].body.as_ref().expect("first rendered JSON body");
    assert_eq!(
        first_body.bytes(),
        br#"{"appliedFacets":{},"limit":2,"offset":0}"#
    );
    assert_eq!(first_body.default_content_type(), Some("application/json"));
    let second_body = requests[1]
        .body
        .as_ref()
        .expect("second rendered JSON body");
    assert_eq!(
        second_body.bytes(),
        br#"{"appliedFacets":{},"limit":2,"offset":2}"#
    );
    assert_eq!(second_body.default_content_type(), Some("application/json"));
}

#[test]
fn json_body_pagination_overlay_rejects_non_post_json_fetch_before_io() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "pagination".to_string(),
        json!({
            "type": "offset_limit",
            "offsetParam": "offset",
            "limitParam": "limit",
            "parameterLocation": "json_body",
            "limit": 2,
            "limits": { "maxRequests": 1 }
        }),
    );
    let outcome = compile_discovery_outcome_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );

    let CompileSourceOutcome::Rejected { diagnostics } = outcome else {
        panic!("json_body pagination on GET must reject during compilation")
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path.ends_with("/pagination/parameterLocation")
            && diagnostic.message.contains("HTTP POST JSON-body")
    }));
}
