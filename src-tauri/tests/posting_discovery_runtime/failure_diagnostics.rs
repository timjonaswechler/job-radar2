use super::*;

#[test]
fn compiled_posting_discovery_runtime_reports_fetch_parse_select_and_extract_failures() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
    let fetch_failure = block_on(execute_posting_discovery_with_fetcher(
        &plan,
        &FakeFetcher::new([]),
    ));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_posting_discovery_with_fetcher(
        &plan,
        &FakeFetcher::new([("https://example.test/jobs.json", "{not-json".to_string())]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let select_plan = compiled_json_posting_discovery_plan(
        default_fields(),
        json!({ "type": "json_path", "jsonPath": "$.jobs[*]" }),
    );
    let select_failure = block_on(execute_posting_discovery_with_fetcher(
        &select_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.json",
            json!({ "jobs": [] }).to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&select_failure.diagnostics[0], "json_path_select_failed");

    let mut fields = default_fields();
    fields["title"] =
        json!({ "type": "json_path", "jsonPath": "$.title[*]", "cardinality": "one" });
    let extract_plan = compiled_json_posting_discovery_plan(fields, default_select());
    let extract_failure = block_on(execute_posting_discovery_with_fetcher(
        &extract_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.json",
            json!({
                "jobs": [{
                    "title": "Rust Engineer",
                    "company": "Example GmbH",
                    "url": "https://example.test/jobs/1"
                }]
            })
            .to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&extract_failure.diagnostics[0], "field_json_path_failed");
    assert!(extract_failure.candidates.is_empty());
}
