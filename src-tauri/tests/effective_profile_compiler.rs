use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, AccessPathFragment, CompileSourceOutcome, CompiledSourceAccess,
    DiagnosticCategory, DiagnosticSeverity, Fetch, RegistrySource, RegistrySourceProfile,
    SelectedAccessPath, SourceDocument, SourceProfileDocument, SourceProfileRegistrySnapshot,
    SourceStatus, SourceValidationState, ValidationStateKind,
};

#[test]
fn profile_source_compiles_to_a_complete_effective_profile_and_plan() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let registry = registry_with_profile(profile);

    let CompileSourceOutcome::Compiled {
        source: compiled,
        diagnostics,
    } = compile_source(&source, &registry)
    else {
        panic!("valid profile Source should compile");
    };

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error));
    let CompiledSourceAccess::Profile { effective_profile } = compiled.access else {
        panic!("profile Source should expose an Effective Source Profile");
    };
    assert_eq!(effective_profile.document.key, "example_jobs");
    assert_eq!(effective_profile.document.access_paths.len(), 1);
    assert_eq!(
        effective_profile.document.access_paths[0]
            .discovery
            .strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(0),
        "direct specialization must be reflected in the Effective Source Profile"
    );
    assert_eq!(compiled.execution_plan.source.key, "example_source");
    assert_eq!(
        compiled.execution_plan.discovery.strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(0)
    );
}

#[test]
fn compiler_validates_the_complete_effective_profile_before_building_a_plan() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut invalid_unselected_path = profile.access_paths[0].clone();
    invalid_unselected_path.key = "invalid_unselected_path".to_string();
    invalid_unselected_path.name = "Invalid unselected path".to_string();
    let Fetch::Http { timeout_ms, .. } = &mut invalid_unselected_path.discovery.strategies[0].fetch
    else {
        panic!("fixture should use HTTP fetch");
    };
    *timeout_ms = None;
    profile.access_paths.push(invalid_unselected_path);
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("an invalid unselected path must reject the complete Effective Source Profile");
    };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "missing_fetch_timeout" && diagnostic.path.starts_with("/accessPaths/1/")
    }));
}

#[test]
fn compiler_recursively_specializes_existing_entries_without_reordering_or_mutating_inputs() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut second_path = profile.access_paths[0].clone();
    second_path.key = "second_path".to_string();
    second_path.name = "Second path".to_string();
    second_path.discovery.strategies[0].key = "second_strategy".to_string();
    profile.access_paths.push(second_path);
    let original_profile = profile.clone();

    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "second_path",
            "discovery": {
                "strategies": [{
                    "key": "second_strategy",
                    "acceptWhen": { "minResults": 0 }
                }]
            }
        },
        {
            "key": "json_feed",
            "discovery": {
                "acceptWhen": { "minResults": 0 },
                "strategies": [{
                    "key": "json_api",
                    "fetch": { "headers": { "x-source": "specialized" } },
                    "acceptWhen": { "requiredFields": ["url"] }
                }]
            }
        }
    ])));
    let original_source = source.clone();

    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = compile_source(&source, &registry_with_profile(profile.clone()))
    else {
        panic!("valid existing-entry specialization should compile");
    };

    let CompiledSourceAccess::Profile { effective_profile } = compiled.access else {
        panic!("profile Source should expose an Effective Source Profile");
    };
    assert_eq!(
        effective_profile
            .document
            .access_paths
            .iter()
            .map(|path| path.key.as_str())
            .collect::<Vec<_>>(),
        vec!["json_feed", "second_path"]
    );
    let first_strategy = &effective_profile.document.access_paths[0]
        .discovery
        .strategies[0];
    assert_eq!(
        effective_profile.document.access_paths[0]
            .discovery
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(0)
    );
    let Fetch::Http { headers, .. } = &first_strategy.fetch else {
        panic!("fixture should retain HTTP fetch");
    };
    assert_eq!(headers.as_ref().unwrap()["accept"], "application/json");
    assert_eq!(headers.as_ref().unwrap()["x-source"], "specialized");
    assert_eq!(
        first_strategy
            .accept_when
            .as_ref()
            .unwrap()
            .required_fields
            .as_ref()
            .unwrap(),
        &vec!["url".to_string()],
        "unkeyed arrays must replace as a whole"
    );
    assert_eq!(
        first_strategy.accept_when.as_ref().unwrap().min_results,
        Some(1),
        "unsupplied object siblings must remain inherited"
    );
    assert_eq!(profile, original_profile);
    assert_eq!(source, original_source);
}

#[test]
fn compiler_appends_complete_new_strategies_and_paths_in_fragment_order() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_config.remove("language");
    let base_strategy =
        serde_json::to_value(&profile.access_paths[0].discovery.strategies[0]).unwrap();
    let mut first_strategy = base_strategy.clone();
    first_strategy["key"] = serde_json::json!("first_new");
    let mut second_strategy = base_strategy;
    second_strategy["key"] = serde_json::json!("second_new");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "discovery": {
                "strategies": [second_strategy, first_strategy]
            }
        },
        {
            "key": "new_path",
            "name": "New path",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [
                    serde_json::to_value(&profile.access_paths[0].discovery.strategies[0]).unwrap()
                ]
            }
        }
    ])));
    source.selected_access_path = SelectedAccessPath::ProfileAccessPath {
        profile_key: "example_jobs".to_string(),
        path_key: "new_path".to_string(),
    };

    let outcome = compile_source(&source, &registry_with_profile(profile));
    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = &outcome
    else {
        panic!("complete additions should compile: {outcome:?}");
    };
    let CompiledSourceAccess::Profile { effective_profile } = &compiled.access else {
        panic!("expected profile access");
    };
    assert_eq!(
        effective_profile
            .document
            .access_paths
            .iter()
            .map(|path| path.key.as_str())
            .collect::<Vec<_>>(),
        vec!["json_feed", "new_path"]
    );
    assert_eq!(
        effective_profile.document.access_paths[0]
            .discovery
            .strategies
            .iter()
            .map(|strategy| strategy.key.as_str())
            .collect::<Vec<_>>(),
        vec!["json_api", "second_new", "first_new"]
    );
    assert_eq!(compiled.execution_plan.discovery.strategies.len(), 1);
}

#[test]
fn compiler_rejects_incomplete_additions_with_sorted_missing_fields() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "discovery": {
                "strategies": [
                    { "key": "incomplete_one", "fetch": { "mode": "http" } },
                    { "key": "incomplete_two", "select": { "type": "document" } }
                ]
            }
        },
        { "key": "incomplete_path" },
        {
            "key": "incomplete_steps",
            "name": "Incomplete steps",
            "discovery": {},
            "detail": {}
        }
    ])));

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("incomplete additions must reject the complete compilation");
    };
    let completeness = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "incomplete_profile_fragment_addition")
        .map(|diagnostic| {
            (
                diagnostic.path.as_str(),
                diagnostic.details.as_ref().unwrap()["missingFields"].clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        completeness,
        vec![
            (
                "/accessPaths/0/discovery/strategies/0",
                serde_json::json!(["extract", "fetch.timeoutMs", "fetch.url", "parse", "select"]),
            ),
            (
                "/accessPaths/0/discovery/strategies/1",
                serde_json::json!(["extract", "fetch", "parse"]),
            ),
            ("/accessPaths/1", serde_json::json!(["discovery", "name"]),),
            (
                "/accessPaths/2/discovery",
                serde_json::json!(["strategies"]),
            ),
            ("/accessPaths/2/detail", serde_json::json!(["strategies"]),),
        ]
    );
}

#[test]
fn effective_profile_rechecks_value_nodes_after_direct_specialization_merge() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let locations = (0..1_024)
        .map(|index| serde_json::json!({ "type": "const", "value": index }))
        .collect::<Vec<_>>();
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "extract": { "fields": { "locations": locations } }
            }]
        }
    }])));

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("post-merge Value node overflow must reject compilation");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "value_node_limit_exceeded")
        .expect("post-merge Value node diagnostic");
    assert_eq!(diagnostic.strategy_key.as_deref(), Some("json_api"));
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["maximum"],
        serde_json::json!(1_024)
    );
}

#[test]
fn compiler_reports_each_duplicate_fragment_key_at_its_real_pointer() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "discovery": {
                "strategies": [
                    { "key": "json_api" },
                    { "key": "json_api" },
                    { "key": "json_api" }
                ]
            }
        },
        { "key": "json_feed" },
        { "key": "json_feed" }
    ])));

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("duplicate fragments must reject compilation");
    };
    let duplicate_paths = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "duplicate_profile_fragment_key")
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        duplicate_paths,
        vec![
            "/accessPaths/1/key",
            "/accessPaths/2/key",
            "/accessPaths/0/discovery/strategies/1/key",
            "/accessPaths/0/discovery/strategies/2/key",
        ]
    );
}

#[test]
fn compiler_rejects_an_invalid_unselected_added_path_before_source_config_validation() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_config.remove("feedUrl");
    let mut strategy =
        serde_json::to_value(&profile.access_paths[0].discovery.strategies[0]).unwrap();
    strategy["fetch"] = serde_json::json!({
        "mode": "browser",
        "url": "https://example.test/jobs",
        "timeoutMs": 10000,
        "interactions": [{ "type": "execute_script", "script": "return 1" }]
    });
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "invalid_unselected",
        "name": "Invalid unselected path",
        "discovery": {
            "policy": { "type": "first_accepted" },
            "strategies": [strategy]
        }
    }])));

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("an invalid unselected addition must reject the complete profile");
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "prohibited_browser_behavior"
            && diagnostic.path.starts_with("/accessPaths/1/")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "missing_source_config_required_property"),
        "Source Config validation must not run after Effective Source Profile rejection"
    );
}

#[test]
fn compiler_is_deterministic_for_equivalent_fragment_object_orders() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let registry = registry_with_profile(profile);

    let outcomes = [
        r#"[{"key":"json_feed","discovery":{"acceptWhen":{"minResults":0},"strategies":[{"key":"json_api","acceptWhen":{"minResults":0,"requiredFields":["url"]}}]}}]"#,
        r#"[{"discovery":{"strategies":[{"acceptWhen":{"requiredFields":["url"],"minResults":0},"key":"json_api"}],"acceptWhen":{"minResults":0}},"key":"json_feed"}]"#,
    ]
    .map(|json| {
        let mut source = source.clone();
        source.access_paths = Some(serde_json::from_str(json).unwrap());
        compile_source(&source, &registry)
    });

    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn final_source_shape_rejects_the_legacy_specialization_model() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let mut source: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    source["accessPaths"] = serde_json::json!([{ "key": "json_feed" }]);
    source["sourceOverrides"] = serde_json::json!({});

    let error = serde_json::from_value::<SourceDocument>(source)
        .expect_err("the final Source shape must not admit legacy sourceOverrides");

    assert!(error.to_string().contains("sourceOverrides"));
}

#[test]
fn directly_supplied_source_is_authoritative_over_same_key_registry_source() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let mut conflicting_source = source.clone();
    conflicting_source.name = "Registry impostor".to_string();
    conflicting_source.source_config.insert(
        "feedUrl".to_string(),
        serde_json::json!("https://wrong.example/jobs.json"),
    );
    let mut registry = registry_with_profile(profile);
    registry.sources.push(registry_source(conflicting_source));

    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = compile_source(&source, &registry)
    else {
        panic!("the directly supplied Source should compile");
    };

    assert_eq!(compiled.execution_plan.source.name, "Example Source");
    let serialized_plan = serde_json::to_string(&compiled.execution_plan).unwrap();
    assert!(!serialized_plan.contains("https://wrong.example/jobs.json"));
    assert!(!serialized_plan.contains("https://example.test/jobs.json"));
}

#[test]
fn source_lifecycle_is_not_part_of_compilation() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    let registry = registry_with_profile(profile);

    let outcomes = [
        SourceStatus::Draft,
        SourceStatus::Active,
        SourceStatus::Disabled,
    ]
    .map(|status| {
        let mut source = source.clone();
        source.status = status;
        compile_source(&source, &registry)
    });

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[1], outcomes[2]);
}

#[test]
fn rejection_diagnostics_have_deterministic_key_order() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.source_config_schema = Some(
        serde_json::from_value(serde_json::json!({
            "type": "object",
            "required": ["zeta", "alpha"],
            "properties": {
                "feedUrl": { "type": "string" },
                "zeta": { "type": "string" },
                "alpha": { "type": "string" }
            },
            "additionalProperties": false
        }))
        .expect("object fixture should deserialize"),
    );
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("invalid Source Config must reject compilation");
    };
    let missing_paths = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_source_config_required_property")
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        missing_paths,
        vec!["/sourceConfig/alpha", "/sourceConfig/zeta"]
    );
}

#[test]
fn compiler_enforces_all_effective_source_config_value_constraints() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.source_config_schema = Some(
        serde_json::json!({
            "type": "object",
            "required": ["feedUrl", "region", "priority", "largeMinimum"],
            "additionalProperties": false,
            "properties": {
                "feedUrl": { "type": "string", "format": "uri", "title": "Feed URL" },
                "region": { "type": "string", "enum": ["eu", "us"], "pattern": "^[a-z]{2}$" },
                "priority": { "type": "integer", "minimum": 1 },
                "largeMinimum": { "type": "integer", "minimum": 9007199254740993_u64 },
                "enabled": { "type": "boolean" },
                "metadata": { "type": "object" },
                "tags": { "type": "array" },
                "nothing": { "type": "null" }
            }
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_config.extend(
        serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(serde_json::json!({
            "region": "eu",
            "priority": 1,
            "largeMinimum": 9007199254740993_u64,
            "enabled": true,
            "metadata": {},
            "tags": [],
            "nothing": null
        }))
        .unwrap(),
    );

    assert!(matches!(
        compile_source(&source, &registry_with_profile(profile.clone())),
        CompileSourceOutcome::Compiled { .. }
    ));

    for (property, value, expected_code) in [
        (
            "feedUrl",
            serde_json::json!("relative/jobs"),
            "invalid_source_config_property_uri",
        ),
        (
            "region",
            serde_json::json!("ap"),
            "invalid_source_config_property_enum",
        ),
        (
            "priority",
            serde_json::json!(0),
            "invalid_source_config_property_minimum",
        ),
        (
            "largeMinimum",
            serde_json::json!(9007199254740992_u64),
            "invalid_source_config_property_minimum",
        ),
        (
            "enabled",
            serde_json::json!("yes"),
            "invalid_source_config_property_type",
        ),
    ] {
        let mut invalid = source.clone();
        invalid.source_config.insert(property.to_string(), value);
        let CompileSourceOutcome::Rejected { diagnostics } =
            compile_source(&invalid, &registry_with_profile(profile.clone()))
        else {
            panic!("invalid {property} must reject compilation");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.category == DiagnosticCategory::SourceValidation
                    && diagnostic.code == expected_code
                    && diagnostic.path == format!("/sourceConfig/{property}")
            }),
            "missing {expected_code}: {diagnostics:?}"
        );
    }
}

#[test]
fn compiler_rejects_malformed_contract_definitions_deterministically() {
    let cases = [
        (
            serde_json::json!({ "properties": { "feedUrl": { "type": "mystery" } } }),
            "invalid_source_config_property_schema_type",
            "/sourceConfigSchema/properties/feedUrl/type",
        ),
        (
            serde_json::json!({ "properties": { "feedUrl": { "type": "string", "pattern": "[" } } }),
            "invalid_source_config_schema_pattern",
            "/sourceConfigSchema/properties/feedUrl/pattern",
        ),
        (
            serde_json::json!({ "required": ["missing", "missing"], "properties": {} }),
            "duplicate_source_config_schema_required_property",
            "/sourceConfigSchema/required/1",
        ),
        (
            serde_json::json!({ "properties": { "feedUrl": { "type": "string", "default": "x" } } }),
            "unsupported_source_config_property_schema_keyword",
            "/sourceConfigSchema/properties/feedUrl/default",
        ),
    ];

    for (schema, expected_code, expected_path) in cases {
        let mut profile: SourceProfileDocument =
            read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
        profile.source_config_schema = Some(schema.as_object().unwrap().clone());
        let source: SourceDocument = read_fixture(
            "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
        );
        let CompileSourceOutcome::Rejected { diagnostics } =
            compile_source(&source, &registry_with_profile(profile))
        else {
            panic!("malformed contract must reject compilation");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.category == DiagnosticCategory::Compiler
                    && diagnostic.code == expected_code
                    && diagnostic.path == expected_path
            }),
            "missing {expected_code} at {expected_path}: {diagnostics:?}"
        );
    }
}

#[test]
fn direct_source_schema_specialization_replaces_arrays_and_preserves_profile_title() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    profile.access_paths[0].source_config_schema = Some(
        serde_json::json!({
            "type": "object",
            "required": ["region"],
            "properties": {
                "language": { "type": "string" },
                "region": { "type": "string", "enum": ["eu", "us"], "title": "Region" }
            }
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source
        .source_config
        .insert("region".to_string(), serde_json::json!("eu"));
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "sourceConfigSchema": {
            "required": [],
            "properties": { "region": { "enum": ["eu"] } }
        }
    }])));

    let outcome = compile_source(&source, &registry_with_profile(profile.clone()));
    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = outcome
    else {
        panic!("same-location schema specialization should compile: {outcome:?}");
    };
    let CompiledSourceAccess::Profile { effective_profile } = compiled.access else {
        panic!("expected Effective Source Profile");
    };
    let region = &effective_profile.document.access_paths[0]
        .source_config_schema
        .as_ref()
        .unwrap()["properties"]["region"];
    assert_eq!(region["title"], "Region");
    assert_eq!(region["enum"], serde_json::json!(["eu"]));
    assert_eq!(
        effective_profile.document.access_paths[0]
            .source_config_schema
            .as_ref()
            .unwrap()["required"],
        serde_json::json!([])
    );

    source
        .source_config
        .insert("region".to_string(), serde_json::json!("us"));
    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("replaced enum must be executable");
    };
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_source_config_property_enum"));
}

#[test]
fn direct_and_source_owned_schema_titles_are_rejected() {
    let direct = serde_json::from_value::<AccessPathFragment>(serde_json::json!({
        "key": "json_feed",
        "sourceConfigSchema": {
            "properties": { "region": { "type": "string", "title": "Region" } }
        }
    }));
    assert!(direct
        .unwrap_err()
        .to_string()
        .contains("title is not authorable"));

    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");
    let SelectedAccessPath::SourceOwnedAccessPath {
        source_config_schema,
        ..
    } = &mut source.selected_access_path
    else {
        unreachable!()
    };
    source_config_schema
        .as_mut()
        .unwrap()
        .get_mut("properties")
        .unwrap()
        .get_mut("startUrl")
        .unwrap()["title"] = serde_json::json!("Start URL");
    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("Source-owned title must reject compilation");
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "source_config_schema_title_not_allowed"
            && diagnostic.path == "/selectedAccessPath/sourceConfigSchema/properties/startUrl/title"
    }));
}

#[test]
fn source_owned_access_is_a_distinct_complete_branch() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-owned-access-path.json");

    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = compile_source(&source, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("valid Source-owned access should compile regardless of lifecycle");
    };

    let CompiledSourceAccess::SourceOwned { access_path } = compiled.access else {
        panic!("Source-owned access must not fabricate an Effective Source Profile");
    };
    assert_eq!(access_path.key, "html_page");
    assert_eq!(access_path.name, "HTML page");
    assert_eq!(access_path.discovery.strategies[0].key, "html_cards");
    assert_eq!(compiled.execution_plan.source.key, "owned_source");
}

#[test]
fn rejection_exposes_diagnostics_and_no_partial_compiled_source() {
    let source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("missing profile must reject compilation");
    };

    assert!(!diagnostics.is_empty());
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error));
    assert_eq!(diagnostics[0].code, "source_profile_not_found");
}

fn fragments(value: serde_json::Value) -> Vec<AccessPathFragment> {
    serde_json::from_value(value).expect("valid typed Access Path fragments")
}

fn registry_with_profile(profile: SourceProfileDocument) -> SourceProfileRegistrySnapshot {
    SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile,
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn registry_source(document: SourceDocument) -> RegistrySource {
    let source_key = document.key.clone();
    RegistrySource {
        origin: "test".into(),
        path: String::new(),
        document,
        validation_state: SourceValidationState {
            source_key,
            state: ValidationStateKind::Valid,
            can_compile: true,
            can_execute: true,
            diagnostics: Vec::new(),
        },
        effective_profile: None,
        compile_outcome: None,
    }
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
