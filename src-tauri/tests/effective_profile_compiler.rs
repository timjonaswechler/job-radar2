use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, AccessPathFragment, CompileSourceOutcome, CompiledSourceAccess,
    DiagnosticSeverity, Fetch, RegistrySource, RegistrySourceProfile, SourceDocument,
    SourceProfileDocument, SourceProfileRegistrySnapshot, SourceStatus, SourceValidationState,
    ValidationStateKind,
};

#[test]
fn profile_source_compiles_to_a_complete_effective_profile_and_plan() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_overrides = None;
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
            .posting_discovery
            .strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(1),
        "without specialization, the Effective Source Profile must preserve the base profile"
    );
    assert_eq!(compiled.execution_plan.source.key, "example_source");
    assert_eq!(
        compiled.execution_plan.posting_discovery.strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(1)
    );
}

#[test]
fn compiler_validates_the_complete_effective_profile_before_building_a_plan() {
    let mut profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut invalid_unselected_path = profile.access_paths[0].clone();
    invalid_unselected_path.key = "invalid_unselected_path".to_string();
    invalid_unselected_path.name = "Invalid unselected path".to_string();
    let Fetch::Http { timeout_ms, .. } =
        &mut invalid_unselected_path.posting_discovery.strategies[0].fetch
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
    second_path.posting_discovery.strategies[0].key = "second_strategy".to_string();
    profile.access_paths.push(second_path);
    let original_profile = profile.clone();

    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_overrides = None;
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "second_path",
            "postingDiscovery": {
                "strategies": [{
                    "key": "second_strategy",
                    "acceptWhen": { "minResults": 0 }
                }]
            }
        },
        {
            "key": "json_feed",
            "postingDiscovery": {
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
        .posting_discovery
        .strategies[0];
    assert_eq!(
        effective_profile.document.access_paths[0]
            .posting_discovery
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
    source.source_overrides = None;
    source.source_config.remove("language");
    let base_strategy =
        serde_json::to_value(&profile.access_paths[0].posting_discovery.strategies[0]).unwrap();
    let mut first_strategy = base_strategy.clone();
    first_strategy["key"] = serde_json::json!("first_new");
    let mut second_strategy = base_strategy;
    second_strategy["key"] = serde_json::json!("second_new");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "postingDiscovery": {
                "strategies": [second_strategy, first_strategy]
            }
        },
        {
            "key": "new_path",
            "name": "New path",
            "postingDiscovery": {
                "strategies": [
                    serde_json::to_value(&profile.access_paths[0].posting_discovery.strategies[0]).unwrap()
                ]
            }
        }
    ])));
    source.selected_access_path = job_radar_lib::SelectedAccessPath::ProfileAccessPath {
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
            .posting_discovery
            .strategies
            .iter()
            .map(|strategy| strategy.key.as_str())
            .collect::<Vec<_>>(),
        vec!["json_api", "second_new", "first_new"]
    );
    assert_eq!(
        compiled.execution_plan.posting_discovery.strategies.len(),
        1
    );
}

#[test]
fn compiler_rejects_incomplete_additions_with_sorted_missing_fields() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_overrides = None;
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "postingDiscovery": {
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
            "postingDiscovery": {},
            "postingDetail": {}
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
                "/accessPaths/0/postingDiscovery/strategies/0",
                serde_json::json!(["extract", "fetch.timeoutMs", "fetch.url", "parse", "select"]),
            ),
            (
                "/accessPaths/0/postingDiscovery/strategies/1",
                serde_json::json!(["extract", "fetch", "parse"]),
            ),
            (
                "/accessPaths/1",
                serde_json::json!(["name", "postingDiscovery"]),
            ),
            (
                "/accessPaths/2/postingDiscovery",
                serde_json::json!(["strategies"]),
            ),
            (
                "/accessPaths/2/postingDetail",
                serde_json::json!(["strategies"]),
            ),
        ]
    );
}

#[test]
fn compiler_reports_each_duplicate_fragment_key_at_its_real_pointer() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_overrides = None;
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "postingDiscovery": {
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
            "/accessPaths/0/postingDiscovery/strategies/1/key",
            "/accessPaths/0/postingDiscovery/strategies/2/key",
        ]
    );
}

#[test]
fn compiler_rejects_an_invalid_unselected_added_path_before_source_config_validation() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.source_overrides = None;
    source.source_config.remove("feedUrl");
    let mut strategy =
        serde_json::to_value(&profile.access_paths[0].posting_discovery.strategies[0]).unwrap();
    strategy["fetch"] = serde_json::json!({
        "mode": "browser",
        "url": "https://example.test/jobs",
        "timeoutMs": 10000,
        "interactions": [{ "type": "execute_script", "script": "return 1" }]
    });
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "invalid_unselected",
        "name": "Invalid unselected path",
        "postingDiscovery": { "strategies": [strategy] }
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
        r#"[{"key":"json_feed","postingDiscovery":{"acceptWhen":{"minResults":0},"strategies":[{"key":"json_api","acceptWhen":{"minResults":0,"requiredFields":["url"]}}]}}]"#,
        r#"[{"postingDiscovery":{"strategies":[{"acceptWhen":{"requiredFields":["url"],"minResults":0},"key":"json_api"}],"acceptWhen":{"minResults":0}},"key":"json_feed"}]"#,
    ]
    .map(|json| {
        let mut source = source.clone();
        source.source_overrides = None;
        source.access_paths = Some(serde_json::from_str(json).unwrap());
        compile_source(&source, &registry)
    });

    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn compiler_rejects_mixed_legacy_and_direct_specialization_models() {
    let profile: SourceProfileDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json");
    let mut source: SourceDocument =
        read_fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([{ "key": "json_feed" }])));

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &registry_with_profile(profile))
    else {
        panic!("mixed specialization models must be rejected without precedence rules");
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        "conflicting_source_specialization_models"
    );
    assert_eq!(diagnostics[0].path, "/accessPaths");
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
    assert_eq!(
        compiled.execution_plan.source_config["feedUrl"],
        "https://example.test/jobs.json"
    );
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
    assert_eq!(
        access_path.posting_discovery.strategies[0].key,
        "html_cards"
    );
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
            origin: "test".to_string(),
            path: "test-profile.json".to_string(),
            document: profile,
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn registry_source(document: SourceDocument) -> RegistrySource {
    RegistrySource {
        origin: "test".to_string(),
        path: "test-source.json".to_string(),
        validation_state: SourceValidationState {
            source_key: document.key.clone(),
            state: ValidationStateKind::Unknown,
            can_compile: false,
            can_execute: false,
            diagnostics: Vec::new(),
        },
        document,
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
