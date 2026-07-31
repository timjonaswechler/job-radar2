use source_profile_dsl::test_support::{
    production_compiled_inventory, production_primitive_inventories,
    validate_primitive_completeness, Family, Owner,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn production_schema_serde_and_compiled_catalogues_are_exactly_equal() {
    let inventories = production_primitive_inventories();
    let normalized = validate_primitive_completeness(&inventories)
        .unwrap_or_else(|errors| panic!("production completeness failed: {errors:#?}"));
    assert_eq!(normalized.len(), inventories.schema.len());
    assert_eq!(normalized.len(), inventories.serde.len());
    assert_eq!(normalized.len(), inventories.compiled.len());
    assert_eq!(normalized.len(), 168);
    for record in &normalized {
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(&record.canonical_file)
                .is_file(),
            "missing {}",
            record.canonical_file
        );
        assert!(!record.schema_pointers.is_empty());
        assert!(!record.schema_evidence.is_empty());
        assert!(!record.serde_file.is_empty());
    }
    let families = normalized.iter().map(|r| r.family).collect::<BTreeSet<_>>();
    assert_eq!(
        families,
        BTreeSet::from([
            Family::Acceptance,
            Family::Browser,
            Family::Capture,
            Family::Cardinality,
            Family::Detection,
            Family::Fetch,
            Family::Pagination,
            Family::PaginationLocation,
            Family::Parse,
            Family::PhaseLimit,
            Family::Predicate,
            Family::Select,
            Family::Template,
            Family::Transform,
            Family::Value,
            Family::ValuePlacement
        ])
    );
}

#[test]
fn each_normalized_identity_has_exactly_one_typed_owner() {
    let mut owners = BTreeMap::new();
    let mut compiled_identities = BTreeMap::new();
    for value in production_compiled_inventory() {
        (value.witness)();
        let id = (value.family, value.key, value.contexts);
        assert!(
            owners.insert(id, value.owner).is_none(),
            "duplicate normalized registration {id:?}"
        );
        assert!(!value.compiled_identity.is_empty());
        assert!(
            compiled_identities
                .insert(value.compiled_identity, (value.family, value.key))
                .is_none(),
            "duplicate compiled plan identity {}",
            value.compiled_identity
        );
    }
    assert!(owners.values().all(|owner| matches!(
        owner,
        Owner::P01
            | Owner::P02
            | Owner::P03
            | Owner::P04
            | Owner::P05
            | Owner::P06a
            | Owner::P06bc
            | Owner::P07
            | Owner::P08
            | Owner::P09
            | Owner::P10
            | Owner::P11
            | Owner::B01
            | Owner::B03a
            | Owner::D02
            | Owner::D03
    )));
}

#[test]
fn compiled_registrations_carry_invocable_owner_local_witnesses() {
    for registration in production_compiled_inventory() {
        (registration.witness)();
        assert!(!registration.compiled_identity.is_empty());
        assert!(!registration
            .compiled_identity
            .contains("CompiledFieldMatch"));
    }
}

#[test]
fn exact_compiled_owner_file_and_plan_identity_catalogue_is_frozen() {
    let actual = production_compiled_inventory()
        .into_iter()
        .map(|registration| {
            format!(
                "{:?}\t{}\t{:?}\t{:?}\t{}\t{:?}\t{}",
                registration.family,
                registration.key,
                registration.contexts,
                registration.owner,
                registration.canonical_file,
                registration.shape,
                registration.compiled_identity
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let expected =
        include_str!("../../fixtures/primitive_completeness/primitive-compiled-catalogue.txt")
            .trim_end()
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
    assert_eq!(actual, expected, "compiled catalogue changed\n{actual}");
}

#[test]
fn every_compiled_identity_has_one_exact_callable_witness() {
    let registrations = production_compiled_inventory();
    let mut witnesses = BTreeMap::<usize, (Family, &'static str)>::new();
    let mut identities = BTreeSet::new();
    for registration in registrations {
        (registration.witness)();
        assert!(!registration.compiled_identity.is_empty());
        assert!(
            identities.insert(registration.compiled_identity),
            "duplicate compiled label {}",
            registration.compiled_identity
        );
        let address = registration.witness as usize;
        assert!(
            witnesses
                .insert(address, (registration.family, registration.key))
                .is_none(),
            "compiled witness reused by {:?} and {:?}",
            witnesses.get(&address),
            (registration.family, registration.key)
        );
    }
}

#[test]
fn frozen_nested_positive_catalogue_is_complete() {
    let keys = production_compiled_inventory()
        .into_iter()
        .map(|r| (r.family, r.key))
        .collect::<BTreeSet<_>>();
    for key in [
        (Family::Parse, "charset"),
        (Family::Select, "json_path.jsonPath"),
        (Family::Transform, "split.separator"),
        (Family::Value, "list.single"),
        (Family::Value, "list.multiple"),
        (Family::Value, "combine.part.value"),
        (Family::Value, "combine.part.optional"),
        (Family::Capture, "entry.from"),
        (Family::Capture, "entry.pattern"),
        (Family::Predicate, "detail.match.left"),
        (Family::Predicate, "detail.match.right"),
        (Family::Fetch, "http.method.GET"),
        (Family::Fetch, "http.body.json.value"),
        (Family::Pagination, "page.parameterLocation"),
        (Family::PaginationLocation, "json_body"),
        (Family::Browser, "browser.url"),
        (Family::Browser, "selector.selector"),
        (Family::Detection, "url.input"),
        (Family::Detection, "pattern_alternatives.alternatives"),
        (Family::Detection, "input_url_pattern.pattern"),
        (Family::Acceptance, "requiredFields"),
        (Family::PhaseLimit, "maxBrowserRenderedBytes"),
    ] {
        assert!(keys.contains(&key), "missing {key:?}");
    }
}
