use job_radar_lib::{
    classify_tagged_variant_keys, escape_json_pointer_token, production_primitive_inventories,
    production_schema_inventory, schema_inventory_from_documents, traverse_schema_root,
    validate_primitive_completeness, Family, PrimitiveContext, SchemaTraversalError,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn schema_inventory_resolves_refs_with_rfc6901_evidence() {
    let docs = BTreeMap::from([
        (
            "root.json".into(),
            json!({"$defs":{"root":{"oneOf":[{"$ref":"#/$defs/local"},{"$ref":"nested.json#/$defs/external"}],"properties":{"a/b~c":{"type":"string"}}},"local":{"properties":{"type":{"const":"local"}}}}}),
        ),
        (
            "nested.json".into(),
            json!({"$defs":{"external":{"properties":{"mode":{"const":"external"}}}}}),
        ),
    ]);
    let evidence =
        traverse_schema_root(&docs, "root.json", "/$defs/root", "discovery_strategy").unwrap();
    assert!(evidence
        .iter()
        .any(|e| e.pointer == "root.json#/$defs/root/properties/a~1b~0c"));
    assert!(evidence
        .iter()
        .any(|e| e.resolved_ref.as_deref() == Some("root.json#/$defs/local")));
    assert!(evidence
        .iter()
        .any(|e| e.resolved_ref.as_deref() == Some("nested.json#/$defs/external")));
    assert_eq!(escape_json_pointer_token("a/b~c"), "a~1b~0c");
}
#[test]
fn missing_refs_and_unguarded_cycles_fail_deterministically() {
    let missing = BTreeMap::from([(
        "root.json".into(),
        json!({"$defs":{"root":{"$ref":"#/$defs/missing"}}}),
    )]);
    assert_eq!(
        traverse_schema_root(&missing, "root.json", "/$defs/root", "detection").unwrap_err(),
        SchemaTraversalError::UnresolvedRef {
            document: "root.json".into(),
            pointer: "/$defs/missing".into()
        }
    );
    let cycle = BTreeMap::from([(
        "root.json".into(),
        json!({"$defs":{"root":{"$ref":"#/$defs/root"}}}),
    )]);
    assert_eq!(
        traverse_schema_root(&cycle, "root.json", "/$defs/root", "detection").unwrap_err(),
        SchemaTraversalError::Cycle {
            document: "root.json".into(),
            pointer: "/$defs/root".into()
        }
    );
}
#[test]
fn unsupported_discriminator_constructions_fail_deterministically() {
    let docs = BTreeMap::from([(
        "root.json".into(),
        json!({"$defs":{"root":{"oneOf":[{"type":"object","properties":{"type":{"const":"x"}}}]}}}),
    )]);
    assert_eq!(
        classify_tagged_variant_keys(&docs, "root.json", "/$defs/root/oneOf", "type").unwrap_err(),
        SchemaTraversalError::UnsupportedDiscriminator {
            document: "root.json".into(),
            pointer: "/$defs/root/oneOf/0".into(),
        }
    );
}

#[test]
fn guarded_recursive_shapes_terminate() {
    let docs = BTreeMap::from([(
        "root.json".into(),
        json!({"$defs":{"value":{"oneOf":[{"type":"string"},{"type":"object","properties":{"child":{"$ref":"#/$defs/value"}}}]}}}),
    )]);
    assert!(
        traverse_schema_root(&docs, "root.json", "/$defs/value", "value")
            .unwrap()
            .len()
            >= 5
    );
}
#[test]
fn checked_in_schema_extraction_discovers_keys_and_options_independently() {
    let schema = production_schema_inventory();
    assert_eq!(schema.len(), 168);
    assert!(schema.iter().all(|shape| !shape.pointers.is_empty()
        && shape
            .pointers
            .iter()
            .all(|pointer| pointer.contains("schema.json#"))));
    let docs = checked_in_schemas();
    for shape in &schema {
        assert_eq!(
            shape.pointers,
            shape
                .evidence
                .iter()
                .map(|entry| entry.pointer.clone())
                .collect()
        );
        assert_eq!(
            shape.contexts,
            shape.evidence.iter().map(|entry| entry.context).collect()
        );
        for context in &shape.contexts {
            assert!(
                shape.evidence.iter().any(|entry| &entry.context == context),
                "{:?}.{} lacks concrete occurrence evidence for {context:?}",
                shape.family,
                shape.key
            );
        }
        for entry in &shape.evidence {
            assert_eq!(entry.chain.first(), Some(&entry.occurrence));
            assert_eq!(entry.chain.last(), Some(&entry.pointer));
            assert!(!entry.chain.is_empty());
            for evidence in &entry.chain {
                let (document, pointer) = evidence.split_once('#').unwrap();
                let schema = docs
                    .get(document)
                    .unwrap_or_else(|| panic!("unknown schema document in {evidence}"));
                assert!(
                    schema.pointer(pointer).is_some(),
                    "unresolved traversal member {evidence} for {:?}.{}",
                    shape.family,
                    shape.key
                );
            }
        }
    }
    let keys = schema
        .iter()
        .map(|shape| (shape.family, shape.key.as_str()))
        .collect::<BTreeSet<_>>();
    for key in [
        (Family::Parse, "json"),
        (Family::Parse, "charset"),
        (Family::Select, "css.selector"),
        (Family::Transform, "regex_replace.replacement"),
        (Family::Value, "combine.part.optional"),
        (Family::Capture, "entry.pattern"),
        (Family::Fetch, "http.body.form.fields"),
        (Family::Browser, "click_until_gone.waitAfterMs"),
        (Family::Pagination, "cursor.nextCursorPath"),
        (Family::Detection, "http.expectStatus"),
        (Family::Acceptance, "minResults"),
        (Family::PhaseLimit, "maxBrowserRenderedBytes"),
    ] {
        assert!(keys.contains(&key), "schema extractor omitted {key:?}");
    }
    let exact = schema
        .iter()
        .map(|shape| {
            format!(
                "{:?}\t{}\t{:?}\t{:?}\t{}",
                shape.family,
                shape.key,
                shape.contexts,
                shape.shape,
                format!("{:?}", shape.evidence)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        exact,
        include_str!("fixtures/primitive_completeness/primitive-positive-catalogue.txt")
            .trim_end()
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n"),
        "exact positive identity catalogue changed\n{exact}"
    );
    let inventories = production_primitive_inventories();
    assert_eq!(inventories.schema, schema);
    validate_primitive_completeness(&inventories).unwrap();
}

#[test]
fn removing_a_reachable_primitive_reference_removes_its_occurrence_context() {
    let mut docs = checked_in_schemas();
    docs.get_mut("profile-dsl/strategy.schema.json")
        .unwrap()
        .pointer_mut("/$defs/discoveryStrategy/properties/fetch/$ref")
        .unwrap()
        .clone_from(&json!("common.schema.json#/$defs/technicalKey"));
    let inventory = schema_inventory_from_documents(docs);
    assert!(inventory
        .iter()
        .filter(|shape| shape.family == Family::Fetch || shape.family == Family::Browser)
        .all(|shape| !shape.contexts.contains(&PrimitiveContext::Discovery)));
}

#[test]
fn removing_one_of_two_same_context_occurrences_is_detected() {
    let mut docs = checked_in_schemas();
    docs.get_mut("profile-dsl/strategy.schema.json")
        .unwrap()
        .pointer_mut("/$defs/discoveryStrategy/properties/where/$ref")
        .unwrap()
        .clone_from(&json!("common.schema.json#/$defs/technicalKey"));
    let inventory = schema_inventory_from_documents(docs);
    let value = inventory
        .iter()
        .find(|shape| shape.family == Family::Value && shape.key == "const")
        .unwrap();
    assert!(
        !value
            .contexts
            .contains(&PrimitiveContext::DiscoveryFilterOutput),
        "the remaining extract occurrence must not conceal the removed where occurrence"
    );
    assert!(value
        .contexts
        .contains(&PrimitiveContext::DetailMatchFilterOutput));
}

#[test]
fn removing_one_value_placement_reference_removes_only_that_typed_context() {
    let mut docs = checked_in_schemas();
    docs.get_mut("profile-dsl/strategy.schema.json")
        .unwrap()
        .pointer_mut("/$defs/discoveryStrategy/properties/captures/$ref")
        .unwrap()
        .clone_from(&json!("common.schema.json#/$defs/technicalKey"));
    let inventory = schema_inventory_from_documents(docs);
    let value = inventory
        .iter()
        .find(|shape| shape.family == Family::Value && shape.key == "const")
        .unwrap();
    assert!(!value
        .contexts
        .contains(&PrimitiveContext::DiscoveryCaptureSource));
    assert!(value
        .contexts
        .contains(&PrimitiveContext::DiscoveryFilterOutput));
    assert!(value
        .contexts
        .contains(&PrimitiveContext::DetailCaptureSource));
    assert!(value
        .contexts
        .contains(&PrimitiveContext::DetailMatchFilterOutput));
}

#[test]
fn removing_browser_url_template_placement_preserves_other_template_contexts() {
    let mut docs = checked_in_schemas();
    docs.get_mut("profile-dsl/fetch.schema.json")
        .unwrap()
        .pointer_mut("/$defs/browserFetch/properties/url/$ref")
        .unwrap()
        .clone_from(&json!("common.schema.json#/$defs/technicalKey"));
    let inventory = schema_inventory_from_documents(docs);
    let template = inventory
        .iter()
        .find(|shape| shape.family == Family::Template && shape.key == "template")
        .unwrap();
    for context in [
        PrimitiveContext::DiscoveryBrowserUrl,
        PrimitiveContext::DetailBrowserUrl,
        PrimitiveContext::DetectionBrowserUrl,
    ] {
        assert!(!template.contexts.contains(&context));
    }
    assert!(template
        .contexts
        .contains(&PrimitiveContext::DiscoveryHttpUrl));
    assert!(template.contexts.contains(&PrimitiveContext::DetailValue));
}

#[test]
fn unclassified_reachable_executable_options_and_discriminators_fail_closed() {
    let mut option_docs = checked_in_schemas();
    option_docs
        .get_mut("profile-dsl/fetch.schema.json")
        .unwrap()
        .pointer_mut("/$defs/httpFetch/properties")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("unclassifiedOption".into(), json!({"type":"string"}));
    assert!(std::panic::catch_unwind(|| schema_inventory_from_documents(option_docs)).is_err());

    let mut discriminator_docs = checked_in_schemas();
    discriminator_docs
        .get_mut("source-profile.schema.json")
        .unwrap()
        .pointer_mut("/$defs/detectionStrategy/oneOf")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"object","properties":{"type":{"const":"unclassified"}}}));
    assert!(
        std::panic::catch_unwind(|| schema_inventory_from_documents(discriminator_docs)).is_err()
    );
}

#[test]
fn source_profile_traversal_reaches_every_strategy_occurrence_and_detection_root() {
    let docs = checked_in_schemas();
    let evidence =
        traverse_schema_root(&docs, "source-profile.schema.json", "", "profile").unwrap();
    for target in [
        "profile-dsl/strategy.schema.json#/$defs/discoveryStrategy",
        "profile-dsl/strategy.schema.json#/$defs/detailStrategy",
        "source-profile.schema.json#/$defs/detectionUrlStrategy",
        "source-profile.schema.json#/$defs/detectionHttpStrategy",
        "source-profile.schema.json#/$defs/detectionBrowserStrategy",
    ] {
        assert!(
            evidence
                .iter()
                .any(|e| e.resolved_ref.as_deref() == Some(target)),
            "missing traversal {target}"
        );
    }
}
fn checked_in_schemas() -> BTreeMap<String, serde_json::Value> {
    [
        (
            "source-profile.schema.json",
            include_str!("../src/schema/source-profile.schema.json"),
        ),
        (
            "profile-dsl/common.schema.json",
            include_str!("../src/schema/profile-dsl/common.schema.json"),
        ),
        (
            "profile-dsl/diagnostics.schema.json",
            include_str!("../src/schema/profile-dsl/diagnostics.schema.json"),
        ),
        (
            "profile-dsl/extract.schema.json",
            include_str!("../src/schema/profile-dsl/extract.schema.json"),
        ),
        (
            "profile-dsl/fetch.schema.json",
            include_str!("../src/schema/profile-dsl/fetch.schema.json"),
        ),
        (
            "profile-dsl/pagination.schema.json",
            include_str!("../src/schema/profile-dsl/pagination.schema.json"),
        ),
        (
            "profile-dsl/parse.schema.json",
            include_str!("../src/schema/profile-dsl/parse.schema.json"),
        ),
        (
            "profile-dsl/policy.schema.json",
            include_str!("../src/schema/profile-dsl/policy.schema.json"),
        ),
        (
            "profile-dsl/predicate.schema.json",
            include_str!("../src/schema/profile-dsl/predicate.schema.json"),
        ),
        (
            "profile-dsl/select.schema.json",
            include_str!("../src/schema/profile-dsl/select.schema.json"),
        ),
        (
            "profile-dsl/strategy.schema.json",
            include_str!("../src/schema/profile-dsl/strategy.schema.json"),
        ),
        (
            "profile-dsl/transform.schema.json",
            include_str!("../src/schema/profile-dsl/transform.schema.json"),
        ),
    ]
    .into_iter()
    .map(|(p, s)| (p.into(), serde_json::from_str(s).unwrap()))
    .collect()
}
