use super::*;

#[test]
fn compiled_posting_discovery_rejects_template_transform_pipes() {
    let mut fields = default_fields();
    fields["company"] = json!({
        "type": "template",
        "template": "{{sourceConfig:feedUrl|slugToTitle}}",
        "cardinality": "one"
    });

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "example_jobs",
        "name": "Example Jobs",
        "kind": "generic",
        "support": { "level": "experimental" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "json_feed",
            "name": "JSON feed",
            "postingDiscovery": {
                "strategies": [{
                    "key": "json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": { "fields": fields }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "example_source",
        "name": "Example Source",
        "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/jobs.json" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "example_source",
    );

    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "template_transform_pipes_unsupported"
            && diagnostic.category == DiagnosticCategory::Compiler
            && diagnostic.path
                == "/accessPaths/0/postingDiscovery/strategies/0/extract/fields/company/template"
    }));
}
