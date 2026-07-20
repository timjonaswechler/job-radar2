use super::*;

#[test]
fn compiled_discovery_runtime_extracts_xml_posting_fields() {
    let fields = json!({
        "title": { "type": "xml_text", "textPath": "title", "cardinality": "one" },
        "company": { "type": "xml_text", "textPath": "company", "cardinality": "one" },
        "url": { "type": "xml_text", "textPath": "url", "cardinality": "one" },
        "locations": { "type": "xml_element", "element": "location", "cardinality": "all" },
        "postingMeta": {
            "jobId": { "type": "xml_text", "textPath": "id", "cardinality": "one" }
        }
    });
    let plan = compiled_discovery_plan(
        json!({ "type": "xml" }),
        json!({ "type": "xml_element", "element": "job" }),
        fields,
        "https://example.test/jobs.xml",
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.xml",
        r#"<jobs>
            <job>
              <id> 42 </id>
              <title> Senior   Rust
Engineer </title>
              <company> Example GmbH </company>
              <url> https://example.test/jobs/42 </url>
              <locations><location> Berlin </location><location>Berlin</location><location> Remote </location></locations>
            </job>
        </jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(result.candidates[0].url, "https://example.test/jobs/42");
    assert_eq!(result.candidates[0].locations, vec!["Berlin", "Remote"]);
    assert_eq!(result.candidates[0].posting_meta["jobId"], "42");
}

#[test]
fn compiled_discovery_runtime_extracts_html_posting_fields_with_css() {
    let fields = json!({
        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
        "company": { "type": "css_text", "selector": ".company", "cardinality": "one" },
        "url": { "type": "css_attribute", "selector": "a.apply", "attribute": "href", "cardinality": "one" },
        "locations": { "type": "css_text", "selector": ".location", "cardinality": "all" }
    });
    let plan = compiled_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        fields,
        "https://example.test/jobs.html",
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.html",
        r#"<html><body>
            <article class="posting">
              <h2 class="title"> Staff   Frontend
Engineer </h2>
              <span class="company"> Example GmbH </span>
              <a class="apply" href="https://example.test/jobs/frontend">Apply</a>
              <span class="location"> Berlin </span><span class="location">Remote</span>
            </article>
        </body></html>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Staff Frontend Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(
        result.candidates[0].url,
        "https://example.test/jobs/frontend"
    );
    assert_eq!(result.candidates[0].locations, vec!["Berlin", "Remote"]);
}

#[test]
fn compiled_discovery_runtime_uses_browser_fetch_rendered_html() {
    let fields = json!({
        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
        "company": { "type": "css_text", "selector": ".company", "cardinality": "one" },
        "url": { "type": "css_attribute", "selector": "a.apply", "attribute": "href", "cardinality": "one" }
    });
    let plan = compiled_browser_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        fields,
        "https://example.test/rendered?tenant=acme",
    );
    let fetcher = FakeFetcher::new(std::iter::empty());
    let browser = FakeBrowser::new([(
        "https://example.test/rendered?tenant=acme",
        r#"<html><body>
            <article class="posting">
              <h2 class="title"> Browser Rendered Engineer </h2>
              <span class="company"> Example GmbH </span>
              <a class="apply" href="https://example.test/jobs/browser">Apply</a>
            </article>
        </body></html>"#
            .to_string(),
    )]);

    let result = block_on(execute_discovery_with_clients(&plan, &fetcher, &browser));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Browser Rendered Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(
        result.candidates[0].url,
        "https://example.test/jobs/browser"
    );
    assert!(fetcher.requests().is_empty());
    let browser_requests = browser.requests();
    assert_eq!(browser_requests.len(), 1);
    assert_eq!(
        browser_requests[0].url,
        "https://example.test/rendered?tenant=acme"
    );
    assert_eq!(browser_requests[0].timeout_ms, 30_000);
    assert_eq!(
        browser_requests[0].waits,
        vec![
            ExecutionPlanBrowserWait::Selector {
                selector: Some("article.posting".to_string()),
                timeout_ms: 5000,
            },
            ExecutionPlanBrowserWait::NetworkIdle {
                selector: None,
                timeout_ms: 250,
            },
        ]
    );
    assert_eq!(
        browser_requests[0].interactions,
        vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: "button.load-more".to_string(),
            max_count: 2,
            wait_after_ms: Some(250),
        }]
    );
}

#[test]
fn compiled_discovery_runtime_reports_browser_fetch_diagnostics() {
    let plan = compiled_browser_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        default_html_fields(),
        "https://example.test/rendered",
    );
    let fetcher = FakeFetcher::new(std::iter::empty());
    let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::WaitTimeout {
            wait_index: Some(0),
        },
        "selector .posting did not appear",
    ));

    let result = block_on(execute_discovery_with_clients(&plan, &fetcher, &browser));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "browser_wait_timeout");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/fetch/waits/0"
    );
}

#[test]
fn compiled_discovery_runtime_reports_xml_and_html_diagnostics() {
    let xml_plan = compiled_discovery_plan(
        json!({ "type": "xml" }),
        json!({ "type": "xml_element", "element": "job" }),
        default_xml_fields(),
        "https://example.test/jobs.xml",
    );
    let xml_parse_failure = block_on(execute_discovery_with_fetcher(
        &xml_plan,
        &FakeFetcher::new([("https://example.test/jobs.xml", "<jobs><job>".to_string())]),
    ));
    assert_runtime_diagnostic(&xml_parse_failure.diagnostics[0], "xml_parse_failed");
    assert_eq!(
        xml_parse_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/parse"
    );

    let html_select_plan = compiled_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "[" }),
        default_html_fields(),
        "https://example.test/jobs.html",
    );
    let html_select_failure = block_on(execute_discovery_with_fetcher(
        &html_select_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.html",
            "<article></article>".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&html_select_failure.diagnostics[0], "css_select_failed");
    assert_eq!(
        html_select_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/select/selector"
    );

    let mut html_fields = default_html_fields();
    html_fields["title"] = json!({ "type": "css_text", "selector": "[", "cardinality": "one" });
    let html_extract_plan = compiled_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article" }),
        html_fields,
        "https://example.test/jobs.html",
    );
    let html_extract_failure = block_on(execute_discovery_with_fetcher(
        &html_extract_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.html",
            "<article><a class='apply' href='https://example.test/jobs/1'></a><span class='company'>Example GmbH</span></article>".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(
        &html_extract_failure.diagnostics[0],
        "field_css_selector_failed",
    );
    assert_eq!(
        html_extract_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/extract/fields/title"
    );
}
