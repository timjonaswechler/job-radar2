use std::collections::{BTreeSet, VecDeque};

use job_radar_lib::{
    capture_descriptors, compile_captures, evaluate_compiled_captures,
    validate_capture_registration_keys, CaptureCompileErrorKind, CaptureEvaluationErrorKind,
    CaptureRegistryError, CaptureRule, Captures, CompiledValueResult, ValueCompileContext,
    ValuePlacement,
};
use serde_json::json;

fn captures(value: serde_json::Value) -> Captures {
    serde_json::from_value(value).unwrap()
}

fn context() -> ValueCompileContext {
    ValueCompileContext {
        placement: ValuePlacement::DiscoveryCaptureSource,
        document_type: None,
        source_config_keys: BTreeSet::new(),
        posting_meta_keys: BTreeSet::new(),
        capture_keys: BTreeSet::new(),
    }
}

#[test]
fn capture_family_has_one_cross_layer_registration() {
    let schema_document: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/src/schema/profile-dsl/select.schema.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let schema = vec![schema_document["$defs"]["captureRule"]["x-primitive"]
        .as_str()
        .unwrap()
        .to_string()];
    let authored: CaptureRule = serde_json::from_value(json!({
        "from": { "type": "const", "value": "x" },
        "pattern": "^(?<capture>x)$"
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&authored).unwrap(),
        json!({
            "from": { "type": "const", "value": "x" },
            "pattern": "^(?<capture>x)$"
        })
    );
    assert!(serde_json::from_value::<CaptureRule>(json!({
        "from": { "type": "const", "value": "x" },
        "pattern": "^(?<capture>x)$",
        "group": "capture"
    }))
    .is_err());
    let serde = vec![CaptureRule::PRIMITIVE_KEY.to_string()];
    let registrations = capture_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();

    assert_eq!(registrations, schema);
    assert_eq!(
        validate_capture_registration_keys(&schema, &serde, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_capture_registration_keys(&schema, &[], &registrations),
        Err(CaptureRegistryError::Missing {
            layer: "serde",
            keys: vec!["capture".to_string()],
        })
    );
    assert_eq!(
        validate_capture_registration_keys(
            &schema,
            &serde,
            &["capture".to_string(), "capture".to_string()]
        ),
        Err(CaptureRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["capture".to_string()],
        })
    );
}

#[test]
fn named_captures_select_the_outer_key_and_preserve_authored_order() {
    let plan = compile_captures(
        &serde_json::from_str::<Captures>(
            r#"{
            "zeta": {
                "from": {"type":"const", "value":"  z value  "},
                "pattern":"^\\s*(?<ignored>z)(?<zeta> value)(?<value>\\s*)$"
            },
            "alpha": {
                "from": {"type":"const", "value":"a  value"},
                "pattern":"^(a)(?<alpha>  value)$"
            }
        }"#,
        )
        .unwrap(),
        &context(),
    )
    .unwrap();

    let mut calls = 0;
    let mut values = VecDeque::from(["  z value  ", "a  value"]);
    let outputs = evaluate_compiled_captures(&plan, |_| {
        calls += 1;
        Ok(CompiledValueResult::Scalar(Some(
            values.pop_front().unwrap().to_string(),
        )))
    })
    .unwrap();

    assert_eq!(calls, 2);
    assert_eq!(outputs[0].key, "zeta");
    assert_eq!(outputs[0].value, "value");
    assert_eq!(outputs[1].key, "alpha");
    assert_eq!(outputs[1].value, "value");
}

#[test]
fn invalid_regex_and_missing_selected_named_group_fail_compilation() {
    let invalid = compile_captures(
        &captures(json!({
            "tenant": {"from":{"type":"const","value":"x"}, "pattern":"("}
        })),
        &context(),
    )
    .unwrap_err();
    assert_eq!(invalid.kind, CaptureCompileErrorKind::InvalidRegex);
    assert_eq!(invalid.path, "/pattern");
    assert_eq!(
        invalid.message,
        "Capture pattern is invalid Rust regex syntax"
    );

    let missing = compile_captures(
        &captures(json!({
            "tenant": {
                "from":{"type":"const","value":"x"},
                "pattern":"^(?<other>x)(x)$"
            }
        })),
        &context(),
    )
    .unwrap_err();
    assert_eq!(missing.kind, CaptureCompileErrorKind::NamedGroupMissing);
    assert_eq!(missing.capture_key, "tenant");
    assert_eq!(
        missing.message,
        "Capture pattern must declare a named group matching its Capture key"
    );
}

#[test]
fn capture_failures_are_atomic_but_every_rule_observes_the_complete_input() {
    let plan = compile_captures(
        &serde_json::from_str::<Captures>(
            r#"{
            "noMatch": {
                "from":{"type":"const","value":"source"},
                "pattern":"^(?<noMatch>different)$"
            },
            "optional": {
                "from":{"type":"const","value":"source"},
                "pattern":"^source(?<optional>suffix)?$"
            },
            "empty": {
                "from":{"type":"const","value":"   "},
                "pattern":"^(?<empty>.*)$"
            }
        }"#,
        )
        .unwrap(),
        &context(),
    )
    .unwrap();

    let mut calls = 0;
    let mut values = VecDeque::from(["source", "source", "   "]);
    let errors = evaluate_compiled_captures(&plan, |_| {
        calls += 1;
        Ok(CompiledValueResult::Scalar(Some(
            values.pop_front().unwrap().to_string(),
        )))
    })
    .unwrap_err();

    assert_eq!(calls, 3);
    assert_eq!(
        errors.iter().map(|error| error.kind).collect::<Vec<_>>(),
        vec![
            CaptureEvaluationErrorKind::PatternNotMatched,
            CaptureEvaluationErrorKind::NamedGroupUnmatched,
            CaptureEvaluationErrorKind::Empty,
        ]
    );
}

#[test]
fn compiled_capture_plan_retains_structure_but_not_runtime_values() {
    let plan = compile_captures(
        &captures(json!({
            "tenant": {
                "from":{"type":"source_config","key":"tenant"},
                "pattern":"^(?<tenant>.+)$"
            }
        })),
        &ValueCompileContext {
            source_config_keys: ["tenant".to_string()].into_iter().collect(),
            ..context()
        },
    )
    .unwrap();
    let serialized = serde_json::to_string(&plan).unwrap();
    assert!(serialized.contains("tenant"));
    assert!(serialized.contains("(?<tenant>.+)"));
    assert!(!serialized.contains("runtime-secret-sentinel"));

    let restored: job_radar_lib::CompiledCapturePlan = serde_json::from_str(&serialized).unwrap();
    let outputs = evaluate_compiled_captures(&restored, |_| {
        Ok(CompiledValueResult::Scalar(Some(
            "runtime-secret-sentinel".to_string(),
        )))
    })
    .unwrap();
    assert_eq!(outputs[0].value, "runtime-secret-sentinel");
    assert!(!serde_json::to_string(&restored)
        .unwrap()
        .contains("runtime-secret-sentinel"));
}
