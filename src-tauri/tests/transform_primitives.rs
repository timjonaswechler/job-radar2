use job_radar_lib::{
    compile_transform_pipeline, transform_descriptors, validate_transform_registration_keys,
    CompileTransformErrorKind, Transform, TransformErrorKind, TransformKind,
    TransformRegistryError, TransformShape, TransformValue,
};
use serde_json::json;

fn authored(value: serde_json::Value) -> Transform {
    serde_json::from_value(value).unwrap()
}

fn text_values(shape: TransformShape<'_, '_>) -> Vec<String> {
    match shape {
        TransformShape::Scalar(TransformValue::Text(value)) => vec![value],
        TransformShape::Sequence(values) => values
            .into_iter()
            .map(|value| match value {
                TransformValue::Text(value) => value,
                _ => panic!("expected text output"),
            })
            .collect(),
        _ => panic!("expected text output"),
    }
}

#[test]
fn transform_family_has_exact_cross_layer_registration_parity() {
    let expected = vec![
        "trim",
        "normalize_whitespace",
        "html_to_text",
        "url_decode",
        "slug_to_title",
        "dedupe",
        "to_string",
        "split",
        "join",
        "regex_replace",
    ];
    let schema = transform_schema_keys();
    let serde = TransformKind::ALL
        .iter()
        .map(|kind| kind.key().to_string())
        .collect::<Vec<_>>();
    let registrations = transform_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(schema, expected);
    assert_eq!(serde, expected);
    assert_eq!(registrations, expected);
    assert_eq!(
        validate_transform_registration_keys(&schema, &serde, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_transform_registration_keys(&serde, &serde[..9], &registrations),
        Err(TransformRegistryError::Missing {
            layer: "serde",
            keys: vec!["regex_replace".to_string()]
        })
    );
    let mut duplicate = registrations.clone();
    duplicate.push("trim".to_string());
    assert_eq!(
        validate_transform_registration_keys(&serde, &serde, &duplicate),
        Err(TransformRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["trim".to_string()]
        })
    );
}

#[test]
fn only_canonical_names_and_split_camel_case_options_are_authored() {
    for legacy in [
        "normalizeWhitespace",
        "htmlToText",
        "urlDecode",
        "slugToTitle",
        "toString",
    ] {
        assert!(
            serde_json::from_value::<Transform>(json!({"type": legacy})).is_err(),
            "{legacy}"
        );
    }
    let split =
        authored(json!({"type":"split", "separator":";", "trimParts":true, "dropEmpty":true}));
    assert_eq!(
        serde_json::to_value(&split).unwrap(),
        json!({
            "type":"split", "separator":";", "trimParts":true, "dropEmpty":true
        })
    );
    let defaults = authored(json!({"type":"split", "separator":";"}));
    assert_eq!(
        serde_json::to_value(defaults).unwrap(),
        json!({"type":"split", "separator":";"})
    );
    assert!(serde_json::from_value::<Transform>(
        json!({"type":"split", "separator":";", "trim_parts":true})
    )
    .is_err());
}

#[test]
fn ordered_pipeline_preserves_shape_and_sequence_order() {
    let transforms = [
        authored(json!({"type":"split", "separator":";", "trimParts":true, "dropEmpty":true})),
        authored(json!({"type":"normalize_whitespace"})),
        authored(json!({"type":"dedupe"})),
        authored(json!({"type":"join", "separator":" | "})),
        authored(json!({"type":"regex_replace", "pattern":"Remote", "replacement":"Hybrid"})),
    ];
    let plan = compile_transform_pipeline(&transforms).unwrap();
    let output = plan
        .execute(TransformShape::Scalar(TransformValue::Text(
            " Berlin ; Remote ; Berlin ;  ".to_string(),
        )))
        .unwrap();
    assert_eq!(output.shape_name(), "scalar");
    assert_eq!(text_values(output), vec!["Berlin | Hybrid"]);
}

#[test]
fn split_options_cover_all_default_and_enabled_combinations() {
    let cases = [
        (false, false, vec![" a ", "", " b "]),
        (true, false, vec!["a", "", "b"]),
        (false, true, vec![" a ", " b "]),
        (true, true, vec!["a", "b"]),
    ];
    for (trim_parts, drop_empty, expected) in cases {
        let plan = compile_transform_pipeline(&[authored(json!({
            "type": "split",
            "separator": ";",
            "trimParts": trim_parts,
            "dropEmpty": drop_empty
        }))])
        .unwrap();
        let output = plan
            .execute(TransformShape::Scalar(TransformValue::Text(
                " a ;; b ".to_string(),
            )))
            .unwrap();
        assert_eq!(output.shape_name(), "sequence");
        assert_eq!(text_values(output), expected);
    }
}

#[test]
fn canonical_text_transforms_have_frozen_behavior() {
    let cases = [
        (json!({"type":"trim"}), "  hello  ", "hello"),
        (
            json!({"type":"normalize_whitespace"}),
            " a\n  b\t c ",
            "a b c",
        ),
        (
            json!({"type":"html_to_text"}),
            "<p>Hello <b>world</b></p>",
            "Hello world",
        ),
        (
            json!({"type":"slug_to_title"}),
            "senior-software_engineer",
            "Senior Software Engineer",
        ),
    ];
    for (transform, input, expected) in cases {
        let output = compile_transform_pipeline(&[authored(transform)])
            .unwrap()
            .execute(TransformShape::Scalar(TransformValue::Text(
                input.to_string(),
            )))
            .unwrap();
        assert_eq!(text_values(output), vec![expected]);
    }
}

#[test]
fn to_string_truthfully_converts_every_admitted_scalar_kind() {
    let json_string = json!("hello");
    let json_number = json!(42.5);
    let json_bool = json!(true);
    let plan = compile_transform_pipeline(&[authored(json!({"type":"to_string"}))]).unwrap();
    let cases = [
        (TransformValue::Text("text".to_string()), "text"),
        (TransformValue::Json(json_string), "hello"),
        (TransformValue::Json(json_number), "42.5"),
        (TransformValue::Json(json_bool), "true"),
    ];
    for (input, expected) in cases {
        let output = plan.clone().execute(TransformShape::Scalar(input)).unwrap();
        assert_eq!(text_values(output), vec![expected]);
    }

    for rejected in [json!(null), json!([1]), json!({"a":1})] {
        let error = plan
            .clone()
            .execute(TransformShape::Scalar(TransformValue::Json(rejected)))
            .unwrap_err();
        assert_eq!(error.kind, TransformErrorKind::TypeMismatch);
    }

    let xml = roxmltree::Document::parse("<title>Hello <b>XML</b></title>").unwrap();
    let output = plan
        .clone()
        .execute(TransformShape::Scalar(TransformValue::Xml(
            xml.root_element(),
        )))
        .unwrap();
    assert_eq!(text_values(output), vec!["Hello XML"]);

    let html = dom_query::Document::from("<p>Hello <b>HTML</b></p>");
    let node = html.select("p").nodes().first().unwrap().clone();
    let output = plan
        .execute(TransformShape::Scalar(TransformValue::Html(node)))
        .unwrap();
    assert_eq!(text_values(output), vec!["Hello HTML"]);
}

#[test]
fn url_decode_is_strict_non_lossy_and_preserves_plus() {
    let plan = compile_transform_pipeline(&[authored(json!({"type":"url_decode"}))]).unwrap();
    for (input, expected) in [
        ("Senior%20Engineer", "Senior Engineer"),
        ("%C3%BC", "ü"),
        ("C++", "C++"),
        ("%2B", "+"),
    ] {
        let output = plan
            .clone()
            .execute(TransformShape::Scalar(TransformValue::Text(
                input.to_string(),
            )))
            .unwrap();
        assert_eq!(text_values(output), vec![expected]);
    }
    for input in ["%", "%2", "%GG"] {
        let error = plan
            .clone()
            .execute(TransformShape::Scalar(TransformValue::Text(
                input.to_string(),
            )))
            .unwrap_err();
        assert_eq!(error.kind, TransformErrorKind::InvalidPercentEncoding);
    }
    let error = plan
        .execute(TransformShape::Scalar(TransformValue::Text(
            "%FF".to_string(),
        )))
        .unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::InvalidUtf8);
}

#[test]
fn invalid_configuration_is_rejected_during_compilation() {
    let error = compile_transform_pipeline(&[authored(json!({"type":"split", "separator":""}))])
        .unwrap_err();
    assert_eq!(error.kind, CompileTransformErrorKind::EmptySeparator);
    let error = compile_transform_pipeline(&[authored(
        json!({"type":"regex_replace", "pattern":"(", "replacement":"x"}),
    )])
    .unwrap_err();
    assert_eq!(error.kind, CompileTransformErrorKind::InvalidRegex);

    let plan = compile_transform_pipeline(&[authored(json!({
        "type":"regex_replace", "pattern":"a+", "replacement":"x"
    }))])
    .unwrap();
    let serialized = serde_json::to_value(&plan).unwrap();
    let restored: job_radar_lib::CompiledTransformPipeline =
        serde_json::from_value(serialized).unwrap();
    assert_eq!(plan, restored);
}

fn transform_schema_keys() -> Vec<String> {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../src/schema/profile-dsl/transform.schema.json"
    ))
    .unwrap();
    let mut keys = schema["$defs"]["simpleTransform"]["properties"]["type"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for definition in ["splitTransform", "joinTransform", "regexReplaceTransform"] {
        keys.push(
            schema["$defs"][definition]["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    keys
}

#[test]
fn one_bad_element_fails_without_partial_output_or_value_leakage() {
    let plan = compile_transform_pipeline(&[authored(json!({"type":"url_decode"}))]).unwrap();
    let error = plan
        .execute(TransformShape::Sequence(vec![
            TransformValue::Text("valid".to_string()),
            TransformValue::Text("secret-%GG-value".to_string()),
        ]))
        .unwrap_err();
    assert_eq!(error.transform_index, 0);
    assert_eq!(error.value_index, Some(1));
    assert_eq!(error.kind, TransformErrorKind::InvalidPercentEncoding);
    assert!(!error.message.contains("secret"));
}
