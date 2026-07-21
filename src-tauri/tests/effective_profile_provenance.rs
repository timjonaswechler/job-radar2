use std::{fs, path::Path};

use job_radar_lib::{
    compile_source, AccessPathFragment, CompileSourceOutcome, CompiledSourceProvenance,
    ProvenanceEntry, ProvenanceOrigin, ProvenancePathSegment, RegistrySourceProfile,
    SourceDocument, SourceProfileDocument, SourceProfileRegistrySnapshot,
};

#[test]
fn base_and_direct_terminals_have_exact_origins_without_metadata() {
    let mut profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let job_radar_lib::Fetch::Http { method, body, .. } =
        &mut profile.access_paths[0].discovery.strategies[0].fetch
    else {
        panic!("fixture uses HTTP fetch")
    };
    *method = Some(job_radar_lib::HttpMethod::Post);
    *body = Some(
        serde_json::from_value(serde_json::json!({
            "type": "json", "value": { "outer": { "inner": "base" } }
        }))
        .unwrap(),
    );
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([{
        "key": "json_feed",
        "sourceConfigSchema": {
            "properties": {
                "language": { "pattern": "^[a-z]{2}$" },
                "region": { "type": "string" }
            },
            "required": ["language"]
        },
        "discovery": {
            "policy": { "type": "first_accepted" },
            "strategies": [{
                "key": "json_api",
                "fetch": { "headers": { "x-requested-with": "direct" } },
                "acceptWhen": { "minResults": 1 }
            }]
        }
    }])));

    let provenance = compiled_provenance(&source, profile);
    let entries = profile_entries(&provenance);

    assert_origin(
        entries,
        &[access("json_feed")],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[access("json_feed"), field("name")],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("fetch"),
            field("headers"),
            map_key("accept"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("fetch"),
            field("headers"),
            map_key("x-requested-with"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("fetch"),
            field("body"),
            field("value"),
            map_key("outer"),
            map_key("inner"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("acceptWhen"),
            field("minResults"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("sourceConfigSchema"),
            field("properties"),
            map_key("language"),
            field("type"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("sourceConfigSchema"),
            field("properties"),
            map_key("language"),
            field("pattern"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("sourceConfigSchema"),
            field("properties"),
            map_key("region"),
            field("type"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("sourceConfigSchema"),
            field("required"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );

    assert!(entries
        .iter()
        .all(|entry| !entry.path.segments.iter().any(|segment| {
            matches!(segment, ProvenancePathSegment::Field { name }
            if matches!(name.as_str(), "description" | "diagnostics" | "support" | "detect"))
        })));
    assert_eq!(
        entries
            .iter()
            .map(|entry| &entry.path)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        entries.len(),
        "every included terminal is represented exactly once",
    );
}

#[test]
fn locator_only_fragment_is_a_noop_and_scalar_and_empty_object_are_terminals() {
    let mut profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let job_radar_lib::Fetch::Http { headers, .. } =
        &mut profile.access_paths[0].discovery.strategies[0].fetch
    else {
        panic!("fixture uses HTTP fetch")
    };
    *headers = Some(Default::default());
    profile.access_paths[0].discovery.strategies[0]
        .extract
        .fields
        .company =
        serde_json::from_value(serde_json::json!({ "type": "const", "value": false })).unwrap();
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.access_paths = Some(fragments(serde_json::json!([{ "key": "json_feed" }])));

    let provenance = compiled_provenance(&source, profile);
    let entries = profile_entries(&provenance);
    assert!(entries
        .iter()
        .all(|entry| entry.origin == ProvenanceOrigin::BaseSourceProfile));
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("fetch"),
            field("headers"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("extract"),
            field("fields"),
            field("company"),
            field("value"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
}

#[test]
fn equivalent_dynamic_map_insertion_orders_serialize_identically() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let base_source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let provenances = [
        r#"[{"key":"json_feed","discovery":{"strategies":[{"key":"json_api","fetch":{"headers":{"user-agent":"z","accept-language":"a"}}}]}}]"#,
        r#"[{"discovery":{"strategies":[{"fetch":{"headers":{"accept-language":"a","user-agent":"z"}},"key":"json_api"}]},"key":"json_feed"}]"#,
    ]
    .map(|json| {
        let mut source = base_source.clone();
        source.access_paths = Some(serde_json::from_str(json).unwrap());
        compiled_provenance(&source, profile.clone())
    });
    assert_eq!(
        serde_json::to_vec(&provenances[0]).unwrap(),
        serde_json::to_vec(&provenances[1]).unwrap()
    );
}

#[test]
fn arrays_are_atomic_and_policy_and_dynamic_maps_are_complete() {
    let mut profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    profile.access_paths[0]
        .source_config_schema
        .as_mut()
        .unwrap()["properties"]
        .as_object_mut()
        .unwrap()["language"]
        .as_object_mut()
        .unwrap()
        .insert("enum".into(), serde_json::json!(["en", "de"]));
    let authored_strategy = &mut profile.access_paths[0].discovery.strategies[0];
    authored_strategy.conditions = Some(
        serde_json::from_value(serde_json::json!([{
            "type": "non_empty",
            "field": { "type": "item_field", "key": "title" }
        }]))
        .unwrap(),
    );
    authored_strategy.fetch = serde_json::from_value(serde_json::json!({
        "mode": "browser",
        "url": "{{sourceConfig:feedUrl}}",
        "timeoutMs": 10000,
        "waits": [{ "type": "selector", "selector": ".job", "timeoutMs": 1000 }],
        "interactions": [{
            "type": "click_if_visible", "selector": ".more", "maxCount": 1, "waitAfterMs": 10
        }]
    }))
    .unwrap();
    let source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let provenance = compiled_provenance(&source, profile);
    let entries = profile_entries(&provenance);

    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            field("policy"),
            field("type"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("extract"),
            field("fields"),
            field("title"),
            field("transforms"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("extract"),
            field("fields"),
            field("postingMeta"),
            map_key("jobId"),
            field("jsonPath"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    for terminal in ["waits", "interactions"] {
        assert_origin(
            entries,
            &[
                access("json_feed"),
                field("discovery"),
                strategy("json_api"),
                field("fetch"),
                field(terminal),
            ],
            ProvenanceOrigin::BaseSourceProfile,
        );
    }
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("json_api"),
            field("where"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("sourceConfigSchema"),
            field("properties"),
            map_key("language"),
            field("enum"),
        ],
        ProvenanceOrigin::BaseSourceProfile,
    );
    assert!(!entries
        .iter()
        .any(|entry| entry.path.segments.iter().any(|segment| {
            matches!(segment, ProvenancePathSegment::MapKey { key } if key.parse::<usize>().is_ok())
        })));
}

#[test]
fn complete_added_paths_and_strategies_are_direct_in_semantic_order() {
    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let mut source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    source.source_config.remove("language");
    let base_strategy =
        serde_json::to_value(&profile.access_paths[0].discovery.strategies[0]).unwrap();
    let mut added_strategy = base_strategy.clone();
    added_strategy["key"] = serde_json::json!("added_strategy");
    let mut path_strategy = base_strategy;
    path_strategy["key"] = serde_json::json!("path_strategy");
    source.access_paths = Some(fragments(serde_json::json!([
        {
            "key": "json_feed",
            "discovery": { "strategies": [added_strategy] }
        },
        {
            "key": "unselected_added",
            "name": "Unselected added",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [path_strategy]
            }
        }
    ])));

    let provenance = compiled_provenance(&source, profile);
    let entries = profile_entries(&provenance);
    let keys = entries
        .iter()
        .filter_map(|entry| match entry.path.segments.as_slice() {
            [ProvenancePathSegment::AccessPath { key }] => Some(key.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["json_feed", "unselected_added"]);
    assert_origin(
        entries,
        &[
            access("json_feed"),
            field("discovery"),
            strategy("added_strategy"),
        ],
        ProvenanceOrigin::DirectSourceFragment,
    );
    assert!(entries.iter().filter(|entry| matches!(entry.path.segments.first(), Some(ProvenancePathSegment::AccessPath { key }) if key == "unselected_added"))
        .all(|entry| entry.origin == ProvenanceOrigin::DirectSourceFragment));
}

#[test]
fn source_owned_provenance_is_distinct_all_owned_and_minimized() {
    let source: SourceDocument = read_fixture("valid/source-owned-access-path.json");
    let CompileSourceOutcome::Compiled {
        source: compiled, ..
    } = compile_source(&source, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("Source-owned fixture must compile")
    };
    let CompiledSourceProvenance::SourceOwned { entries } = compiled.provenance else {
        panic!("Source-owned compilation must carry SourceOwned provenance")
    };
    assert!(!entries.is_empty());
    assert!(entries
        .iter()
        .all(|entry| entry.origin == ProvenanceOrigin::SourceOwnedAccessPath));
    assert!(entries.iter().all(|entry| !entry.path.segments.iter().any(|segment| {
        matches!(segment, ProvenancePathSegment::Field { name }
            if matches!(name.as_str(), "sourceConfig" | "sourceSupport" | "diagnostics" | "description"))
    })));
}

#[test]
fn provenance_serialization_is_typed_stable_and_rejects_unknown_shape() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/source-profile-dsl/valid/compiled-source-provenance.json");
    let fixture_value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path).unwrap()).unwrap();
    let fixture: CompiledSourceProvenance = serde_json::from_value(fixture_value.clone()).unwrap();
    assert_eq!(serde_json::to_value(fixture).unwrap(), fixture_value);

    let profile: SourceProfileDocument = read_fixture("valid/simple-source-profile.json");
    let source: SourceDocument = read_fixture("valid/source-selecting-access-path.json");
    let first = compiled_provenance(&source, profile.clone());
    let second = compiled_provenance(&source, profile);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );

    let value = serde_json::to_value(&first).unwrap();
    assert_eq!(value["kind"], "profile");
    assert!(value["entries"][0].get("origin").is_some());
    assert!(value["entries"][0]["path"]["segments"][0]
        .get("kind")
        .is_some());
    assert!(
        serde_json::from_value::<CompiledSourceProvenance>(serde_json::json!({
            "kind": "profile", "entries": [], "version": 1
        }))
        .is_err()
    );
}

fn compiled_provenance(
    source: &SourceDocument,
    profile: SourceProfileDocument,
) -> CompiledSourceProvenance {
    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile,
        }],
        sources: vec![],
        diagnostics: Vec::new(),
    };
    let outcome = compile_source(source, &registry);
    let CompileSourceOutcome::Compiled { source, .. } = outcome else {
        panic!("fixture must compile: {outcome:?}")
    };
    source.provenance
}

fn profile_entries(provenance: &CompiledSourceProvenance) -> &[ProvenanceEntry] {
    let CompiledSourceProvenance::Profile { entries } = provenance else {
        panic!("expected Profile provenance")
    };
    entries
}

fn assert_origin(
    entries: &[ProvenanceEntry],
    segments: &[ProvenancePathSegment],
    expected: ProvenanceOrigin,
) {
    let entry = entries
        .iter()
        .find(|entry| entry.path.segments == segments)
        .unwrap_or_else(|| panic!("missing provenance path: {segments:?}"));
    assert_eq!(entry.origin, expected, "wrong origin for {segments:?}");
}

fn field(name: &str) -> ProvenancePathSegment {
    ProvenancePathSegment::Field { name: name.into() }
}
fn access(key: &str) -> ProvenancePathSegment {
    ProvenancePathSegment::AccessPath { key: key.into() }
}
fn strategy(key: &str) -> ProvenancePathSegment {
    ProvenancePathSegment::Strategy { key: key.into() }
}
fn map_key(key: &str) -> ProvenancePathSegment {
    ProvenancePathSegment::MapKey { key: key.into() }
}

fn fragments(value: serde_json::Value) -> Vec<AccessPathFragment> {
    serde_json::from_value(value).expect("valid direct fragments")
}

fn read_fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/source-profile-dsl")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}
