use std::collections::BTreeSet;

use job_radar_lib::{
    compile_value_foundation, validate_value_placement_registration_keys,
    value_placement_descriptors, FieldExpression, ParseType, SelectedItem, SelectedSequence,
    SelectedValueCarrier, ValueCompileContext, ValueCompileErrorKind, ValuePlacement,
    ValuePlacementRegistryError, ValueShape, VALUE_MAX_DEPTH, VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES,
    VALUE_MAX_NODES,
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
        let result = compile_value_foundation(
            &expression(authored.clone()),
            &context(placement, Some(ParseType::Json)),
        );
        assert_eq!(result.is_ok(), accepted, "{placement:?}: {authored}");
    }
}

#[test]
fn declared_keys_templates_and_recursive_children_use_the_same_placement() {
    let unknown_source = compile_value_foundation(
        &expression(json!({"type":"source_config","key":"missing"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(
        unknown_source.kind,
        ValueCompileErrorKind::UnknownSourceConfigKey
    );

    let unknown_meta = compile_value_foundation(
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

    let nested_capture = compile_value_foundation(
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

    let fallback = compile_value_foundation(
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
    let incompatible = compile_value_foundation(
        &expression(json!({"type":"css_text","selector":".title"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(
        incompatible.kind,
        ValueCompileErrorKind::DocumentIncompatible
    );

    let scalar = compile_value_foundation(
        &expression(json!({"type":"const","value":"x"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap();
    assert_eq!(scalar.shape, ValueShape::Scalar);
    let sequence = compile_value_foundation(
        &expression(json!({"type":"const","value":"x","cardinality":"all"})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap();
    assert_eq!(sequence.shape, ValueShape::Sequence);

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
    assert!(compile_value_foundation(
        &at_depth,
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let over_depth = compile_value_foundation(
        &nested_combine(VALUE_MAX_DEPTH + 1),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over_depth.kind, ValueCompileErrorKind::DepthLimitExceeded);
    assert_eq!(over_depth.actual, Some(VALUE_MAX_DEPTH + 1));

    let parts = (0..VALUE_MAX_NODES - 1)
        .map(|index| json!({ "value": { "type": "const", "value": index } }))
        .collect::<Vec<_>>();
    assert!(compile_value_foundation(
        &expression(json!({ "type": "combine", "parts": parts })),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let parts = (0..VALUE_MAX_NODES)
        .map(|index| json!({ "value": { "type": "const", "value": index } }))
        .collect::<Vec<_>>();
    let over_nodes = compile_value_foundation(
        &expression(json!({ "type": "combine", "parts": parts })),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over_nodes.kind, ValueCompileErrorKind::NodeLimitExceeded);
    assert_eq!(over_nodes.actual, Some(VALUE_MAX_NODES + 1));

    let empty = compile_value_foundation(
        &expression(json!({"type":"first_non_empty","candidates":[]})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(empty.kind, ValueCompileErrorKind::EmptyCandidates);

    let candidates = (0..VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES)
        .map(|index| json!({"type":"const","value":index}))
        .collect::<Vec<_>>();
    assert!(compile_value_foundation(
        &expression(json!({"type":"first_non_empty","candidates":candidates})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .is_ok());
    let candidates = (0..=VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES)
        .map(|index| json!({"type":"const","value":index}))
        .collect::<Vec<_>>();
    let over = compile_value_foundation(
        &expression(json!({"type":"first_non_empty","candidates":candidates})),
        &context(ValuePlacement::DiscoveryFilterOutput, Some(ParseType::Json)),
    )
    .unwrap_err();
    assert_eq!(over.kind, ValueCompileErrorKind::CandidateLimitExceeded);
    assert_eq!(over.actual, Some(VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES + 1));
}

fn nested_combine(depth: usize) -> FieldExpression {
    let mut value = json!({"type":"const","value":"leaf"});
    for _ in 1..depth {
        value = json!({"type":"combine","parts":[{"value":value}]});
    }
    expression(value)
}
