use std::{collections::BTreeMap, fs, path::Path};

use job_radar_lib::{
    compile_source, prepare_source_behavior_fingerprints, CompileSourceOutcome,
    RegistrySourceProfile, ReusableAccessPathDocument, SelectedAccessPath, SourceDocument,
    SourceProfileDocument, SourceProfileRegistrySnapshot, SourceRuntimeBinding,
};

#[test]
fn profile_success_prepares_the_closed_order_and_optional_runtime_binding() {
    let mut profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let job_radar_lib::Fetch::Http { url, .. } =
        &mut profile.access_paths[0].discovery.strategies[0].fetch
    else {
        panic!("fixture uses HTTP fetch")
    };
    *url = "https://example.test/{{source:name}}".to_string();
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.name = "Fingerprint source".to_string();
    let registry = registry_with_profile(profile.clone());
    let outcome = compile_source(&source, &registry);

    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = &outcome
    else {
        panic!("fixture must compile")
    };
    assert_eq!(
        compiled.runtime_binding_dependencies.bindings,
        vec![SourceRuntimeBinding::Name]
    );

    let fingerprints =
        prepare_source_behavior_fingerprints(&source, Some(&profile), &outcome).unwrap();
    assert_eq!(fingerprints.len(), 13);
    assert_eq!(
        fingerprints
            .iter()
            .map(|fingerprint| (
                fingerprint.kind.as_str(),
                fingerprint.reference.as_deref().unwrap()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("source_behavior", "base_source_profile"),
            ("source_behavior", "direct_source_specialization"),
            ("source_behavior", "effective_source_profile"),
            ("source_behavior", "compiler_provenance"),
            ("source_behavior", "source_config"),
            ("source_behavior", "selected_access_path"),
            ("source_behavior", "source_runtime_bindings"),
            ("behavior_version", "profile_compiler"),
            ("behavior_version", "profile_runtime"),
            ("behavior_version", "immutable_globals"),
            (
                "immutable_global_behavior",
                "source_live_check_pagination_smoke_budget",
            ),
            (
                "immutable_global_behavior",
                "compiler_max_fallback_strategies",
            ),
            (
                "immutable_global_behavior",
                "security_forbidden_request_key_behavior",
            ),
        ]
    );
    assert!(fingerprints.iter().all(|fingerprint| {
        fingerprint.sha256.as_ref().is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
    }));
}

#[test]
fn all_success_and_rejection_branches_have_the_authoritative_counts() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.access_paths = None;
    assert_behavior_order(
        &prepare_profile(&source, &profile),
        &[
            "base_source_profile",
            "effective_source_profile",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
        ],
    );

    let mut direct = source.clone();
    direct.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "discovery": { "acceptWhen": { "minResults": 1 } }
    }])));
    assert_behavior_order(
        &prepare_profile(&direct, &profile),
        &[
            "base_source_profile",
            "direct_source_specialization",
            "effective_source_profile",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
        ],
    );

    let mut binding_profile = profile.clone();
    set_selected_fetch_url(&mut binding_profile, "https://example.test/{{source:name}}");
    assert_behavior_order(
        &prepare_profile(&source, &binding_profile),
        &[
            "base_source_profile",
            "effective_source_profile",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
            "source_runtime_bindings",
        ],
    );
    assert_behavior_order(
        &prepare_profile(&direct, &binding_profile),
        &[
            "base_source_profile",
            "direct_source_specialization",
            "effective_source_profile",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
            "source_runtime_bindings",
        ],
    );

    let owned: SourceDocument = read_fixture("valid/source-owned-access-path.json");
    assert_behavior_order(
        &prepare_owned(&owned),
        &[
            "source_owned_access_path",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
        ],
    );
    let mut owned_binding = owned.clone();
    set_owned_fetch_url(&mut owned_binding, "https://example.test/{{source:name}}");
    assert_behavior_order(
        &prepare_owned(&owned_binding),
        &[
            "source_owned_access_path",
            "compiler_provenance",
            "source_config",
            "selected_access_path",
            "source_runtime_bindings",
        ],
    );

    let mut rejected_profile = profile.clone();
    rejected_profile.access_paths[0]
        .discovery
        .strategies
        .clear();
    let rejected = compile_source(&source, &registry_with_profile(rejected_profile.clone()));
    assert!(matches!(rejected, CompileSourceOutcome::Rejected { .. }));
    assert_behavior_order(
        &prepare_source_behavior_fingerprints(&source, Some(&rejected_profile), &rejected).unwrap(),
        &[
            "base_source_profile",
            "source_config",
            "selected_access_path",
        ],
    );
    let rejected_direct = compile_source(&direct, &registry_with_profile(rejected_profile.clone()));
    assert_behavior_order(
        &prepare_source_behavior_fingerprints(&direct, Some(&rejected_profile), &rejected_direct)
            .unwrap(),
        &[
            "base_source_profile",
            "direct_source_specialization",
            "source_config",
            "selected_access_path",
        ],
    );

    let unresolved = compile_source(&source, &SourceProfileRegistrySnapshot::default());
    assert_behavior_order(
        &prepare_source_behavior_fingerprints(&source, None, &unresolved).unwrap(),
        &["source_config", "selected_access_path"],
    );
    let unresolved_direct = compile_source(&direct, &SourceProfileRegistrySnapshot::default());
    assert_behavior_order(
        &prepare_source_behavior_fingerprints(&direct, None, &unresolved_direct).unwrap(),
        &[
            "direct_source_specialization",
            "source_config",
            "selected_access_path",
        ],
    );
}

#[test]
fn empty_direct_behavior_is_absent_but_equal_value_replacement_is_retained() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed"
    }])));
    let empty = prepare_profile(&source, &profile);
    assert_eq!(empty.len(), 11);
    assert!(!references(&empty).contains(&"direct_source_specialization"));

    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "discovery": { "acceptWhen": { "minResults": 1 } }
    }])));
    let equal_replacement = prepare_profile(&source, &profile);
    assert_eq!(equal_replacement.len(), 12);
    assert!(references(&equal_replacement).contains(&"direct_source_specialization"));
}

#[test]
fn executable_posting_title_provenance_is_retained() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.access_paths = None;
    let baseline = digest_map(&prepare_profile(&source, &profile));
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "discovery": {
            "strategies": [{
                "key": "json_api",
                "extract": {
                    "fields": {
                        "title": {
                            "type": "json_path",
                            "jsonPath": "$.title",
                            "cardinality": "one",
                            "transforms": [{ "type": "trim" }]
                        }
                    }
                }
            }]
        }
    }])));
    let replaced = digest_map(&prepare_profile(&source, &profile));
    assert_eq!(
        baseline[&("source_behavior".into(), "effective_source_profile".into())],
        replaced[&("source_behavior".into(), "effective_source_profile".into())]
    );
    assert_ne!(
        baseline[&("source_behavior".into(), "compiler_provenance".into())],
        replaced[&("source_behavior".into(), "compiler_provenance".into())]
    );
}

#[test]
fn dynamic_object_order_is_canonical_while_array_order_remains_semantic() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let mut first: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let mut second = first.clone();
    first.source_config.insert(
        "nested".into(),
        serde_json::json!({"z": {"b": true, "a": false}, "a": [1, 2]}),
    );
    second.source_config.insert(
        "nested".into(),
        serde_json::json!({"a": [1, 2], "z": {"a": false, "b": true}}),
    );
    let first_digests = digest_map(&prepare_profile(&first, &profile));
    let second_digests = digest_map(&prepare_profile(&second, &profile));
    assert_eq!(first_digests, second_digests);

    second.source_config.insert(
        "nested".into(),
        serde_json::json!({"a": [2, 1], "z": {"a": false, "b": true}}),
    );
    let reordered = digest_map(&prepare_profile(&second, &profile));
    assert_ne!(
        first_digests[&("source_behavior".into(), "source_config".into())],
        reordered[&("source_behavior".into(), "source_config".into())]
    );
    for (identity, digest) in &first_digests {
        if identity.1 != "source_config" {
            assert_eq!(digest, &reordered[identity]);
        }
    }
}

#[test]
fn source_name_is_hashed_only_when_the_compiler_emits_its_binding() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let mut renamed = source.clone();
    renamed.name = "A different name".into();
    assert_eq!(
        digest_map(&prepare_profile(&source, &profile)),
        digest_map(&prepare_profile(&renamed, &profile))
    );

    let mut binding_profile = profile.clone();
    set_selected_fetch_url(&mut binding_profile, "https://example.test/{{source:name}}");
    let original = digest_map(&prepare_profile(&source, &binding_profile));
    let changed = digest_map(&prepare_profile(&renamed, &binding_profile));
    let differing = original
        .iter()
        .filter(|(identity, digest)| changed[*identity] != **digest)
        .map(|(identity, _)| identity.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        differing,
        vec![("source_behavior".into(), "source_runtime_bindings".into())]
    );
}

#[test]
fn preparation_rejects_a_compile_outcome_for_different_source_material() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let outcome = compile_source(&source, &registry_with_profile(profile.clone()));
    let mut different = source.clone();
    different.name = "secret source name".into();
    let error = prepare_source_behavior_fingerprints(&different, Some(&profile), &outcome)
        .expect_err("mixed authoritative inputs must be rejected");
    assert_eq!(error.component_reference, "compiled_source");
    assert!(!error.to_string().contains("secret source name"));
}

#[test]
fn unselected_path_binding_and_excluded_metadata_do_not_affect_digests() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let baseline = digest_map(&prepare_profile(&source, &profile));

    let mut changed_profile = profile.clone();
    changed_profile.name = "Excluded profile name".into();
    let mut unselected = changed_profile.access_paths[0].clone();
    unselected.key = "unselected".into();
    unselected.name = "Unselected".into();
    set_path_fetch_url(&mut unselected, "https://example.test/{{source:name}}");
    changed_profile.access_paths.push(unselected);
    let expanded = prepare_profile(&source, &changed_profile);
    assert!(!references(&expanded).contains(&"source_runtime_bindings"));

    changed_profile.access_paths.pop();
    if let Some(schema) = changed_profile.source_config_schema.as_mut() {
        if let Some(properties) = schema
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
        {
            if let Some(property) = properties
                .values_mut()
                .next()
                .and_then(serde_json::Value::as_object_mut)
            {
                property.insert("title".into(), serde_json::json!("Changed title metadata"));
            }
        }
    }
    let metadata_only = digest_map(&prepare_profile(&source, &changed_profile));
    assert_eq!(baseline, metadata_only);
}

#[test]
fn rejected_source_owned_preparation_contains_no_compiled_only_rows() {
    let mut source: SourceDocument = read_fixture("valid/source-owned-access-path.json");
    let SelectedAccessPath::SourceOwnedAccessPath { discovery, .. } =
        &mut source.selected_access_path
    else {
        panic!("fixture uses Source-owned access")
    };
    discovery.strategies.clear();
    let outcome = compile_source(&source, &SourceProfileRegistrySnapshot::default());
    assert!(matches!(outcome, CompileSourceOutcome::Rejected { .. }));

    let fingerprints = prepare_source_behavior_fingerprints(&source, None, &outcome).unwrap();
    assert_behavior_order(
        &fingerprints,
        &[
            "source_owned_access_path",
            "source_config",
            "selected_access_path",
        ],
    );
    let references = references(&fingerprints);
    assert!(!references.contains(&"compiler_provenance"));
    assert!(!references.contains(&"source_runtime_bindings"));
}

fn prepare_profile(
    source: &SourceDocument,
    profile: &SourceProfileDocument,
) -> Vec<job_radar_lib::CheckFingerprint> {
    let outcome = compile_source(source, &registry_with_profile(profile.clone()));
    prepare_source_behavior_fingerprints(source, Some(profile), &outcome).unwrap()
}

fn prepare_owned(source: &SourceDocument) -> Vec<job_radar_lib::CheckFingerprint> {
    let outcome = compile_source(source, &SourceProfileRegistrySnapshot::default());
    prepare_source_behavior_fingerprints(source, None, &outcome).unwrap()
}

fn assert_behavior_order(
    fingerprints: &[job_radar_lib::CheckFingerprint],
    branch_references: &[&str],
) {
    let mut expected = branch_references.to_vec();
    expected.extend([
        "profile_compiler",
        "profile_runtime",
        "immutable_globals",
        "source_live_check_pagination_smoke_budget",
        "compiler_max_fallback_strategies",
        "security_forbidden_request_key_behavior",
    ]);
    assert_eq!(references(fingerprints), expected);
}

fn references(fingerprints: &[job_radar_lib::CheckFingerprint]) -> Vec<&str> {
    fingerprints
        .iter()
        .map(|fingerprint| fingerprint.reference.as_deref().unwrap())
        .collect()
}

fn digest_map(
    fingerprints: &[job_radar_lib::CheckFingerprint],
) -> BTreeMap<(String, String), String> {
    fingerprints
        .iter()
        .map(|fingerprint| {
            (
                (
                    fingerprint.kind.clone(),
                    fingerprint.reference.clone().unwrap(),
                ),
                fingerprint.sha256.clone().unwrap(),
            )
        })
        .collect()
}

fn fragments(value: serde_json::Value) -> Vec<job_radar_lib::AccessPathFragment> {
    serde_json::from_value(value).unwrap()
}

fn set_selected_fetch_url(profile: &mut SourceProfileDocument, url: &str) {
    set_path_fetch_url(&mut profile.access_paths[0], url);
}

fn set_path_fetch_url(path: &mut ReusableAccessPathDocument, value: &str) {
    let job_radar_lib::Fetch::Http { url, .. } = &mut path.discovery.strategies[0].fetch else {
        panic!("fixture uses HTTP fetch")
    };
    *url = value.into();
}

fn set_owned_fetch_url(source: &mut SourceDocument, value: &str) {
    let SelectedAccessPath::SourceOwnedAccessPath { discovery, .. } =
        &mut source.selected_access_path
    else {
        panic!("fixture uses Source-owned access")
    };
    match &mut discovery.strategies[0].fetch {
        job_radar_lib::Fetch::Http { url, .. } | job_radar_lib::Fetch::Browser { url, .. } => {
            *url = value.into();
        }
    }
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

fn read_fixture<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/source-profile-dsl")
        .join(relative);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
