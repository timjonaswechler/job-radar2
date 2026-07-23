use std::{collections::BTreeMap, fs, path::Path};

use job_radar_lib::{
    compile_source, compile_template, render_template, CompileSourceOutcome, CompiledValue,
    DiagnosticCategory, RegistrySourceProfile, SourceDocument, SourceProfileDocument,
    SourceProfileRegistrySnapshot, TemplateCompileErrorKind, TemplateDescriptor, TemplateReference,
    TemplateValueView,
};
use serde_json::json;

struct Values(BTreeMap<String, String>);
impl TemplateValueView for Values {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        let key = reference
            .namespace
            .as_ref()
            .map(|namespace| format!("{namespace}:{}", reference.key))
            .unwrap_or_else(|| reference.key.clone());
        self.0.get(&key).cloned()
    }
}

fn fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn compile(profile: SourceProfileDocument) -> CompileSourceOutcome {
    let source: SourceDocument =
        fixture("tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json");
    compile_source(
        &source,
        &SourceProfileRegistrySnapshot {
            profiles: vec![RegistrySourceProfile {
                origin: "template-test".into(),
                path: String::new(),
                document: profile,
            }],
            sources: Vec::new(),
            diagnostics: Vec::new(),
        },
    )
}

fn profile_value() -> serde_json::Value {
    fixture("tests/fixtures/source-profile-dsl/valid/simple-source-profile.json")
}

#[test]
fn canonical_template_compiles_literals_references_and_doubled_delimiter_escapes() {
    let descriptor = TemplateDescriptor::new().allow_namespace("sourceConfig", ["tenant"]);
    let template = compile_template(
        "literal {{{{sourceConfig:tenant}}}} / {{sourceConfig:tenant}} } tail",
        &descriptor,
    )
    .unwrap();
    let values = Values(BTreeMap::from([(
        "sourceConfig:tenant".to_string(),
        "acme".to_string(),
    )]));

    assert_eq!(
        render_template(&template, &values).unwrap(),
        "literal {{sourceConfig:tenant}} / acme } tail"
    );
    let serialized = serde_json::to_string(&template).unwrap();
    assert!(serialized.contains("sourceConfig"));
    assert!(!serialized.contains("acme"));
}

#[test]
fn canonical_template_rejects_malformed_and_unavailable_references_with_real_offsets() {
    let descriptor = TemplateDescriptor::new()
        .allow_bare("inputUrl")
        .allow_namespace("sourceConfig", ["tenant"]);
    let cases = [
        ("{{", TemplateCompileErrorKind::UnmatchedOpeningDelimiter),
        ("}}", TemplateCompileErrorKind::UnmatchedClosingDelimiter),
        ("{{ }}", TemplateCompileErrorKind::EmptyReference),
        (
            "{{sourceConfig.tenant}}",
            TemplateCompileErrorKind::InvalidReference,
        ),
        (
            "{{sourceConfig:tenant|trim}}",
            TemplateCompileErrorKind::TransformPipeUnsupported,
        ),
        (
            "{{posting:url}}",
            TemplateCompileErrorKind::UnknownNamespace,
        ),
        (
            "{{sourceConfig:missing}}",
            TemplateCompileErrorKind::UnknownKey,
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(
            compile_template(source, &descriptor).unwrap_err().kind,
            expected,
            "{source}"
        );
    }
    let later = compile_template(
        "ok {{sourceConfig:tenant}} then {{sourceConfig:missing}}",
        &descriptor,
    )
    .unwrap_err();
    assert_eq!(later.offset, 32);
    assert!(compile_template("{{inputUrl}}", &descriptor).is_ok());
}

#[test]
fn compile_source_compiles_value_http_browser_and_detection_templates_into_typed_plans() {
    let mut value = profile_value();
    let strategy = &mut value["accessPaths"][0]["discovery"]["strategies"][0];
    strategy["fetch"]["url"] = json!("{{sourceConfig:feedUrl}}?literal={{{{x}}}}");
    strategy["fetch"]["method"] = json!("POST");
    strategy["fetch"]["headers"] = json!({ "x-requested-with": "{{source:name}}" });
    strategy["fetch"]["body"] = json!({
        "type": "json",
        "value": { "outer": { "nested": "{{sourceConfig:feedUrl}}" } }
    });
    strategy["extract"]["providerValues"]["title"] = json!({
        "type": "template", "template": "{{source:name}} {{{{role}}}}"
    });
    let mut browser = strategy.clone();
    browser["key"] = json!("browser_fallback");
    browser["fetch"] = json!({
        "mode": "browser", "url": "{{sourceConfig:feedUrl}}/browser", "timeoutMs": 10000
    });
    value["accessPaths"][0]["discovery"]["strategies"]
        .as_array_mut()
        .unwrap()
        .push(browser);
    value["detection"] = json!({
        "policy": { "type": "all_required" },
        "strategies": [
            { "type": "url", "key": "input_url", "input": { "type": "pattern_alternatives", "alternatives": [{ "pattern": "^https://(?<tenant>[^/]+)", "captures": ["tenant"] }] } },
            { "type": "http", "key": "http", "fetch": { "mode": "http", "method": "GET", "url": "{{inputUrl}}", "timeoutMs": 1000 }, "expectStatus": 200 },
            { "type": "browser", "key": "browser", "fetch": { "mode": "browser", "url": "https://{{capture:tenant}}/browser", "timeoutMs": 3000 }, "contains": "jobs" }
        ],
        "sourceConfig": { "feedUrl": "{{inputUrl}}" },
        "keyCandidates": ["{{capture:tenant}}"],
        "nameCandidates": ["Detected {{capture:tenant}}"]
    });

    let outcome = compile(serde_json::from_value(value).unwrap());
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = outcome
    else {
        panic!("representative templates must compile: {outcome:?}");
    };
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.category != DiagnosticCategory::Compiler));
    let serialized = serde_json::to_string(&source.execution_plan).unwrap();
    assert!(serialized.contains("reference"));
    assert!(serialized.contains("sourceConfig"));
    assert!(!serialized.contains("{{sourceConfig"));
    let CompiledValue::Template { template, .. } = source.execution_plan.discovery.strategies[0]
        .extract
        .output
        .provider_values
        .as_ref()
        .and_then(|values| values.title.as_ref())
        .expect("compiled title")
    else {
        panic!("Value template must be compiled")
    };
    assert_eq!(
        render_template(
            template,
            &Values(BTreeMap::from([(
                "source:name".into(),
                "Example Source".into()
            )]))
        )
        .unwrap(),
        "Example Source {{role}}"
    );
}

#[test]
fn compile_source_rejects_detail_fetch_capture_before_io_and_detection_malformed_template() {
    let mut value = profile_value();
    value["accessPaths"][0]["detail"]["strategies"][0]["captures"] = json!({
        "tenant": { "from": { "type": "posting_meta", "key": "jobId" }, "pattern": "^(?<tenant>.+)$" }
    });
    value["accessPaths"][0]["detail"]["strategies"][0]["fetch"]["url"] =
        json!("{{captures:tenant}}");
    value["detection"] = json!({
        "policy": { "type": "all_required" },
        "strategies": [{ "type": "url", "key": "input_url", "input": { "type": "pattern_alternatives", "alternatives": [{ "pattern": "^https://example\\.test" }] } }],
        "keyCandidates": ["prefix {{"]
    });

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile(serde_json::from_value(value.clone()).unwrap())
    else {
        panic!("invalid pre-I/O templates must reject compilation")
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "template_namespace_unavailable"
            && diagnostic.path == "/accessPaths/0/detail/strategies/0/fetch/url"
            && diagnostic.strategy_key.as_deref() == Some("detail_api")
    }));
    let profile: SourceProfileDocument = serde_json::from_value(value).unwrap();
    let detection_diagnostics = job_radar_lib::compile_detection_plan(&profile).unwrap_err();
    assert!(detection_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "invalid_detection_proposal_template"
            && diagnostic.path == "/detection/keyCandidates/0"
    }));
}

#[test]
fn authored_map_keys_are_json_pointer_escaped_in_template_diagnostic_paths() {
    let mut value = profile_value();
    let strategy = &mut value["accessPaths"][0]["discovery"]["strategies"][0];
    strategy["fetch"]["method"] = json!("POST");
    strategy["fetch"]["headers"] = json!({ "accept": "application/json" });
    strategy["fetch"]["body"] = json!({ "type": "json", "value": { "j~/x": "{{unknown:x}}" } });
    strategy["captures"] = json!({
        "c~/x": { "from": { "type": "template", "template": "{{unknown:x}}" }, "pattern": "^(?<value>.+)$" }
    });
    strategy["extract"]["postingMeta"] = json!({
        "p~/x": { "type": "template", "template": "{{unknown:x}}" }
    });
    let mut form_strategy = strategy.clone();
    form_strategy["key"] = json!("form_fallback");
    form_strategy["fetch"]["method"] = json!("POST");
    form_strategy["fetch"]
        .as_object_mut()
        .unwrap()
        .remove("headers");
    form_strategy["fetch"]["body"] = json!({
        "type": "form", "fields": { "f~/x": "{{unknown:x}}" }
    });
    value["accessPaths"][0]["discovery"]["strategies"]
        .as_array_mut()
        .unwrap()
        .push(form_strategy);

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile(serde_json::from_value(value).unwrap())
    else {
        panic!("unknown namespaces must reject")
    };
    for path in [
        "/accessPaths/0/discovery/strategies/0/fetch/body/value/j~0~1x",
        "/accessPaths/0/discovery/strategies/0/captures/c~0~1x/from/template",
        "/accessPaths/0/discovery/strategies/0/extract/postingMeta/p~0~1x/template",
        "/accessPaths/0/discovery/strategies/1/fetch/body/fields/f~0~1x",
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.path == path),
            "missing escaped diagnostic path {path}: {diagnostics:?}"
        );
    }
}
