use super::*;

#[test]
fn compiled_discovery_runtime_reports_fetch_parse_select_and_extract_failures() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetch_failure = block_on(execute_discovery_failed_test(&plan, &fake_fetcher([])));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_discovery_failed_test(
        &plan,
        &fake_fetcher([("https://example.test/jobs.json", "{not-json".to_string())]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({ "jobs": [] }).to_string(),
    )]);
    let select_failure = compile_discovery_outcome(
        json!({ "type": "json" }),
        json!({ "type": "json_path", "jsonPath": "$.jobs[*]" }),
        default_fields(),
        "https://example.test/jobs.json",
    );
    let CompileSourceOutcome::Rejected { diagnostics } = select_failure else {
        panic!("invalid JSONPath Select should be rejected before execution");
    };
    assert_eq!(diagnostics[0].category, DiagnosticCategory::Compiler);
    assert_eq!(diagnostics[0].code, "invalid_select_syntax");
    assert_eq!(fetcher.request_count(), 0);

    let mut fields = default_fields();
    fields["title"] =
        json!({ "type": "json_path", "jsonPath": "$.title[*]", "cardinality": "one" });
    let extract_failure = compile_discovery_outcome(
        json!({ "type": "json" }),
        default_select(),
        fields,
        "https://example.test/jobs.json",
    );
    let CompileSourceOutcome::Rejected { diagnostics } = extract_failure else {
        panic!("invalid Value selector should be rejected before execution");
    };
    assert_eq!(diagnostics[0].code, "value_selector_syntax_invalid");
}

#[test]
fn strict_decode_terminal_exposes_no_document_or_parse_diagnostic() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs.json".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![0xff])],
        content_length: None,
    }]);

    let result = block_on(execute_discovery_failed_test(&plan, &fetcher));

    assert_eq!(fetcher.requests().len(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "fetch_failed");
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.code.ends_with("_parse_failed")));
}

#[test]
fn non_success_http_status_exposes_no_discovery_payload_or_parse_result() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 404,
        final_url: "https://example.test/jobs.json".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({
                "jobs": [{
                    "title": "Must not escape",
                    "company": "Example GmbH",
                    "url": "https://example.test/jobs/404"
                }]
            })
            .to_string()
            .into_bytes(),
        )],
        content_length: None,
    }]);

    let result = block_on(execute_discovery_failed_test(&plan, &fetcher));

    assert_eq!(fetcher.request_count(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "http_fetch_non_success_status");
    assert_eq!(
        result.diagnostics[0].details,
        Some(json!({ "method": "GET", "status": 404 }))
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.code.ends_with("_parse_failed")));
    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::PolicyUnsatisfied);
    assert_eq!(report.usage.requests, 1);
}

#[test]
fn discovery_url_render_failure_does_not_expose_authored_template() {
    const SECRET: &str = "raw-authored-discovery-secret";
    let mut plan = compiled_json_discovery_plan(default_fields(), default_select());
    let ExecutionPlanFetch::Http(fetch) = &mut plan.discovery.strategies[0].fetch else {
        panic!("fixture must use HTTP fetch");
    };
    fetch.url = job_radar_lib::compile_template(
        &format!("https://{SECRET}.example.test/{{{{unsupported:key}}}}"),
        &job_radar_lib::TemplateDescriptor::new().allow_namespace("unsupported", ["key"]),
    )
    .unwrap();

    let result = block_on(execute_discovery_failed_test(&plan, &fake_fetcher([])));

    let diagnostic = &result.diagnostics[0];
    assert_runtime_diagnostic(diagnostic, "fetch_url_template_failed");
    let serialized = serde_json::to_string(diagnostic).unwrap();
    assert!(!serialized.contains(SECRET));
    assert_eq!(diagnostic.details, Some(json!({})));
}
