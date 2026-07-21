use job_radar_lib::{
    cardinality_descriptors, compile_cardinality, validate_cardinality_registration_keys,
    Cardinality, CardinalityDiagnosticContext, CardinalityError, CardinalityOutcome,
    CardinalityRegistryError, DiagnosticCategory, DiagnosticSeverity, FieldExpression,
    ScriptedProfileHttpClient, SelectedItem, SelectedSequence,
};

#[derive(Clone, Copy)]
enum Expected {
    Scalar(Option<&'static str>),
    Sequence(&'static [&'static str]),
    Error {
        expected: &'static str,
        actual: usize,
    },
}

#[test]
fn cardinality_empty_one_many_matrix_is_frozen_at_the_public_sequence_seam() {
    let cases = [
        (cardinality("one"), &[][..], Expected::Scalar(None)),
        (
            cardinality("one"),
            &["alpha"][..],
            Expected::Scalar(Some("alpha")),
        ),
        (
            cardinality("one"),
            &["alpha", "beta"][..],
            Expected::Error {
                expected: "one",
                actual: 2,
            },
        ),
        (cardinality("first"), &[][..], Expected::Scalar(None)),
        (
            cardinality("first"),
            &["alpha"][..],
            Expected::Scalar(Some("alpha")),
        ),
        (
            cardinality("first"),
            &["alpha", "beta"][..],
            Expected::Scalar(Some("alpha")),
        ),
        (cardinality("optional"), &[][..], Expected::Scalar(None)),
        (
            cardinality("optional"),
            &["alpha"][..],
            Expected::Scalar(Some("alpha")),
        ),
        (
            cardinality("optional"),
            &["alpha", "beta"][..],
            Expected::Error {
                expected: "optional",
                actual: 2,
            },
        ),
        (cardinality("all"), &[][..], Expected::Sequence(&[])),
        (
            cardinality("all"),
            &["alpha"][..],
            Expected::Sequence(&["alpha"]),
        ),
        (
            cardinality("all"),
            &["alpha", "beta"][..],
            Expected::Sequence(&["alpha", "beta"]),
        ),
    ];

    for (authored, values, expected) in cases {
        let plan = compile_cardinality(authored);
        let actual = plan.execute(
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
        );
        match expected {
            Expected::Scalar(expected) => assert_eq!(
                actual,
                Ok(CardinalityOutcome::Scalar(expected.map(str::to_string))),
                "{authored:?} over {values:?}"
            ),
            Expected::Sequence(expected) => assert_eq!(
                actual,
                Ok(CardinalityOutcome::Sequence(
                    expected.iter().map(|value| value.to_string()).collect()
                )),
                "{authored:?} over {values:?}"
            ),
            Expected::Error {
                expected,
                actual: count,
            } => assert_eq!(
                actual,
                Err(CardinalityError {
                    expected: expected.to_string(),
                    actual_count: count,
                }),
                "{authored:?} over {values:?}"
            ),
        }
    }
}

#[test]
fn cardinality_consumes_p03_selected_sequence_without_reordering_or_conversion() {
    let selected = SelectedSequence::new(vec![
        SelectedItem::Text("second".to_string()),
        SelectedItem::Text("first".to_string()),
    ]);
    let outcome = compile_cardinality(cardinality("all"))
        .execute(selected)
        .unwrap();
    let CardinalityOutcome::Sequence(values) = outcome else {
        panic!("all must return a sequence")
    };
    let text = values
        .into_iter()
        .map(|value| match value {
            SelectedItem::Text(value) => value,
            _ => panic!("cardinality must not convert selected items"),
        })
        .collect::<Vec<_>>();
    assert_eq!(text, vec!["second", "first"]);
}

#[test]
fn cardinality_failure_has_bounded_phase_context() {
    let diagnostic = compile_cardinality(cardinality("one"))
        .execute(vec!["alpha", "beta"])
        .unwrap_err()
        .into_diagnostic(CardinalityDiagnosticContext {
            path: "/discovery/strategies/0/extract/fields/title",
            strategy_key: Some("api"),
            item_index: Some(7),
        });

    assert_eq!(diagnostic.category, DiagnosticCategory::Runtime);
    assert_eq!(diagnostic.code, "field_cardinality_mismatch");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.path,
        "/discovery/strategies/0/extract/fields/title"
    );
    assert_eq!(diagnostic.strategy_key.as_deref(), Some("api"));
    assert_eq!(
        diagnostic.details,
        Some(serde_json::json!({
            "expectedCardinality": "one",
            "actualCount": 2,
            "itemIndex": 7,
        }))
    );
}

#[test]
fn cardinality_family_has_exact_cross_layer_registration_parity() {
    let expected = vec!["one", "first", "optional", "all"];
    let schema = cardinality_schema_keys();
    let serde = Cardinality::ALL
        .iter()
        .map(|cardinality| {
            serde_json::to_value(cardinality)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    let registrations = cardinality_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();

    assert_eq!(schema, expected);
    assert_eq!(serde, expected);
    assert_eq!(registrations, expected);
    assert_eq!(
        validate_cardinality_registration_keys(&schema, &serde, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_cardinality_registration_keys(&schema, &serde[..3], &registrations),
        Err(CardinalityRegistryError::Missing {
            layer: "serde",
            keys: vec!["all".to_string()],
        })
    );
    let mut duplicate = registrations.clone();
    duplicate.push("first".to_string());
    assert_eq!(
        validate_cardinality_registration_keys(&schema, &serde, &duplicate),
        Err(CardinalityRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["first".to_string()],
        })
    );
}

#[test]
fn invalid_cardinality_is_rejected_before_a_plan_or_external_effect() {
    let client = ScriptedProfileHttpClient::new([]);

    serde_json::from_value::<FieldExpression>(serde_json::json!({
        "type": "json_path",
        "jsonPath": "$.title",
        "cardinality": "many"
    }))
    .expect_err("an invalid authored variant must not reach the typed compiler");
    serde_json::from_value::<job_radar_lib::CompiledCardinality>(serde_json::json!("many"))
        .expect_err("an invalid compiled variant must not reach runtime");

    assert_eq!(client.request_count(), 0);
}

fn cardinality(value: &str) -> Cardinality {
    serde_json::from_value(serde_json::json!(value)).unwrap()
}

fn cardinality_schema_keys() -> Vec<String> {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../src/schema/profile-dsl/extract.schema.json"
    ))
    .unwrap();
    schema["$defs"]["cardinality"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_string())
        .collect()
}
