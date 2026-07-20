use super::*;

#[test]
fn compiled_discovery_runtime_reports_fetch_parse_select_and_extract_failures() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetch_failure = block_on(execute_discovery_test(&plan, &fake_fetcher([])));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_discovery_test(
        &plan,
        &fake_fetcher([("https://example.test/jobs.json", "{not-json".to_string())]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let select_plan = compiled_json_discovery_plan(
        default_fields(),
        json!({ "type": "json_path", "jsonPath": "$.jobs[*]" }),
    );
    let select_failure = block_on(execute_discovery_test(
        &select_plan,
        &fake_fetcher([(
            "https://example.test/jobs.json",
            json!({ "jobs": [] }).to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&select_failure.diagnostics[0], "json_path_select_failed");

    let mut fields = default_fields();
    fields["title"] =
        json!({ "type": "json_path", "jsonPath": "$.title[*]", "cardinality": "one" });
    let extract_plan = compiled_json_discovery_plan(fields, default_select());
    let extract_failure = block_on(execute_discovery_test(
        &extract_plan,
        &fake_fetcher([(
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

#[test]
fn discovery_url_render_failure_does_not_expose_authored_template() {
    const SECRET: &str = "raw-authored-discovery-secret";
    let mut plan = compiled_json_discovery_plan(default_fields(), default_select());
    let ExecutionPlanFetch::Http { url, .. } = &mut plan.discovery.strategies[0].fetch else {
        panic!("fixture must use HTTP fetch");
    };
    *url = job_radar_lib::compile_template(
        &format!("https://{SECRET}.example.test/{{{{unsupported:key}}}}"),
        &job_radar_lib::TemplateDescriptor::new().allow_namespace("unsupported", ["key"]),
    )
    .unwrap();

    let result = block_on(execute_discovery_test(&plan, &fake_fetcher([])));

    let diagnostic = &result.diagnostics[0];
    assert_runtime_diagnostic(diagnostic, "fetch_url_template_failed");
    let serialized = serde_json::to_string(diagnostic).unwrap();
    assert!(!serialized.contains(SECRET));
    assert_eq!(diagnostic.details, Some(json!({})));
}
