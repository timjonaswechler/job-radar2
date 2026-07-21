use job_radar_lib::{
    compile_parse, parse_descriptors, validate_parse_registration_keys, AccessPathFragment,
    BrowserRenderedText, CompiledParse, DecodedHttpText, Parse, ParseFailureKind, ParseInput,
    ParseInputKind, ParseRegistryError, ParseType, ParseTypeFragment,
};

#[test]
fn parse_family_has_exact_cross_layer_registration_parity() {
    let schema_keys = parse_schema_keys();
    let serde_keys = ParseType::ALL
        .iter()
        .map(|parse_type| parse_type.key().to_string())
        .collect::<Vec<_>>();
    let fragment_serde_keys = ParseTypeFragment::ALL
        .iter()
        .map(|parse_type| parse_type.key().to_string())
        .collect::<Vec<_>>();
    let registration_keys = parse_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();

    assert_eq!(schema_keys, vec!["json", "xml", "html"]);
    assert_eq!(serde_keys, schema_keys);
    assert_eq!(fragment_serde_keys, schema_keys);
    assert_eq!(parse_fragment_schema_keys(), schema_keys);
    assert_eq!(registration_keys, schema_keys);
    assert_eq!(
        validate_parse_registration_keys(&schema_keys, &serde_keys, &registration_keys),
        Ok(())
    );

    assert_eq!(
        validate_parse_registration_keys(
            &schema_keys,
            &["json".to_string(), "xml".to_string()],
            &registration_keys,
        ),
        Err(ParseRegistryError::Missing {
            layer: "serde",
            keys: vec!["html".to_string()],
        })
    );
    assert_eq!(
        validate_parse_registration_keys(
            &schema_keys,
            &serde_keys,
            &[
                "json".to_string(),
                "xml".to_string(),
                "html".to_string(),
                "html".to_string(),
            ],
        ),
        Err(ParseRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["html".to_string()],
        })
    );
}

#[test]
fn text_is_rejected_by_authored_fragment_and_compiled_serde() {
    let error = serde_json::from_value::<Parse>(serde_json::json!({ "type": "text" }))
        .expect_err("text must not remain an authored Parse variant");
    assert!(error.to_string().contains("unknown variant `text`"));

    serde_json::from_value::<Vec<AccessPathFragment>>(serde_json::json!([{
        "key": "feed",
        "discovery": {
            "strategies": [{ "key": "text", "parse": { "type": "text" } }]
        }
    }]))
    .expect_err("text must not remain in direct Source specialization Serde");

    serde_json::from_value::<CompiledParse>(serde_json::json!({
        "type": "text",
        "inputKind": "decoded_http"
    }))
    .expect_err("text must not remain a compiled Parse descriptor");
}

#[test]
fn json_xml_and_html_parse_complete_typed_inputs_into_one_document_family() {
    let json = compile_parse(
        &serde_json::from_value(serde_json::json!({ "type": "json" })).unwrap(),
        ParseInputKind::DecodedHttp,
    )
    .unwrap();
    let json_document = json
        .parse(ParseInput::DecodedHttp(DecodedHttpText::new(
            r#"{"jobs":[{"title":"Engineer"}]}"#,
        )))
        .unwrap();
    assert_eq!(
        json_document.as_json().unwrap()["jobs"][0]["title"],
        "Engineer"
    );

    let xml = compile_parse(
        &serde_json::from_value(serde_json::json!({ "type": "xml" })).unwrap(),
        ParseInputKind::DecodedHttp,
    )
    .unwrap();
    let xml_document = xml
        .parse(ParseInput::DecodedHttp(DecodedHttpText::new(
            "<jobs><job>Engineer</job></jobs>",
        )))
        .unwrap();
    assert_eq!(
        xml_document
            .as_xml()
            .unwrap()
            .root_element()
            .tag_name()
            .name(),
        "jobs"
    );

    let html = compile_parse(
        &serde_json::from_value(serde_json::json!({ "type": "html" })).unwrap(),
        ParseInputKind::BrowserRendered,
    )
    .unwrap();
    let html_document = html
        .parse(ParseInput::BrowserRendered(BrowserRenderedText::new(
            "<main><article>Engineer</article></main>",
        )))
        .unwrap();
    assert!(html_document.as_html().is_some());
}

#[test]
fn malformed_inputs_return_one_typed_failure_and_no_partial_document() {
    for (parse_type, input, expected_kind) in [
        ("json", "{\"jobs\":[", ParseFailureKind::MalformedJson),
        ("xml", "<jobs><job></jobs>", ParseFailureKind::MalformedXml),
        (
            "html",
            "<!doctype html><html><body><main><article></main></body></html>",
            ParseFailureKind::MalformedHtml,
        ),
    ] {
        let plan = compile_parse(
            &serde_json::from_value(serde_json::json!({ "type": parse_type })).unwrap(),
            ParseInputKind::DecodedHttp,
        )
        .unwrap();
        let failure = plan
            .parse(ParseInput::DecodedHttp(DecodedHttpText::new(input)))
            .expect_err("malformed input must not expose a ParsedDocument");
        assert_eq!(failure.kind, expected_kind);
        assert_eq!(failure.input_kind, ParseInputKind::DecodedHttp);
        assert!(failure.message.len() <= 512);
        assert!(!failure.message.contains(input));
    }
}

#[test]
fn malformed_and_truncated_html_never_exposes_the_repaired_tree() {
    let plan = compile_parse(
        &serde_json::from_value(serde_json::json!({ "type": "html" })).unwrap(),
        ParseInputKind::DecodedHttp,
    )
    .unwrap();

    for input in [
        "<main><article>bad</main>",
        "<main><article>",
        "<main></span></main>",
        "<p><div>misnested</div></p>",
    ] {
        let failure = plan
            .parse(ParseInput::DecodedHttp(DecodedHttpText::new(input)))
            .expect_err("html5ever repairs must not become a partial ParsedDocument");
        assert_eq!(failure.kind, ParseFailureKind::MalformedHtml);
        assert!(!failure.message.contains(input));
    }
}

#[test]
fn compilation_rejects_http_charset_on_browser_rendered_input() {
    let authored = serde_json::from_value(serde_json::json!({
        "type": "html",
        "charset": "utf-8"
    }))
    .unwrap();
    let error = compile_parse(&authored, ParseInputKind::BrowserRendered)
        .expect_err("Browser-rendered input must not be decoded as HTTP text");
    assert_eq!(
        error.message,
        "charset is valid only for strictly decoded HTTP parse input"
    );
}

#[test]
fn compiled_parse_plan_rejects_the_wrong_typed_input_kind() {
    let plan = compile_parse(
        &serde_json::from_value(serde_json::json!({ "type": "html" })).unwrap(),
        ParseInputKind::BrowserRendered,
    )
    .unwrap();

    let failure = plan
        .parse(ParseInput::DecodedHttp(DecodedHttpText::new(
            "<main></main>",
        )))
        .expect_err("input kind mismatch must fail before parser invocation");
    assert_eq!(failure.kind, ParseFailureKind::InputKindMismatch);
    assert_eq!(failure.input_kind, ParseInputKind::DecodedHttp);
}

fn parse_schema_keys() -> Vec<String> {
    enum_keys(
        include_str!("../../src/schema/profile-dsl/parse.schema.json"),
        &["$defs", "parse", "properties", "type", "enum"],
    )
}

fn parse_fragment_schema_keys() -> Vec<String> {
    enum_keys(
        include_str!("../../src/schema/profile-dsl/fragments.schema.json"),
        &["$defs", "parseFragment", "properties", "type", "enum"],
    )
}

fn enum_keys(schema: &str, path: &[&str]) -> Vec<String> {
    let schema: serde_json::Value = serde_json::from_str(schema).unwrap();
    let values = path.iter().fold(&schema, |value, segment| &value[*segment]);
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}
