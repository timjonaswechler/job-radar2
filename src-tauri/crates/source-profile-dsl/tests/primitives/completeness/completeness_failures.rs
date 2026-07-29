use source_profile_dsl::profile_dsl::primitives::completeness::{
    validate_primitive_completeness, AuthoredShapeKind::*, CompiledRegistration,
    CompletenessPolicy, Family, InventoryLayer, Owner, PrimitiveCompletenessError,
    PrimitiveCompletenessInventories, PrimitiveContext, SchemaPointerContext, SchemaShape,
    SerdeShape,
};
use std::collections::{BTreeMap, BTreeSet};

fn synthetic_compiled_witness() {}
const CONTEXTS: &[PrimitiveContext] = &[PrimitiveContext::DiscoveryFilterOutput];
fn identity(key: &str) -> String {
    format!("CompiledValue::Synthetic::{key}")
}
fn schema(key: &str) -> SchemaShape {
    SchemaShape {
        family: Family::Value,
        key: key.into(),
        contexts: CONTEXTS.iter().copied().collect(),
        owner: Owner::P06bc,
        canonical_file:
            "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/value/mod.rs".into(),
        compiled_identity: identity(key),
        shape: Tagged,
        pointers: BTreeSet::from([format!("schema#/{key}")]),
        evidence: BTreeSet::from([SchemaPointerContext {
            occurrence: format!("schema#/{key}"),
            pointer: format!("schema#/{key}"),
            chain: vec![format!("schema#/{key}")],
            context: PrimitiveContext::DiscoveryFilterOutput,
        }]),
    }
}
fn serde(key: &str) -> SerdeShape {
    SerdeShape {
        family: Family::Value,
        key: key.into(),
        contexts: CONTEXTS.iter().copied().collect(),
        owner: Owner::P06bc,
        canonical_file:
            "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/value/mod.rs".into(),
        compiled_identity: identity(key),
        shape: Tagged,
        authored_file: "src-tauri/crates/source-profile-dsl/src/profile_dsl/documents/extract.rs",
    }
}
fn compiled(key: &'static str) -> CompiledRegistration {
    let identity = Box::leak(identity(key).into_boxed_str());
    CompiledRegistration {
        family: Family::Value,
        key,
        contexts: CONTEXTS,
        owner: Owner::P06bc,
        canonical_file:
            "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/value/mod.rs",
        shape: Tagged,
        compiled_identity: identity,
        witness: synthetic_compiled_witness,
        behavior_bearing: false,
    }
}
fn complete(key: &'static str) -> PrimitiveCompletenessInventories {
    PrimitiveCompletenessInventories {
        schema: vec![schema(key)],
        serde: vec![serde(key)],
        compiled: vec![compiled(key)],
        policy: policy(&[]),
    }
}

fn policy(forbidden: &[(Family, &str)]) -> CompletenessPolicy {
    CompletenessPolicy {
        forbidden: forbidden
            .iter()
            .map(|(family, key)| (*family, (*key).to_owned()))
            .collect(),
        canonical_files: BTreeMap::from([
            (
                Owner::P06bc,
                BTreeSet::from([
                    "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/value/mod.rs"
                        .to_owned(),
                ]),
            ),
            (
                Owner::P02,
                BTreeSet::from([
                    "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/parse/mod.rs"
                        .to_owned(),
                ]),
            ),
        ]),
    }
}
fn errors(v: &PrimitiveCompletenessInventories) -> Vec<PrimitiveCompletenessError> {
    validate_primitive_completeness(v).unwrap_err()
}

#[test]
fn each_missing_side_fails_independently() {
    for layer in [
        InventoryLayer::Schema,
        InventoryLayer::Serde,
        InventoryLayer::Compiled,
    ] {
        let mut v = complete("template");
        match layer {
            InventoryLayer::Schema => v.schema.clear(),
            InventoryLayer::Serde => v.serde.clear(),
            InventoryLayer::Compiled => v.compiled.clear(),
        };
        assert!(errors(&v).iter().any(
            |e| matches!(e,PrimitiveCompletenessError::Missing{layer:actual,..} if *actual==layer)
        ));
    }
}
#[test]
fn duplicate_conflicting_owner_context_shape_file_and_behavior_fail() {
    let mut v = complete("template");
    v.compiled.push(CompiledRegistration {
        owner: Owner::P05,
        ..compiled("template")
    });
    let e = errors(&v);
    assert!(e.iter().any(|e| matches!(
        e,
        PrimitiveCompletenessError::Duplicate {
            layer: InventoryLayer::Compiled,
            ..
        }
    )));
    assert!(e.iter().any(|e|matches!(e,PrimitiveCompletenessError::DuplicateOwner{owners,..} if owners==&BTreeSet::from([Owner::P05,Owner::P06bc]))));
    let mut v = complete("template");
    v.schema[0].evidence.clear();
    assert!(errors(&v).iter().any(|error| matches!(
        error,
        PrimitiveCompletenessError::SchemaContextEvidenceMismatch { .. }
    )));
    let mut v = complete("template");
    v.serde[0].contexts = BTreeSet::from([PrimitiveContext::DetailMatch]);
    assert!(
        errors(&v)
            .iter()
            .filter(|e| matches!(e, PrimitiveCompletenessError::Missing { .. }))
            .count()
            >= 2
    );
    let mut v = complete("template");
    v.serde[0].shape = Untagged;
    assert!(errors(&v).iter().any(|e| matches!(
        e,
        PrimitiveCompletenessError::ShapeMismatch {
            layer: InventoryLayer::Serde,
            ..
        }
    )));
    let mut v = complete("template");
    v.compiled[0].canonical_file =
        "src-tauri/crates/source-profile-dsl/src/profile_dsl/compiler/mod.rs";
    assert!(errors(&v)
        .iter()
        .any(|e| matches!(e, PrimitiveCompletenessError::NoncanonicalFile { .. })));
    let mut v = complete("template");
    v.compiled[0].behavior_bearing = true;
    assert!(errors(&v).iter().any(|e| matches!(
        e,
        PrimitiveCompletenessError::BehaviorBearingRegistration { .. }
    )));
}
#[test]
fn schema_and_serde_owner_file_and_compiled_identity_must_match_independently() {
    for (layer, field) in [
        (InventoryLayer::Schema, "owner"),
        (InventoryLayer::Schema, "canonical_file"),
        (InventoryLayer::Schema, "compiled_identity"),
        (InventoryLayer::Serde, "owner"),
        (InventoryLayer::Serde, "canonical_file"),
        (InventoryLayer::Serde, "compiled_identity"),
    ] {
        let mut value = complete("template");
        match (layer, field) {
            (InventoryLayer::Schema, "owner") => value.schema[0].owner = Owner::P02,
            (InventoryLayer::Schema, "canonical_file") => {
                value.schema[0].canonical_file = "wrong".into()
            }
            (InventoryLayer::Schema, _) => value.schema[0].compiled_identity = "wrong".into(),
            (InventoryLayer::Serde, "owner") => value.serde[0].owner = Owner::P02,
            (InventoryLayer::Serde, "canonical_file") => {
                value.serde[0].canonical_file = "wrong".into()
            }
            (InventoryLayer::Serde, _) => value.serde[0].compiled_identity = "wrong".into(),
            _ => unreachable!(),
        }
        assert!(errors(&value).iter().any(|error| matches!(
            error,
            PrimitiveCompletenessError::MetadataMismatch { layer: actual_layer, field: actual_field, .. }
                if *actual_layer == layer && actual_field == field
        )));
    }
}

#[test]
fn compiled_identities_are_non_empty_and_globally_unique() {
    let mut empty = complete("template");
    empty.compiled[0].compiled_identity = "";
    assert!(errors(&empty).iter().any(|error| matches!(
        error,
        PrimitiveCompletenessError::EmptyCompiledIdentity { .. }
    )));

    let mut missing_witness = complete("template");
    missing_witness.compiled[0].witness =
        source_profile_dsl::profile_dsl::primitives::completeness::missing_witness;
    assert!(errors(&missing_witness).iter().any(|error| matches!(
        error,
        PrimitiveCompletenessError::MissingCompiledWitness { .. }
    )));

    let mut duplicate = PrimitiveCompletenessInventories {
        policy: policy(&[]),
        ..PrimitiveCompletenessInventories::default()
    };
    for key in ["template", "const"] {
        let leaked = Box::leak(key.to_owned().into_boxed_str());
        duplicate.schema.push(schema(key));
        duplicate.serde.push(serde(key));
        let mut registration = compiled(leaked);
        registration.compiled_identity = "CompiledValue::Conflict";
        duplicate.compiled.push(registration);
    }
    assert!(errors(&duplicate).iter().any(|error| matches!(
        error,
        PrimitiveCompletenessError::DuplicateCompiledIdentity { compiled_identity, registrations }
            if compiled_identity == "CompiledValue::Conflict" && registrations.len() == 2
    )));
}

#[test]
fn every_nested_shape_class_and_removed_key_is_visible() {
    let nested = [
        ("list.single", Untagged),
        ("list.multiple", Untagged),
        ("capture.entry.from", ParentOption),
        ("detail.match.left", ParentOption),
        ("acceptance.requiredFields", ParentOption),
        ("combine.part.optional", ParentOption),
    ];
    let mut v = PrimitiveCompletenessInventories {
        policy: policy(&[]),
        ..PrimitiveCompletenessInventories::default()
    };
    for (key, shape) in nested {
        let leaked: &'static str = Box::leak(key.to_owned().into_boxed_str());
        let mut s = schema(key);
        s.shape = shape;
        let mut d = serde(key);
        d.shape = shape;
        let mut c = compiled(leaked);
        c.shape = shape;
        v.schema.push(s);
        v.serde.push(d);
        v.compiled.push(c);
    }
    v.serde.retain(|s| s.key != "combine.part.optional");
    assert!(errors(&v).iter().any(|e|matches!(e,PrimitiveCompletenessError::Missing{layer:InventoryLayer::Serde,key,..} if key=="combine.part.optional")));
    let ctx = BTreeSet::from([PrimitiveContext::Discovery]);
    let removed = PrimitiveCompletenessInventories {
        schema: vec![SchemaShape {
            family: Family::Parse,
            key: "text".into(),
            contexts: ctx.clone(),
            owner: Owner::P02,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/parse/mod.rs".into(),
            compiled_identity: "removed".into(),
            shape: Tagged,
            pointers: BTreeSet::from(["schema".into()]),
            evidence: BTreeSet::from([SchemaPointerContext {
                occurrence: "schema".into(),
                pointer: "schema".into(),
                chain: vec!["schema".into()],
                context: PrimitiveContext::Discovery,
            }]),
        }],
        serde: vec![SerdeShape {
            family: Family::Parse,
            key: "text".into(),
            contexts: ctx,
            owner: Owner::P02,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/parse/mod.rs".into(),
            compiled_identity: "removed".into(),
            shape: Tagged,
            authored_file: "serde",
        }],
        compiled: vec![CompiledRegistration {
            family: Family::Parse,
            key: "text",
            contexts: &[PrimitiveContext::Discovery],
            owner: Owner::P02,
            canonical_file:
                "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/parse/mod.rs",
            shape: Tagged,
            compiled_identity: "removed",
            witness: synthetic_compiled_witness,
            behavior_bearing: false,
        }],
        policy: policy(&[(Family::Parse, "text")]),
    };
    assert_eq!(
        errors(&removed)
            .iter()
            .filter(|e| matches!(e, PrimitiveCompletenessError::RemovedKey { .. }))
            .count(),
        3
    );
}
#[test]
fn serde_omission_fails_while_schema_and_compiled_agree() {
    let mut v = complete("combine.part.optional");
    v.serde.clear();
    assert!(errors(&v).iter().any(|e| matches!(
        e,
        PrimitiveCompletenessError::Missing {
            layer: InventoryLayer::Serde,
            ..
        }
    )));
}
#[test]
fn normalization_and_errors_are_permutation_stable() {
    let mut a = PrimitiveCompletenessInventories {
        policy: policy(&[]),
        ..PrimitiveCompletenessInventories::default()
    };
    for key in ["template", "const", "combine"] {
        let leaked = Box::leak(key.to_owned().into_boxed_str());
        a.schema.push(schema(key));
        a.serde.push(serde(key));
        a.compiled.push(compiled(leaked));
    }
    let expected = validate_primitive_completeness(&a).unwrap();
    a.schema.reverse();
    a.serde.reverse();
    a.compiled.reverse();
    assert_eq!(validate_primitive_completeness(&a).unwrap(), expected);
    a.compiled.clear();
    let first = errors(&a);
    a.schema.reverse();
    a.serde.reverse();
    assert_eq!(errors(&a), first);
}
