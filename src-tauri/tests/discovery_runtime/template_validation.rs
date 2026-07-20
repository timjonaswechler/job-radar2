use super::*;

#[test]
fn compiled_discovery_rejects_template_transform_pipes() {
    let mut fields = default_fields();
    fields["company"] = json!({
        "type": "template",
        "template": "{{sourceConfig:feedUrl|slugToTitle}}",
        "cardinality": "one"
    });

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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
            "discovery": {
                "policy": { "type": "first_accepted" },
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
        "schemaVersion": 3,
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

    let result = compile_test_source(&source, Some(profile));
    let CompileSourceOutcome::Rejected { diagnostics } = result else {
        panic!("invalid template must reject compilation: {result:?}");
    };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "template_transform_pipes_unsupported"
            && diagnostic.category == DiagnosticCategory::Compiler
            && diagnostic.path
                == "/accessPaths/0/discovery/strategies/0/extract/fields/company/template"
    }));
}
