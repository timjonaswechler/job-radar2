use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, Acceptance, AccessPathFragment, CompileSourceOutcome, Fetch, FieldExpression,
    ListFieldExpression, Predicate, RegistrySourceProfile, ScriptedProfileHttpClient,
    SourceDocument, SourceExecutionPlan, SourceProfileDocument, SourceProfileRegistrySnapshot,
    SourceStatus,
};

#[derive(Debug)]
struct TestCompileResult {
    execution_plan: Option<SourceExecutionPlan>,
    diagnostics: job_radar_lib::Diagnostics,
}

fn compile_test_source(
    source: &SourceDocument,
    profile: Option<SourceProfileDocument>,
) -> TestCompileResult {
    let registry = SourceProfileRegistrySnapshot {
        profiles: profile
            .into_iter()
            .map(|document| RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document,
            })
            .collect(),
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };
    match compile_source(source, &registry) {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } => TestCompileResult {
            execution_plan: Some(source.execution_plan),
            diagnostics,
        },
        CompileSourceOutcome::Rejected { diagnostics } => TestCompileResult {
            execution_plan: None,
            diagnostics,
        },
    }
}

#[test]
fn compiler_rejects_acceptance_fields_and_thresholds_outside_the_phase_catalogue() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let mut detail_profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    detail_profile.access_paths[0]
        .detail
        .as_mut()
        .unwrap()
        .accept_when = Some(Acceptance {
        required_fields: None,
        min_description_length: None,
        min_results: Some(1),
    });
    let detail_result = compile_test_source(&source, Some(detail_profile));
    assert_eq!(detail_result.execution_plan, None);
    assert!(detail_result
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.path.ends_with("/detail/acceptWhen/minResults") }));

    let mut discovery_profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    discovery_profile.access_paths[0].discovery.accept_when = Some(Acceptance {
        required_fields: Some(vec!["postingMeta.unknown".into()]),
        min_description_length: None,
        min_results: None,
    });
    let discovery_result = compile_test_source(&source, Some(discovery_profile));
    assert_eq!(discovery_result.execution_plan, None);
    assert!(discovery_result.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .path
            .ends_with("/discovery/acceptWhen/requiredFields")
    }));
}

#[test]
fn compiler_validates_structural_capability_compatibility() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    profile.access_paths[0].discovery.strategies[0].select =
        serde_json::from_value(serde_json::json!({ "type": "css", "selector": ".job" })).unwrap();
    profile.access_paths[0].discovery.strategies[0]
        .extract
        .provider_values
        .as_mut()
        .unwrap()
        .title = Some(FieldExpression::CssText {
        selector: ".title".to_string(),
        cardinality: None,
        transforms: None,
    });

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "incompatible_parse_select_capability"));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "value_document_incompatible"));
}

#[test]
fn compiler_rejects_invalid_predicate_regex_during_plan_compilation() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    profile.access_paths[0].discovery.strategies[0].conditions =
        Some(vec![serde_json::from_value::<Predicate>(
            serde_json::json!({
                "type": "regex",
                "field": { "type": "json_path", "jsonPath": "$.title" },
                "pattern": "["
            }),
        )
        .unwrap()]);
    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "predicate_regex_invalid")
        .expect("predicate compile diagnostic");
    assert!(diagnostic
        .path
        .ends_with("/accessPaths/0/discovery/strategies/0/where/0/pattern"));
    assert_eq!(diagnostic.strategy_key.as_deref(), Some("json_api"));
}

#[test]
fn compiler_preserves_base_and_direct_capture_order_in_the_typed_plan() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    profile.access_paths[0].discovery.strategies[0].captures = Some(
        serde_json::from_str(
            r#"{
                "zeta": {
                    "from": { "type": "const", "value": "z" },
                    "pattern": "^(?<zeta>z)$"
                },
                "alpha": {
                    "from": { "type": "const", "value": "a" },
                    "pattern": "^(?<alpha>a)$"
                }
            }"#,
        )
        .unwrap(),
    );
    source.access_paths = Some(
        serde_json::from_str(
            r#"[{
                "key": "json_feed",
                "discovery": {
                    "strategies": [{
                        "key": "json_api",
                        "captures": {
                            "beta": {
                                "from": { "type": "const", "value": "b" },
                                "pattern": "^(?<beta>b)$"
                            },
                            "gamma": {
                                "from": { "type": "const", "value": "g" },
                                "pattern": "^(?<gamma>g)$"
                            }
                        }
                    }]
                }
            }]"#,
        )
        .unwrap(),
    );

    let result = compile_test_source(&source, Some(profile));
    let plan = result.execution_plan.expect("capture profile compiles");
    let keys = plan.discovery.strategies[0]
        .captures
        .as_ref()
        .unwrap()
        .rules()
        .iter()
        .map(|rule| rule.key())
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["zeta", "alpha", "beta", "gamma"]);
}

#[test]
fn compiler_rejects_invalid_capture_regex_and_missing_selected_group_before_execution() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    for (pattern, expected_code) in [
        ("(", "capture_pattern_invalid"),
        ("^(?<other>.+)$", "capture_named_group_missing"),
    ] {
        let mut profile: SourceProfileDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
        profile.access_paths[0].discovery.strategies[0].captures = Some(
            serde_json::from_value(serde_json::json!({
                "tenant": {
                    "from": { "type": "const", "value": "runtime-secret-sentinel" },
                    "pattern": pattern
                }
            }))
            .unwrap(),
        );

        let result = compile_test_source(&source, Some(profile));

        assert_eq!(result.execution_plan, None);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == expected_code)
            .expect("capture compiler diagnostic");
        assert!(diagnostic
            .path
            .ends_with("/discovery/strategies/0/captures/tenant/pattern"));
        assert_eq!(diagnostic.strategy_key.as_deref(), Some("json_api"));
        assert!(!serde_json::to_string(diagnostic)
            .unwrap()
            .contains("runtime-secret-sentinel"));
    }
}

#[test]
fn compiler_rejects_invalid_transform_plans_with_stable_context() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let mut unselected_path = profile.access_paths[0].clone();
    unselected_path.key = "invalid_unselected_path".to_string();
    unselected_path.name = "Invalid unselected path".to_string();
    unselected_path.discovery.strategies[0]
        .extract
        .provider_values
        .as_mut()
        .unwrap()
        .title = Some(
        serde_json::from_value(serde_json::json!({
            "type": "json_path",
            "jsonPath": "$.title",
            "transforms": [{
                "type": "regex_replace",
                "pattern": "(",
                "replacement": "x"
            }]
        }))
        .unwrap(),
    );
    profile.access_paths.push(unselected_path);

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "transform_invalid_regex")
        .expect("transform compile diagnostic");
    assert!(diagnostic.path.ends_with(
        "/accessPaths/1/discovery/strategies/0/extract/providerValues/title/transforms/0"
    ));
    assert_eq!(
        diagnostic.details,
        Some(serde_json::json!({ "transformIndex": 0 }))
    );
}

#[test]
fn compiler_enforces_the_four_value_context_placements_recursively() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let mut discovery_capture_profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    discovery_capture_profile.access_paths[0]
        .discovery
        .strategies[0]
        .captures = Some(
        serde_json::from_value(serde_json::json!({
            "slug": {
                "from": {
                    "type": "combine",
                    "parts": [{ "value": { "type": "capture", "key": "slug" } }]
                },
                "pattern": "(.*)"
            }
        }))
        .unwrap(),
    );
    let result = compile_test_source(&source, Some(discovery_capture_profile));
    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "value_capture_unavailable"
            && diagnostic
                .path
                .ends_with("/captures/slug/from/parts/0/value")
    }));

    let mut detail_capture_profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    detail_capture_profile.access_paths[0]
        .detail
        .as_mut()
        .unwrap()
        .strategies[0]
        .captures = Some(
        serde_json::from_value(serde_json::json!({
            "selected": {
                "from": { "type": "json_path", "jsonPath": "$.description" },
                "pattern": "(.*)"
            }
        }))
        .unwrap(),
    );
    let result = compile_test_source(&source, Some(detail_capture_profile));
    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "value_selected_item_unavailable"
            && diagnostic.path.ends_with("/captures/selected/from")
    }));

    let mut discovery_output_profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    discovery_output_profile.access_paths[0]
        .discovery
        .strategies[0]
        .extract
        .provider_values
        .as_mut()
        .unwrap()
        .title = Some(
        serde_json::from_value(serde_json::json!({
            "type": "posting_meta",
            "key": "jobId"
        }))
        .unwrap(),
    );
    let result = compile_test_source(&source, Some(discovery_output_profile));
    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "value_posting_meta_unavailable"
            && diagnostic.path.ends_with("/extract/providerValues/title")
    }));
}

#[test]
fn compiler_enforces_the_complete_effective_value_node_limit_once() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    profile.access_paths[0].discovery.strategies[0]
        .extract
        .provider_values
        .as_mut()
        .unwrap()
        .locations = Some(ListFieldExpression::Multiple(
        (0..1_025)
            .map(|index| FieldExpression::Const {
                value: serde_json::from_value(serde_json::json!(index)).unwrap(),
                cardinality: None,
                transforms: None,
            })
            .collect(),
    ));

    let result = compile_test_source(&source, Some(profile));

    assert!(result.execution_plan.is_none());
    let diagnostics = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "value_node_limit_exceeded")
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].strategy_key.as_deref(), Some("json_api"));
    assert_eq!(
        diagnostics[0].details,
        Some(serde_json::json!({ "actual": 1025, "maximum": 1024 }))
    );
}

#[test]
fn source_owned_plan_uses_declared_optional_source_config_keys() {
    let mut source_json: serde_json::Value =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    source_json["selectedAccessPath"]["sourceConfigSchema"]["properties"]["optionalTenant"] =
        serde_json::json!({ "type": "string" });
    source_json["selectedAccessPath"]["discovery"]["strategies"][0]["extract"]["providerValues"]
        ["title"] = serde_json::json!({
        "type": "template",
        "template": "{{sourceConfig:optionalTenant}}"
    });
    let source: SourceDocument = serde_json::from_value(source_json).unwrap();

    let result = compile_test_source(&source, None);

    assert!(result.execution_plan.is_some(), "{:?}", result.diagnostics);
}

#[test]
fn compiler_validates_template_variable_namespaces_keys_and_context() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    if let Fetch::Http { url, .. } = &mut profile.access_paths[0].discovery.strategies[0].fetch {
        *url = "{{posting:url}}?q={{sourceConfig:missing}}&x={{unknown:thing}}".to_string();
    }
    if let Fetch::Http { url, .. } =
        &mut profile.access_paths[0].detail.as_mut().unwrap().strategies[0].fetch
    {
        *url = "{{postingMeta:missingMeta}}".to_string();
    }

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    for expected_code in ["template_namespace_unavailable", "unknown_template_key"] {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "expected diagnostic code {expected_code}, got {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn compiler_validates_capabilities_after_direct_specialization() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(
        serde_json::from_value::<Vec<AccessPathFragment>>(serde_json::json!([{
            "key": "json_feed",
            "discovery": {
                "strategies": [{
                    "key": "json_api",
                    "parse": { "type": "html" }
                }]
            }
        }]))
        .unwrap(),
    );

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "incompatible_parse_select_capability"
                && diagnostic.path == "/accessPaths/0/discovery/strategies/0/select"
                && diagnostic.strategy_key.as_deref() == Some("json_api")
        }),
        "got diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn compiler_rejects_invalid_profile_schema_before_validating_source_config() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_config.remove("feedUrl");
    source
        .source_config
        .insert("radius".to_string(), serde_json::json!(25));
    profile
        .source_config_schema
        .as_mut()
        .unwrap()
        .get_mut("properties")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "keywords".to_string(),
            serde_json::json!({ "type": "string" }),
        );

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "forbidden_search_criteria_in_source_config_schema"
    }));
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "missing_source_config_required_property"
            && diagnostic.code != "forbidden_search_criteria_in_source_config"
    }));
}

#[test]
fn compiler_rejects_invalid_sitemap_select_in_unselected_access_path() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let mut profile = serde_json::to_value(profile).unwrap();
    let mut access_path = profile["accessPaths"][0].clone();
    access_path["key"] = serde_json::json!("invalid_unselected_sitemap");
    let strategy = &mut access_path["discovery"]["strategies"][0];
    strategy["parse"] = serde_json::json!({ "type": "xml" });
    strategy["select"] = serde_json::json!({ "type": "document" });
    strategy["pagination"] = serde_json::json!({
        "type": "sitemap",
        "postingUrlSelector": { "type": "sitemap_urls", "urlPattern": "[" },
        "limits": { "maxRequests": 1, "maxItems": 10 }
    });
    strategy["extract"] = serde_json::json!({
        "reference": {
            "url": { "type": "item_field", "key": "value", "cardinality": "one" }
        },
        "providerValues": {
            "title": { "type": "const", "value": "Sitemap posting" },
            "company": { "type": "const", "value": "Example" }
        }
    });
    profile["accessPaths"]
        .as_array_mut()
        .unwrap()
        .push(access_path);
    let profile: SourceProfileDocument = serde_json::from_value(profile).unwrap();
    let fetcher = ScriptedProfileHttpClient::new([]);

    let unselected = compile_test_source(&source, Some(profile.clone()));
    assert_invalid_sitemap_diagnostic(&unselected);

    let mut selected_source = serde_json::to_value(source).unwrap();
    selected_source["selectedAccessPath"]["pathKey"] =
        serde_json::json!("invalid_unselected_sitemap");
    let selected_source: SourceDocument = serde_json::from_value(selected_source).unwrap();
    let selected = compile_test_source(&selected_source, Some(profile));
    assert_invalid_sitemap_diagnostic(&selected);

    assert_eq!(fetcher.request_count(), 0);
}

fn assert_invalid_sitemap_diagnostic(result: &TestCompileResult) {
    assert!(result.execution_plan.is_none());
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "invalid_select_syntax"
                && diagnostic.path
                    == "/accessPaths/1/discovery/strategies/0/pagination/postingUrlSelector/urlPattern"
        }),
        "got diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn compiler_validates_required_support_metadata() {
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    source.status = SourceStatus::Active;
    source.source_support = None;

    let missing_source_support = compile_test_source(&source, None);

    assert_eq!(missing_source_support.execution_plan, None);
    assert!(missing_source_support.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "missing_source_support"
            && diagnostic.path == "/sourceSupport"
            && diagnostic.details.as_ref().unwrap()["sourceKey"] == "owned_source"
    }));
}

#[test]
fn compiler_reports_duplicate_strategy_keys_within_each_step() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let duplicate_discovery = profile.access_paths[0].discovery.strategies[0].clone();
    profile.access_paths[0]
        .discovery
        .strategies
        .push(duplicate_discovery);
    let duplicate_detail = profile.access_paths[0].detail.as_ref().unwrap().strategies[0].clone();
    profile.access_paths[0]
        .detail
        .as_mut()
        .unwrap()
        .strategies
        .push(duplicate_detail);

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "duplicate_strategy_key"
            && diagnostic.path == "/accessPaths/0/discovery/strategies/1/key"
            && diagnostic.strategy_key.as_deref() == Some("json_api")
    }));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "duplicate_strategy_key"
            && diagnostic.path == "/accessPaths/0/detail/strategies/1/key"
            && diagnostic.strategy_key.as_deref() == Some("detail_api")
    }));
}

#[test]
fn compiler_reports_duplicate_reusable_access_path_keys() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let duplicate = profile.access_paths[0].clone();
    profile.access_paths.push(duplicate);

    let result = compile_test_source(&source, Some(profile));

    assert_eq!(result.execution_plan, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "duplicate_access_path_key")
        .expect("duplicate reusable Access Path key should be diagnosed");
    assert_eq!(diagnostic.path, "/accessPaths/1/key");
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["accessPathKey"],
        "json_feed"
    );
}

fn read_fixture<T>(relative_path: &str) -> T
where
    T: serde::de::DeserializeOwned,
{
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to deserialize {}: {error}", path.display()))
}
