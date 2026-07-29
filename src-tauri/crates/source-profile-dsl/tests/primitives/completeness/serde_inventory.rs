use serde_json::json;
use source_profile_dsl::profile_dsl::primitives::completeness::{
    production_schema_inventory, production_serde_inventory, AuthoredShapeKind, Family,
};
use source_profile_dsl::{
    Acceptance, BrowserInteraction, BrowserWait, CaptureRule, DetectionStrategy, DetectionUrlInput,
    Fetch, FieldExpression, InputUrlPattern, ListFieldExpression, Pagination, Parse, Predicate,
    RequestBody, Select, Transform,
};
use std::collections::BTreeSet;

fn assert_declared_options<T>(
    value: serde_json::Value,
    family: Family,
    parent: &str,
    discriminator: Option<&str>,
) where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let authored: T = serde_json::from_value(value).unwrap();
    let mut actual = serde_json::to_value(authored)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(discriminator) = discriminator {
        actual.remove(discriminator);
    }
    let prefix = format!("{parent}.");
    let declared = production_serde_inventory()
        .into_iter()
        .filter(|shape| shape.family == family)
        .filter_map(|shape| {
            shape
                .key
                .strip_prefix(&prefix)
                .map(|suffix| suffix.split('.').next().unwrap().to_owned())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, declared,
        "authored fields drifted from {family:?}.{parent}"
    );
}

fn assert_exact_object_shape<T>(value: serde_json::Value)
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let expected = value
        .as_object()
        .expect("structural fixture is an object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let authored: T = serde_json::from_value(value).unwrap();
    let actual = serde_json::to_value(authored)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn serde_inventory_is_independent_and_exact() {
    let schema = production_schema_inventory();
    let serde = production_serde_inventory();
    assert_eq!(schema.len(), serde.len());
    let schema_ids = schema
        .into_iter()
        .map(|v| (v.family, v.key, v.contexts, v.shape))
        .collect::<BTreeSet<_>>();
    let serde_ids = serde
        .iter()
        .map(|v| (v.family, v.key.clone(), v.contexts.clone(), v.shape))
        .collect::<BTreeSet<_>>();
    assert_eq!(schema_ids, serde_ids);
    assert!(serde
        .iter()
        .all(|v| v.authored_file.contains("profile_dsl")));
    assert_eq!(
        serde.iter().map(|v| v.shape).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            AuthoredShapeKind::Tagged,
            AuthoredShapeKind::Keyed,
            AuthoredShapeKind::Untagged,
            AuthoredShapeKind::ParentOption
        ])
    );
}

#[test]
fn exhaustive_authored_carriers_and_structural_fixtures_cover_all_shape_classes() {
    for kind in ["json", "xml", "html"] {
        assert_exact_object_shape::<Parse>(json!({"type":kind,"charset":"utf-8"}));
    }
    for body in [
        json!({"type":"json","value":{}}),
        json!({"type":"text","value":"x"}),
        json!({"type":"form","fields":{}}),
    ] {
        let parent = format!("http.body.{}", body["type"].as_str().unwrap());
        assert_exact_object_shape::<RequestBody>(body.clone());
        assert_declared_options::<RequestBody>(body, Family::Fetch, &parent, Some("type"));
    }
    assert_exact_object_shape::<Fetch>(
        json!({"mode":"http","method":"POST","url":"https://example.test","headers":{"accept":"x"},"body":{"type":"json","value":{}},"timeoutMs":1}),
    );
    assert_exact_object_shape::<Fetch>(
        json!({"mode":"browser","url":"https://example.test","timeoutMs":1,"waits":[],"interactions":[]}),
    );
    assert_declared_options::<Fetch>(
        json!({"mode":"http","method":"POST","url":"https://example.test","headers":{"accept":"x"},"body":{"type":"json","value":{}} ,"timeoutMs":1}),
        Family::Fetch,
        "http",
        Some("mode"),
    );
    assert_declared_options::<Fetch>(
        json!({"mode":"browser","url":"https://example.test","timeoutMs":1,"waits":[],"interactions":[]}),
        Family::Browser,
        "browser",
        Some("mode"),
    );
    for value in [
        json!({"type":"selector","selector":"main","timeoutMs":1}),
        json!({"type":"network_idle","timeoutMs":1}),
    ] {
        let parent = value["type"].as_str().unwrap().to_owned();
        assert_exact_object_shape::<BrowserWait>(value.clone());
        assert_declared_options::<BrowserWait>(value, Family::Browser, &parent, Some("type"));
    }
    for value in [
        json!({"type":"click_if_visible","selector":".x","maxCount":1,"waitAfterMs":0}),
        json!({"type":"click_until_gone","selector":".x","maxCount":1,"waitAfterMs":0}),
    ] {
        let parent = value["type"].as_str().unwrap().to_owned();
        assert_exact_object_shape::<BrowserInteraction>(value.clone());
        assert_declared_options::<BrowserInteraction>(
            value,
            Family::Browser,
            &parent,
            Some("type"),
        );
    }
    assert_exact_object_shape::<DetectionUrlInput>(
        json!({"type":"pattern_alternatives","alternatives":[{"pattern":"(?<tenant>.+)","captures":["tenant"]}]}),
    );
    assert_exact_object_shape::<DetectionUrlInput>(json!({"type":"absolute_url"}));
    assert_exact_object_shape::<InputUrlPattern>(json!({"pattern":"x","captures":["tenant"]}));
    assert_exact_object_shape::<DetectionStrategy>(
        json!({"type":"url","key":"url","input":{"type":"absolute_url"}}),
    );
    assert_exact_object_shape::<DetectionStrategy>(
        json!({"type":"http","key":"probe","fetch":{"mode":"http","url":"https://example.test","timeoutMs":1},"expectStatus":200,"contains":"ok","regex":"(?<tenant>ok)","captures":["tenant"],"evidence":"ok"}),
    );
    assert_exact_object_shape::<DetectionStrategy>(
        json!({"type":"browser","key":"browser","fetch":{"mode":"browser","url":"https://example.test","timeoutMs":1,"waits":[],"interactions":[]},"contains":"ok","regex":"(?<tenant>ok)","captures":["tenant"],"evidence":"ok"}),
    );
    assert_declared_options::<DetectionStrategy>(
        json!({"type":"url","key":"url","input":{"type":"absolute_url"}}),
        Family::Detection,
        "url",
        Some("type"),
    );
    assert_declared_options::<DetectionStrategy>(
        json!({"type":"http","key":"probe","fetch":{"mode":"http","url":"https://example.test","timeoutMs":1},"expectStatus":200,"contains":"ok","regex":"(?<tenant>ok)","captures":["tenant"],"evidence":"ok"}),
        Family::Detection,
        "http",
        Some("type"),
    );
    assert_declared_options::<DetectionStrategy>(
        json!({"type":"browser","key":"browser","fetch":{"mode":"browser","url":"https://example.test","timeoutMs":1,"waits":[],"interactions":[]},"contains":"ok","regex":"(?<tenant>ok)","captures":["tenant"],"evidence":"ok"}),
        Family::Detection,
        "browser",
        Some("type"),
    );
    assert_declared_options::<DetectionUrlInput>(
        json!({"type":"pattern_alternatives","alternatives":[{"pattern":"x"}]}),
        Family::Detection,
        "pattern_alternatives",
        Some("type"),
    );
    let acceptance: Acceptance = serde_json::from_value(json!({
        "requiredFields": ["url"], "minDescriptionLength": 1, "minResults": 1
    }))
    .unwrap();
    let acceptance_keys = serde_json::to_value(acceptance)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        acceptance_keys,
        production_serde_inventory()
            .into_iter()
            .filter(|shape| shape.family == Family::Acceptance)
            .map(|shape| shape.key)
            .collect()
    );
    let capture: CaptureRule = serde_json::from_value(json!({
        "from": { "type": "const", "value": "x" }, "pattern": "(?<key>x)"
    }))
    .unwrap();
    let capture_keys = serde_json::to_value(capture)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let declared_capture = production_serde_inventory()
        .into_iter()
        .filter(|shape| shape.family == Family::Capture)
        .filter_map(|shape| shape.key.strip_prefix("entry.").map(str::to_owned))
        .collect();
    assert_eq!(capture_keys, declared_capture);
    let combine: FieldExpression = serde_json::from_value(json!({
        "type": "combine", "parts": [{ "value": { "type": "const", "value": "x" }, "optional": true }], "join": " "
    })).unwrap();
    let combine = serde_json::to_value(combine).unwrap();
    assert_eq!(
        combine["parts"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["value".into(), "optional".into()])
    );
    let expression_variants = [
        json!({"type":"const","value":"x"}),
        json!({"type":"template","template":"x"}),
        json!({"type":"source_config","key":"x"}),
        json!({"type":"posting_meta","key":"x"}),
        json!({"type":"capture","key":"x"}),
        json!({"type":"item_field","key":"x"}),
        json!({"type":"json_path","jsonPath":"$.x"}),
        json!({"type":"xml_text","textPath":"x"}),
        json!({"type":"xml_element","element":"x"}),
        json!({"type":"css_text","selector":".x"}),
        json!({"type":"css_attribute","selector":".x","attribute":"href"}),
        json!({"type":"combine","parts":[{"value":{"type":"const","value":"x"},"optional":true}],"join":" "}),
        json!({"type":"first_non_empty","candidates":[{"type":"const","value":"x"}]}),
    ];
    for expression in &expression_variants {
        assert_exact_object_shape::<FieldExpression>(expression.clone());
    }
    assert!(matches!(
        serde_json::from_value::<ListFieldExpression>(expression_variants[0].clone()).unwrap(),
        ListFieldExpression::Single(_)
    ));
    assert!(matches!(
        serde_json::from_value::<ListFieldExpression>(json!([expression_variants[0].clone()]))
            .unwrap(),
        ListFieldExpression::Multiple(_)
    ));

    for value in [
        json!({"type":"document"}),
        json!({"type":"json_path","jsonPath":"$.x"}),
        json!({"type":"xml_element","element":"x"}),
        json!({"type":"xml_text","textPath":""}),
        json!({"type":"css","selector":".x"}),
        json!({"type":"sitemap_urls","urlPattern":"jobs"}),
    ] {
        let parent = value["type"].as_str().unwrap().to_owned();
        assert_exact_object_shape::<Select>(value.clone());
        assert_declared_options::<Select>(value, Family::Select, &parent, Some("type"));
    }
    for value in [
        json!({"type":"trim"}),
        json!({"type":"normalize_whitespace"}),
        json!({"type":"html_to_text"}),
        json!({"type":"url_decode"}),
        json!({"type":"slug_to_title"}),
        json!({"type":"dedupe"}),
        json!({"type":"to_string"}),
        json!({"type":"split","separator":",","trimParts":true,"dropEmpty":true}),
        json!({"type":"join","separator":""}),
        json!({"type":"regex_replace","pattern":"x","replacement":""}),
    ] {
        let parent = value["type"].as_str().unwrap().to_owned();
        assert_exact_object_shape::<Transform>(value.clone());
        assert_declared_options::<Transform>(value, Family::Transform, &parent, Some("type"));
    }
    let field = json!({"type":"const","value":"x"});
    for value in [
        json!({"type":"non_empty","field":field.clone()}),
        json!({"type":"regex","field":field.clone(),"pattern":"x"}),
        json!({"type":"equal","left":field.clone(),"right":field.clone()}),
    ] {
        let parent = match value["type"].as_str().unwrap() {
            "equal" => "detail.match".to_owned(),
            other => other.to_owned(),
        };
        assert_exact_object_shape::<Predicate>(value.clone());
        assert_declared_options::<Predicate>(value, Family::Predicate, &parent, Some("type"));
    }
    for value in [
        json!({"type":"page","pageParam":"page","parameterLocation":"json_body","firstPage":0,"pageSizeParam":"size","pageSize":10,"totalPath":"$.total","limits":{"maxRequests":1,"maxItems":1,"maxDepth":0}}),
        json!({"type":"offset_limit","offsetParam":"offset","limitParam":"limit","parameterLocation":"json_body","startOffset":0,"limit":10,"totalPath":"$.total","limits":{"maxRequests":1,"maxItems":1,"maxDepth":0}}),
        json!({"type":"cursor","cursorParam":"cursor","parameterLocation":"json_body","nextCursorPath":"$.next","limits":{"maxRequests":1,"maxItems":1,"maxDepth":0}}),
        json!({"type":"sitemap","childSitemapSelector":{"type":"xml_text","textPath":"loc"},"postingUrlSelector":{"type":"xml_text","textPath":"loc"},"limits":{"maxRequests":1,"maxItems":1,"maxDepth":0}}),
    ] {
        let parent = value["type"].as_str().unwrap().to_owned();
        assert_exact_object_shape::<Pagination>(value.clone());
        assert_declared_options::<Pagination>(value, Family::Pagination, &parent, Some("type"));
    }
    let keys = production_serde_inventory()
        .into_iter()
        .map(|v| (v.family, v.key))
        .collect::<BTreeSet<_>>();
    for key in [
        (Family::Value, "list.single".into()),
        (Family::Value, "list.multiple".into()),
        (Family::Value, "combine.part.value".into()),
        (Family::Value, "combine.part.optional".into()),
        (Family::Capture, "entry.from".into()),
        (Family::Capture, "entry.pattern".into()),
        (Family::Predicate, "detail.match.left".into()),
        (Family::Acceptance, "requiredFields".into()),
        (Family::Browser, "selector.selector".into()),
        (Family::Detection, "input_url_pattern.captures".into()),
    ] {
        assert!(
            keys.contains(&key),
            "missing structural Serde shape {key:?}"
        );
    }
}

#[test]
fn removed_and_illegal_shapes_remain_rejected() {
    assert!(serde_json::from_value::<Parse>(json!({"type":"text"})).is_err());

    for operation in [
        "execute_script",
        "eval",
        "mutate_dom",
        "login_flow",
        "captcha_bypass",
    ] {
        assert!(
            serde_json::from_value::<BrowserInteraction>(json!({"type": operation})).is_err(),
            "removed Browser operation {operation} must reject"
        );
    }
    for alias in [
        "normalizeWhitespace",
        "htmlToText",
        "urlDecode",
        "slugToTitle",
        "toString",
    ] {
        assert!(
            serde_json::from_value::<Transform>(json!({"type": alias})).is_err(),
            "removed Transform alias {alias} must reject"
        );
    }
    for composition in ["all", "any", "none", "not", "negation", "count"] {
        assert!(
            serde_json::from_value::<Predicate>(json!({"type": composition})).is_err(),
            "unlanded Predicate composition {composition} must reject"
        );
    }
    assert!(serde_json::from_value::<Acceptance>(json!({"maxErrorRatio": 0.5})).is_err());
    assert!(serde_json::from_value::<Fetch>(json!({
        "mode": "http",
        "url": "https://example.test",
        "timeoutMs": 1,
        "retry": {"maxAttempts": 2}
    }))
    .is_err());

    assert!(serde_json::from_value::<DetectionUrlInput>(
        json!({"type":"pattern_alternatives","alternatives":[]})
    )
    .is_err());
    assert!(serde_json::from_value::<DetectionStrategy>(json!({"type":"http","key":"probe","fetch":{"mode":"http","url":"https://example.test","timeoutMs":1},"expectStatus":99})).is_err());
}
