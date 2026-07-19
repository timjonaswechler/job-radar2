use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, CompileSourceOutcome, CompiledSourceAccess, DiagnosticSeverity, Fetch,
    RegistrySource, RegistrySourceProfile, SourceDocument, SourceProfileDocument,
    SourceProfileRegistrySnapshot, SourceStatus, SourceValidationState, ValidationStateKind,
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
            .posting_discovery
            .strategies[0]
            .accept_when
            .as_ref()
            .and_then(|acceptance| acceptance.min_results),
        Some(0),
        "the complete Effective Source Profile must include existing Source Overrides"
    );
    assert_eq!(compiled.execution_plan.source.key, "example_source");
    assert_eq!(
        compiled.execution_plan.posting_discovery.strategies[0]
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
                "zeta": { "type": "string" },
                "roles": { "type": "string" },
                "keywords": { "type": "string" },
                "alpha": { "type": "string" }
            },
            "additionalProperties": false
        }))
        .expect("object fixture should deserialize"),
    );
    profile.access_paths[0].source_config_schema = Some(
        serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "zeta": { "type": "string" },
                "radius": { "type": "string" },
                "country": { "type": "string" },
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

    let redefinition_paths = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "source_config_schema_property_redefinition")
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        redefinition_paths,
        vec![
            "/accessPaths/0/sourceConfigSchema/properties/alpha",
            "/accessPaths/0/sourceConfigSchema/properties/zeta",
        ]
    );

    let forbidden_paths = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "forbidden_search_criteria_in_source_config_schema")
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        forbidden_paths,
        vec![
            "/sourceConfigSchema/properties/keywords",
            "/sourceConfigSchema/properties/roles",
            "/accessPaths/0/sourceConfigSchema/properties/country",
            "/accessPaths/0/sourceConfigSchema/properties/radius",
        ]
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
