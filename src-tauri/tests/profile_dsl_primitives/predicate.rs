use std::collections::{BTreeSet, VecDeque};

use job_radar_lib::{
    compile_predicate, evaluate_compiled_predicate, literal_contains, predicate_descriptors,
    validate_predicate_registration_keys, values_equal, CompiledValueResult, ParseType, Predicate,
    PredicateCompileContext, PredicateCompileErrorKind, PredicateKind, PredicatePlacement,
    PredicateRegistryError, ValueCompileContext, ValuePlacement,
};
use serde_json::json;

fn predicate(value: serde_json::Value) -> Predicate {
    serde_json::from_value(value).unwrap()
}

fn context(placement: PredicatePlacement) -> PredicateCompileContext {
    PredicateCompileContext {
        placement,
        value: ValueCompileContext {
            placement: ValuePlacement::DetailMatchFilterOutput,
            document_type: Some(ParseType::Json),
            source_config_keys: BTreeSet::new(),
            posting_meta_keys: BTreeSet::new(),
            capture_keys: BTreeSet::new(),
        },
    }
}

#[test]
fn predicate_family_has_exact_cross_layer_registration_parity() {
    let expected = vec!["non_empty", "regex", "equal"];
    let schema = predicate_schema_keys();
    let serde = PredicateKind::ALL
        .iter()
        .map(|kind| kind.key().to_string())
        .collect::<Vec<_>>();
    let registrations = predicate_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(schema, expected);
    assert_eq!(serde, expected);
    assert_eq!(registrations, expected);
    assert_eq!(
        validate_predicate_registration_keys(&schema, &serde, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_predicate_registration_keys(&schema, &serde[..2], &registrations),
        Err(PredicateRegistryError::Missing {
            layer: "serde",
            keys: vec!["equal".to_string()],
        })
    );
    let mut duplicate = registrations.clone();
    duplicate.push("regex".to_string());
    assert_eq!(
        validate_predicate_registration_keys(&schema, &serde, &duplicate),
        Err(PredicateRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["regex".to_string()],
        })
    );
    let mut extra = serde.clone();
    extra.push("contains".to_string());
    assert_eq!(
        validate_predicate_registration_keys(&schema, &extra, &registrations),
        Err(PredicateRegistryError::Extra {
            layer: "serde",
            keys: vec!["contains".to_string()],
        })
    );
    for key in ["all", "any", "none", "not", "count", "contains"] {
        assert!(serde_json::from_value::<Predicate>(json!({"type":key})).is_err());
    }
    assert!(serde_json::from_value::<Predicate>(json!({
        "left": {"type":"const","value":"x"},
        "right": {"type":"const","value":"x"}
    }))
    .is_err());
}

#[test]
fn predicates_evaluate_each_typed_value_operand_once() {
    let non_empty = compile_predicate(
        &predicate(json!({"type":"non_empty","field":{"type":"const","value":"x"}})),
        &context(PredicatePlacement::Where),
    )
    .unwrap();
    let mut calls = 0;
    assert!(evaluate_compiled_predicate(&non_empty, |_| {
        calls += 1;
        Ok(CompiledValueResult::Sequence(vec![
            "".into(),
            "value".into(),
        ]))
    })
    .unwrap());
    assert_eq!(calls, 1);

    let regex = compile_predicate(
        &predicate(
            json!({"type":"regex","field":{"type":"const","value":"x"},"pattern":"^published$"}),
        ),
        &context(PredicatePlacement::Where),
    )
    .unwrap();
    calls = 0;
    assert!(evaluate_compiled_predicate(&regex, |_| {
        calls += 1;
        Ok(CompiledValueResult::Scalar(Some("published".into())))
    })
    .unwrap());
    assert_eq!(calls, 1);

    let equal = compile_predicate(
        &predicate(json!({"type":"equal","left":{"type":"const","value":"a"},"right":{"type":"const","value":"b"}})),
        &context(PredicatePlacement::DetailMatch),
    )
    .unwrap();
    let mut values = VecDeque::from([
        CompiledValueResult::Scalar(None),
        CompiledValueResult::Scalar(None),
    ]);
    calls = 0;
    assert!(evaluate_compiled_predicate(&equal, |_| {
        calls += 1;
        Ok(values.pop_front().unwrap())
    })
    .unwrap());
    assert_eq!(calls, 2);
}

#[test]
fn regex_is_compiled_statically_and_requires_a_scalar_value() {
    let error = compile_predicate(
        &predicate(json!({"type":"regex","field":{"type":"const","value":"x"},"pattern":"["})),
        &context(PredicatePlacement::Where),
    )
    .unwrap_err();
    assert_eq!(error.kind, PredicateCompileErrorKind::InvalidRegex);
    assert_eq!(error.path, "/pattern");

    let error = compile_predicate(
        &predicate(json!({"type":"regex","field":{"type":"const","value":"x","cardinality":"all"},"pattern":"x"})),
        &context(PredicatePlacement::Where),
    )
    .unwrap_err();
    assert_eq!(error.kind, PredicateCompileErrorKind::OperandShape);
    assert_eq!(error.path, "/field");

    let compiled = compile_predicate(
        &predicate(json!({"type":"regex","field":{"type":"const","value":"x"},"pattern":"^x$"})),
        &context(PredicatePlacement::Where),
    )
    .unwrap();
    let round_trip = serde_json::from_value(serde_json::to_value(&compiled).unwrap()).unwrap();
    assert_eq!(compiled, round_trip);
    assert!(
        !evaluate_compiled_predicate(&compiled, |_| Ok(CompiledValueResult::Scalar(None))).unwrap()
    );
    assert!(
        !evaluate_compiled_predicate(&compiled, |_| Ok(CompiledValueResult::Scalar(Some(
            String::new()
        ))))
        .unwrap()
    );
}

#[test]
fn predicate_placements_are_closed_and_evaluated_values_are_not_retained() {
    let error = compile_predicate(
        &predicate(json!({"type":"equal","left":{"type":"const","value":"a"},"right":{"type":"const","value":"a"}})),
        &context(PredicatePlacement::Where),
    )
    .unwrap_err();
    assert_eq!(error.kind, PredicateCompileErrorKind::Placement);

    let compiled = compile_predicate(
        &predicate(json!({"type":"non_empty","field":{"type":"const","value":"authored"}})),
        &context(PredicatePlacement::Where),
    )
    .unwrap();
    let sentinel = "resolved-provider-value-sentinel";
    assert!(evaluate_compiled_predicate(&compiled, |_| {
        Ok(CompiledValueResult::Scalar(Some(sentinel.to_string())))
    })
    .unwrap());
    assert!(!serde_json::to_string(&compiled).unwrap().contains(sentinel));
}

#[test]
fn detection_strategy_options_can_reuse_unregistered_predicate_behaviors() {
    let response_status = 200_u16;
    let expected_status = 200_u16;
    assert!(values_equal(&response_status, &expected_status));
    assert!(!values_equal(&response_status, &404_u16));

    assert!(literal_contains("rendered literal body", "literal"));
    assert!(!literal_contains("rendered literal body", "Literal"));
    assert!(!PredicateKind::ALL
        .iter()
        .any(|kind| matches!(kind.key(), "contains" | "status_equal")));
}

fn predicate_schema_keys() -> Vec<String> {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../src/schema/profile-dsl/predicate.schema.json"
    ))
    .unwrap();
    schema["$defs"]["predicate"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let definition = entry["$ref"].as_str().unwrap().rsplit('/').next().unwrap();
            schema["$defs"][definition]["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}
