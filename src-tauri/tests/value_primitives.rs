use std::collections::BTreeSet;

use job_radar_lib::{
    compile_list_value, compile_value, evaluate_discovery_output_value,
    validate_value_placement_registration_keys, validate_value_registration_keys,
    value_descriptors, value_placement_descriptors, CompiledListValue, CompiledValueResult,
    DiscoveryFilterOutputValueContext, FieldExpression, ListFieldExpression, ParseType,
    SelectedItem, SelectedSequence, SelectedValueCarrier, SourceValueView, ValueCompileContext,
    ValueCompileErrorKind, ValueEvaluationErrorKind, ValueKind, ValuePlacement,
    ValuePlacementRegistryError, ValueRegistryError, ValueShape, VALUE_MAX_DEPTH,
    VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES, VALUE_MAX_NODES,
};
use serde_json::json;

fn expression(value: serde_json::Value) -> FieldExpression {
    serde_json::from_value(value).unwrap()
}

fn context(placement: ValuePlacement, document_type: Option<ParseType>) -> ValueCompileContext {
    ValueCompileContext {
        placement,
        document_type,
        source_config_keys: BTreeSet::from(["tenant".to_string()]),
        posting_meta_keys: BTreeSet::from(["jobId".to_string()]),
        capture_keys: BTreeSet::from(["slug".to_string()]),
    }
}

#[test]
fn four_value_placements_have_exhaustive_registration_parity() {
    let expected = vec![
        "discovery_capture_source",
        "discovery_filter_output",
        "detail_capture_source",
        "detail_match_filter_output",
    ];
    let keys = ValuePlacement::ALL
        .iter()
        .map(|placement| placement.key().to_string())
        .collect::<Vec<_>>();
    let descriptors = value_placement_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(keys, expected);
    assert_eq!(descriptors, expected);
    assert_eq!(
        ValuePlacement::ALL
            .iter()
            .map(|placement| {
                let descriptor = placement.descriptor();
                (
                    descriptor.selected_item,
                    descriptor.posting,
                    descriptor.captures,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (true, false, false),
            (true, false, true),
            (false, true, false),
            (true, true, true),
        ]
    );
    assert_eq!(
        validate_value_placement_registration_keys(&keys, &descriptors),
        Ok(())
    );
    assert_eq!(
        validate_value_placement_registration_keys(&keys, &keys[..3]),
        Err(ValuePlacementRegistryError::Missing {
            keys: vec!["detail_match_filter_output".to_string()]
        })
    );
    let mut duplicate = keys.clone();
    duplicate.push("discovery_capture_source".to_string());
    assert_eq!(
        validate_value_placement_registration_keys(&keys, &duplicate),
        Err(ValuePlacementRegistryError::Duplicate {
            keys: vec!["discovery_capture_source".to_string()]
        })
    );
}

#[test]
fn placement_matrix_admits_only_available_context() {
    let cases = [
        (
            ValuePlacement::DiscoveryCaptureSource,
            json!({"type":"json_path","jsonPath":"$.title"}),
            true,
        ),
        (
            ValuePlacement::DiscoveryCaptureSource,
            json!({"type":"capture","key":"slug"}),
            false,
        ),
        (
            ValuePlacement::DiscoveryCaptureSource,
            json!({"type":"posting_meta","key":"jobId"}),
            false,
        ),
        (
            ValuePlacement::DiscoveryFilterOutput,
            json!({"type":"capture","key":"slug"}),
            true,
        ),
        (
            ValuePlacement::DiscoveryFilterOutput,
            json!({"type":"posting_meta","key":"jobId"}),
            false,
        ),
        (
            ValuePlacement::DetailCaptureSource,
            json!({"type":"posting_meta","key":"jobId"}),
            true,
        ),
        (
            ValuePlacement::DetailCaptureSource,
            json!({"type":"json_path","jsonPath":"$.title"}),
            false,
        ),
        (
            ValuePlacement::DetailCaptureSource,
            json!({"type":"capture","key":"slug"}),
            false,
        ),
        (
            ValuePlacement::DetailMatchFilterOutput,
            json!({"type":"posting_meta","key":"jobId"}),
            true,
        ),
        (
            ValuePlacement::DetailMatchFilterOutput,
            json!({"type":"json_path","jsonPath":"$.title"}),
            true,
        ),
        (
            ValuePlacement::DetailMatchFilterOutput,
            json!({"type":"capture","key":"slug"}),
            true,
        ),
    ];
    for (placement, authored, accepted) in cases {
        let result = compile_value(
            &expression(authored.clone()),
            &context(placement, Some(ParseType::Json)),
        );
        assert_eq!(result.is_ok(), accepted, "{placement:?}: {authored}");
    }
}

#[test]
fn declared_keys_templates_and_recursive_children_use_the_same_placement() {
    let unknown_source = compile_value(
        &expression(json!({"type":"source_config","key":"missing"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(
        unknown_source.kind,
        ValueCompileErrorKind::UnknownSourceConfigKey
    );

    let unknown_meta = compile_value(
        &expression(json!({"type":"posting_meta","key":"missing"})),
        &context(
            ValuePlacement::DetailMatchFilterOutput,
            Some(ParseType::Json),
        ),
    )
    .unwrap_err();
    assert_eq!(
        unknown_meta.kind,
        ValueCompileErrorKind::UnknownPostingMetaKey
    );

    let nested_capture = compile_value(
        &expression(json!({
            "type":"combine",
            "parts":[{"value":{"type":"template","template":"{{captures:slug}}"}}]
        })),
        &context(
            ValuePlacement::DiscoveryCaptureSource,
            Some(ParseType::Json),
        ),
    )
    .unwrap_err();
    assert_eq!(nested_capture.kind, ValueCompileErrorKind::Template);
    assert_eq!(nested_capture.path, "/parts/0/value/template");

    let fallback = compile_value(
        &expression(json!({
            "type":"first_non_empty",
            "candidates":[
                {"type":"const","value":"fallback"},
                {"type":"capture","key":"slug"}
            ]
        })),
        &context(
            ValuePlacement::DiscoveryCaptureSource,
            Some(ParseType::Json),
        ),
    )
    .unwrap_err();
    assert_eq!(fallback.kind, ValueCompileErrorKind::CaptureUnavailable);
    assert_eq!(fallback.path, "/candidates/1");
}

#[test]
fn document_compatibility_and_scalar_sequence_shape_are_typed() {
    let incompatible = compile_value(
        &expression(json!({"type":"css_text","selector":".title"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(
        incompatible.kind,
        ValueCompileErrorKind::DocumentIncompatible
    );

    let scalar = compile_value(
        &expression(json!({"type":"const","value":"x"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap();
    assert_eq!(scalar.shape(), ValueShape::Scalar);
    let sequence = compile_value(
        &expression(json!({"type":"const","value":"x","cardinality":"all"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap();
    assert_eq!(sequence.shape(), ValueShape::Sequence);

    let selected = SelectedSequence::new(vec![
        SelectedItem::Text("second".to_string()),
        SelectedItem::Text("first".to_string()),
    ]);
    let SelectedValueCarrier::Sequence(selected) = SelectedValueCarrier::from(selected) else {
        panic!("selected sequence must retain sequence shape")
    };
    assert_eq!(selected.len(), 2);
}

#[test]
fn recursive_limits_accept_the_boundary_and_reject_one_over() {
    let at_depth = nested_combine(VALUE_MAX_DEPTH);
    assert!(compile_value(
        &at_depth,
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let over_depth = compile_value(
        &nested_combine(VALUE_MAX_DEPTH + 1),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over_depth.kind, ValueCompileErrorKind::DepthLimitExceeded);
    assert_eq!(over_depth.actual, Some(VALUE_MAX_DEPTH + 1));

    let parts = (0..VALUE_MAX_NODES - 1)
        .map(|index| json!({ "value": { "type": "const", "value": index } }))
        .collect::<Vec<_>>();
    assert!(compile_value(
        &expression(json!({ "type": "combine", "parts": parts })),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let parts = (0..VALUE_MAX_NODES)
        .map(|index| json!({ "value": { "type": "const", "value": index } }))
        .collect::<Vec<_>>();
    let over_nodes = compile_value(
        &expression(json!({ "type": "combine", "parts": parts })),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over_nodes.kind, ValueCompileErrorKind::NodeLimitExceeded);
    assert_eq!(over_nodes.actual, Some(VALUE_MAX_NODES + 1));

    let empty = compile_value(
        &expression(json!({"type":"first_non_empty","candidates":[]})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(empty.kind, ValueCompileErrorKind::EmptyCandidates);

    let candidates = (0..VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES)
        .map(|index| json!({"type":"const","value":index}))
        .collect::<Vec<_>>();
    assert!(compile_value(
        &expression(json!({"type":"first_non_empty","candidates":candidates})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let candidates = (0..=VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES)
        .map(|index| json!({"type":"const","value":index}))
        .collect::<Vec<_>>();
    let over = compile_value(
        &expression(json!({"type":"first_non_empty","candidates":candidates})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over.kind, ValueCompileErrorKind::CandidateLimitExceeded);
    assert_eq!(over.actual, Some(VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES + 1));
}

#[test]
fn thirteen_value_keys_have_exact_cross_layer_registration_parity() {
    let expected = vec![
        "const",
        "template",
        "source_config",
        "posting_meta",
        "capture",
        "item_field",
        "json_path",
        "xml_text",
        "xml_element",
        "css_text",
        "css_attribute",
        "combine",
        "first_non_empty",
    ];
    let kind_keys = ValueKind::ALL
        .iter()
        .map(|kind| kind.key().to_string())
        .collect::<Vec<_>>();
    let serde_samples = [
        json!({"type":"const","value":"x"}),
        json!({"type":"template","template":"literal"}),
        json!({"type":"source_config","key":"tenant"}),
        json!({"type":"posting_meta","key":"jobId"}),
        json!({"type":"capture","key":"slug"}),
        json!({"type":"item_field","key":"title"}),
        json!({"type":"json_path","jsonPath":"$.title"}),
        json!({"type":"xml_text","textPath":"title"}),
        json!({"type":"xml_element","element":"job"}),
        json!({"type":"css_text","selector":".title"}),
        json!({"type":"css_attribute","selector":"a","attribute":"href"}),
        json!({"type":"combine","parts":[{"value":{"type":"const","value":"x"}}]}),
        json!({"type":"first_non_empty","candidates":[{"type":"const","value":"x"}]}),
    ];
    let serde_keys = serde_samples
        .into_iter()
        .map(|sample| {
            let expression: FieldExpression = serde_json::from_value(sample).unwrap();
            serde_json::to_value(expression).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    let registrations = value_descriptors()
        .iter()
        .map(|entry| entry.key.to_string())
        .collect::<Vec<_>>();
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../src/schema/profile-dsl/extract.schema.json"
    ))
    .unwrap();
    let schema_keys = schema["$defs"]["fieldExpression"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let reference = entry["$ref"].as_str().unwrap();
            let definition = reference.rsplit('/').next().unwrap();
            let definition = &schema["$defs"][definition];
            let typed_object = definition
                .get("properties")
                .map(|_| definition)
                .or_else(|| {
                    definition["allOf"].as_array().and_then(|parts| {
                        parts.iter().find(|part| part.get("properties").is_some())
                    })
                })
                .unwrap();
            typed_object["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        schema_keys,
        expected.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    assert_eq!(serde_keys, schema_keys);
    assert_eq!(kind_keys, schema_keys);
    assert_eq!(registrations, schema_keys);
    assert_eq!(
        validate_value_registration_keys(&schema_keys, &serde_keys, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_value_registration_keys(&schema_keys, &serde_keys[..12], &registrations),
        Err(ValueRegistryError::Missing {
            layer: "serde",
            keys: vec!["first_non_empty".to_string()]
        })
    );
    let mut duplicate = registrations.clone();
    duplicate.push("const".to_string());
    assert_eq!(
        validate_value_registration_keys(&schema_keys, &serde_keys, &duplicate),
        Err(ValueRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["const".to_string()]
        })
    );
    let mut extra = registrations.clone();
    extra.push("unregistered_extra".to_string());
    assert_eq!(
        validate_value_registration_keys(&schema_keys, &serde_keys, &extra),
        Err(ValueRegistryError::Extra {
            layer: "registration",
            keys: vec!["unregistered_extra".to_string()]
        })
    );
}

#[test]
fn direct_serde_const_accepts_only_string_number_and_boolean() {
    for value in [json!("text"), json!(42), json!(false)] {
        assert!(
            serde_json::from_value::<FieldExpression>(json!({"type":"const","value":value}))
                .is_ok()
        );
    }
    for value in [json!(null), json!([]), json!({})] {
        assert!(
            serde_json::from_value::<FieldExpression>(json!({"type":"const","value":value}))
                .is_err()
        );
    }
}

#[test]
fn combine_and_empty_authored_multiple_preserve_order_and_missing_rules() {
    let combined = expression(json!({
        "type":"combine",
        "join":"/",
        "parts":[
            {"value":{"type":"const","value":" first "}},
            {"value":{"type":"source_config","key":"tenant"}},
            {"value":{"type":"capture","key":"slug"},"optional":true}
        ]
    }));
    assert_eq!(
        evaluate(&combined, json!({}), []),
        Ok(CompiledValueResult::Scalar(Some("first/acme".to_string())))
    );

    let required_missing = expression(json!({
        "type":"combine",
        "parts":[{"value":{"type":"capture","key":"slug"}}]
    }));
    let error = evaluate(&required_missing, json!({}), []).unwrap_err();
    assert_eq!(
        error.kind,
        ValueEvaluationErrorKind::RequiredCombinePartMissing
    );
    assert_eq!(error.relative_path, "/parts/0/value");

    let list: ListFieldExpression = serde_json::from_value(json!([])).unwrap();
    assert_eq!(
        compile_list_value(
            &list,
            &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json))
        )
        .unwrap(),
        CompiledListValue::Multiple(Vec::new())
    );
}

#[test]
fn first_non_empty_is_ordered_bounded_scalar_fallback() {
    let fallback = expression(json!({
        "type":"first_non_empty",
        "candidates":[
            {"type":"const","value":"   "},
            {"type":"const","value":false},
            {"type":"const","value":"later"}
        ],
        "transforms":[{"type":"to_string"}]
    }));
    assert_eq!(
        evaluate(&fallback, json!({}), []),
        Ok(CompiledValueResult::Scalar(Some("false".to_string())))
    );

    let zero = expression(json!({
        "type":"first_non_empty",
        "candidates":[{"type":"const","value":0},{"type":"const","value":1}]
    }));
    assert_eq!(
        evaluate(&zero, json!({}), []),
        Ok(CompiledValueResult::Scalar(Some("0".to_string())))
    );

    let null_is_empty = expression(json!({
        "type":"first_non_empty",
        "candidates":[
            {"type":"json_path","jsonPath":"$.candidate"},
            {"type":"const","value":false}
        ]
    }));
    assert_eq!(
        evaluate(&null_is_empty, json!({"candidate": null}), []),
        Ok(CompiledValueResult::Scalar(Some("false".to_string())))
    );

    let all_empty = expression(json!({
        "type":"first_non_empty",
        "candidates":[{"type":"capture","key":"slug"}]
    }));
    assert_eq!(
        evaluate(&all_empty, json!({}), []),
        Ok(CompiledValueResult::Scalar(Some(String::new())))
    );

    let null_to_string_is_hard = expression(json!({
        "type":"first_non_empty",
        "candidates":[
            {"type":"json_path","jsonPath":"$.candidate","transforms":[{"type":"to_string"}]},
            {"type":"const","value":"must-not-win"}
        ]
    }));
    let error = evaluate(&null_to_string_is_hard, json!({"candidate": null}), []).unwrap_err();
    assert_eq!(error.kind, ValueEvaluationErrorKind::TransformTypeMismatch);
    assert_eq!(error.relative_path, "/candidates/0");

    let hard = expression(json!({
        "type":"first_non_empty",
        "candidates":[
            {"type":"const","value":"%ZZ","transforms":[{"type":"to_string"},{"type":"url_decode"}]},
            {"type":"const","value":"must-not-win"}
        ]
    }));
    let error = evaluate(&hard, json!({}), []).unwrap_err();
    assert_eq!(
        error.kind,
        ValueEvaluationErrorKind::TransformInvalidPercentEncoding
    );
    assert_eq!(error.relative_path, "/candidates/0");

    let wrapper_failure = expression(json!({
        "type":"first_non_empty",
        "candidates":[
            {"type":"const","value":"%ZZ"},
            {"type":"const","value":"must-not-backtrack"}
        ],
        "transforms":[{"type":"url_decode"}]
    }));
    let error = evaluate(&wrapper_failure, json!({}), []).unwrap_err();
    assert_eq!(
        error.kind,
        ValueEvaluationErrorKind::TransformInvalidPercentEncoding
    );
    assert_eq!(error.relative_path, "");
}

#[test]
fn fallback_rejects_sequence_candidates_nesting_and_wrapper_cardinality() {
    let all = compile_value(
        &expression(json!({"type":"first_non_empty","candidates":[{"type":"const","value":"x","cardinality":"all"}]})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    ).unwrap_err();
    assert_eq!(all.kind, ValueCompileErrorKind::CandidateSequence);

    let nested = compile_value(
        &expression(json!({"type":"first_non_empty","candidates":[{"type":"combine","parts":[{"value":{"type":"first_non_empty","candidates":[{"type":"const","value":"x"}]}}]}]})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    ).unwrap_err();
    assert_eq!(nested.kind, ValueCompileErrorKind::NestedFallback);

    for authored in [
        json!({"type":"first_non_empty","candidates":[{"type":"const","value":"x","transforms":[{"type":"to_string"},{"type":"split","separator":"-"}]}]}),
        json!({"type":"first_non_empty","candidates":[{"type":"const","value":"x"}],"transforms":[{"type":"split","separator":"-"}]}),
    ] {
        let sequence = compile_value(
            &expression(authored),
            &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
        )
        .unwrap_err();
        assert_eq!(sequence.kind, ValueCompileErrorKind::CandidateSequence);
    }

    assert!(serde_json::from_value::<FieldExpression>(json!({
        "type":"first_non_empty","candidates":[{"type":"const","value":"x"}],"cardinality":"one"
    }))
    .is_err());
}

fn evaluate<const N: usize>(
    authored: &FieldExpression,
    selected: serde_json::Value,
    captures: [(&str, &str); N],
) -> Result<CompiledValueResult, job_radar_lib::ValueEvaluationError> {
    let compiled = compile_value(
        authored,
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap();
    let selected = SelectedItem::Json(&selected);
    let source_config = serde_json::from_value(json!({"tenant":"acme"})).unwrap();
    let captures = captures
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    let context = DiscoveryFilterOutputValueContext {
        source: SourceValueView {
            source_name: "Acme",
            source_config: &source_config,
        },
        selected: &selected,
        captures: &captures,
    };
    evaluate_discovery_output_value(&compiled, &context)
}

fn nested_combine(depth: usize) -> FieldExpression {
    let mut value = json!({"type":"const","value":"leaf"});
    for _ in 1..depth {
        value = json!({"type":"combine","parts":[{"value":value}]});
    }
    expression(value)
}
