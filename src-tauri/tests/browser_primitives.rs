use std::collections::BTreeSet;

use job_radar_lib::{
    browser_primitive_descriptors, compile_template, validate_browser_primitive_descriptors,
    BrowserInteraction, BrowserPrimitiveDescriptor, BrowserWait, ExecutionPlanBrowserInteraction,
    ExecutionPlanBrowserWait, ExecutionPlanFetch, Fetch, TemplateDescriptor,
    BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR, BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR,
    BROWSER_FETCH_DESCRIPTOR, BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR,
    BROWSER_SELECTOR_WAIT_DESCRIPTOR,
};
use serde_json::{json, Value};

#[test]
fn authored_browser_family_admits_exactly_the_canonical_variants() {
    let fetch = json!({
        "mode": "browser",
        "url": "{{sourceConfig:startUrl}}",
        "timeoutMs": 120000,
        "waits": [
            { "type": "selector", "selector": "main", "timeoutMs": 60000 },
            { "type": "network_idle", "timeoutMs": 60000 }
        ],
        "interactions": [
            { "type": "click_if_visible", "selector": ".optional", "maxCount": 1 },
            { "type": "click_until_gone", "selector": ".more", "maxCount": 50, "waitAfterMs": 60000 }
        ]
    });

    let parsed: Fetch = serde_json::from_value(fetch.clone()).expect("canonical Browser Fetch");
    assert_eq!(serde_json::to_value(parsed).unwrap(), fetch);

    for prohibited in [
        "execute_script",
        "eval",
        "mutate_dom",
        "login_flow",
        "captcha_bypass",
        "retry",
    ] {
        assert_rejected::<BrowserInteraction>(json!({ "type": prohibited }));
    }
}

#[test]
fn authored_browser_family_rejects_missing_foreign_and_out_of_bound_fields() {
    for invalid_fetch in [
        json!({ "mode": "browser", "url": "https://example.test" }),
        json!({ "mode": "browser", "url": "https://example.test", "timeoutMs": 0 }),
        json!({ "mode": "browser", "url": "https://example.test", "timeoutMs": 120001 }),
        json!({ "mode": "browser", "url": "https://example.test", "timeoutMs": 1, "maxBrowserRenderedBytes": 1 }),
        json!({ "mode": "browser", "url": "https://example.test", "timeoutMs": 1, "body": { "type": "text", "value": "x" } }),
    ] {
        assert_rejected::<Fetch>(invalid_fetch);
    }

    for invalid_wait in [
        json!({ "type": "selector", "selector": "main" }),
        json!({ "type": "selector", "timeoutMs": 1 }),
        json!({ "type": "selector", "selector": " ", "timeoutMs": 1 }),
        json!({ "type": "selector", "selector": "main", "timeoutMs": 0 }),
        json!({ "type": "selector", "selector": "main", "timeoutMs": 60001 }),
        json!({ "type": "network_idle" }),
        json!({ "type": "network_idle", "selector": "main", "timeoutMs": 1 }),
        json!({ "type": "network_idle", "timeoutMs": 60001 }),
    ] {
        assert_rejected::<BrowserWait>(invalid_wait);
    }

    for interaction_type in ["click_if_visible", "click_until_gone"] {
        for invalid_interaction in [
            json!({ "type": interaction_type, "selector": ".more" }),
            json!({ "type": interaction_type, "maxCount": 1 }),
            json!({ "type": interaction_type, "selector": "", "maxCount": 1 }),
            json!({ "type": interaction_type, "selector": ".more", "maxCount": 0 }),
            json!({ "type": interaction_type, "selector": ".more", "maxCount": 51 }),
            json!({ "type": interaction_type, "selector": ".more", "maxCount": 1, "waitAfterMs": 60001 }),
            json!({ "type": interaction_type, "selector": ".more", "maxCount": 1, "timeoutMs": 1 }),
        ] {
            assert_rejected::<BrowserInteraction>(invalid_interaction);
        }
    }
}

#[test]
fn compiled_browser_primitives_are_closed_and_disjoint() {
    let selector: ExecutionPlanBrowserWait = serde_json::from_value(json!({
        "type": "selector", "selector": "main", "timeoutMs": 1
    }))
    .unwrap();
    assert!(matches!(
        selector,
        ExecutionPlanBrowserWait::Selector { .. }
    ));

    let idle: ExecutionPlanBrowserWait = serde_json::from_value(json!({
        "type": "network_idle", "timeoutMs": 1
    }))
    .unwrap();
    assert!(matches!(idle, ExecutionPlanBrowserWait::NetworkIdle { .. }));

    assert_rejected::<ExecutionPlanBrowserWait>(
        json!({ "type": "network_idle", "selector": "main", "timeoutMs": 1 }),
    );
    assert_rejected::<ExecutionPlanBrowserWait>(
        json!({ "type": "selector", "selector": "main", "timeoutMs": 1, "foreign": true }),
    );
    assert_rejected::<ExecutionPlanBrowserWait>(
        json!({ "type": "selector", "selector": "", "timeoutMs": 1 }),
    );
    assert_rejected::<ExecutionPlanBrowserWait>(
        json!({ "type": "network_idle", "timeoutMs": 60001 }),
    );
    assert_rejected::<ExecutionPlanBrowserInteraction>(
        json!({ "type": "click_if_visible", "selector": ".more", "maxCount": 1, "timeoutMs": 1 }),
    );
    assert_rejected::<ExecutionPlanBrowserInteraction>(
        json!({ "type": "click_until_gone", "selector": ".more", "maxCount": 51 }),
    );
}

#[test]
fn browser_descriptor_catalogue_is_exact_exhaustive_and_rejects_synthetic_faults() {
    let descriptors = browser_primitive_descriptors();
    assert_eq!(
        descriptors,
        &[
            BROWSER_FETCH_DESCRIPTOR,
            BROWSER_SELECTOR_WAIT_DESCRIPTOR,
            BROWSER_NETWORK_IDLE_WAIT_DESCRIPTOR,
            BROWSER_CLICK_IF_VISIBLE_DESCRIPTOR,
            BROWSER_CLICK_UNTIL_GONE_DESCRIPTOR,
        ]
    );
    validate_browser_primitive_descriptors(descriptors).unwrap();
    assert!(descriptors
        .iter()
        .all(|descriptor| descriptor.owner == "B03a"
            && descriptor
                .canonical_file
                .ends_with("profile_dsl/primitives/fetch/browser.rs")));

    let authored_waits = [
        BrowserWait::Selector {
            selector: "main".into(),
            timeout_ms: 1,
        },
        BrowserWait::NetworkIdle { timeout_ms: 1 },
    ];
    let compiled_waits = [
        ExecutionPlanBrowserWait::Selector {
            selector: "main".into(),
            timeout_ms: 1,
        },
        ExecutionPlanBrowserWait::NetworkIdle { timeout_ms: 1 },
    ];
    assert_eq!(
        authored_waits
            .iter()
            .map(|value| value.descriptor().key)
            .collect::<Vec<_>>(),
        compiled_waits
            .iter()
            .map(|value| value.descriptor().key)
            .collect::<Vec<_>>()
    );
    for wait in &authored_waits {
        assert_descriptor_shape(wait, wait.descriptor(), "type");
    }

    let authored_interactions = [
        BrowserInteraction::ClickIfVisible {
            selector: ".x".into(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
        BrowserInteraction::ClickUntilGone {
            selector: ".x".into(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
    ];
    let compiled_interactions = [
        ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: ".x".into(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
        ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector: ".x".into(),
            max_count: 1,
            wait_after_ms: Some(0),
        },
    ];
    assert_eq!(
        authored_interactions
            .iter()
            .map(|value| value.descriptor().key)
            .collect::<Vec<_>>(),
        compiled_interactions
            .iter()
            .map(|value| value.descriptor().key)
            .collect::<Vec<_>>()
    );
    for interaction in &authored_interactions {
        assert_descriptor_shape(interaction, interaction.descriptor(), "type");
    }
    assert!(descriptors
        .iter()
        .filter(|descriptor| descriptor
            .options
            .iter()
            .any(|option| option.key == "selector"))
        .all(|descriptor| descriptor
            .options
            .iter()
            .find(|option| option.key == "selector")
            .is_some_and(|option| option.non_empty)));

    let authored: Fetch = serde_json::from_value(json!({
        "mode": "browser",
        "url": "https://example.test",
        "timeoutMs": 1,
        "waits": [],
        "interactions": []
    }))
    .unwrap();
    let compiled = ExecutionPlanFetch::Browser {
        url: compile_template("https://example.test", &TemplateDescriptor::new()).unwrap(),
        timeout_ms: 1,
        waits: vec![],
        interactions: vec![],
    };
    assert_eq!(authored.browser_descriptor(), compiled.browser_descriptor());
    assert_descriptor_shape(&authored, authored.browser_descriptor().unwrap(), "mode");

    assert!(validate_browser_primitive_descriptors(&descriptors[..4]).is_err());
    let mut duplicate = descriptors.to_vec();
    duplicate.push(descriptors[0]);
    assert!(validate_browser_primitive_descriptors(&duplicate).is_err());
    let mut conflict = descriptors.to_vec();
    conflict[0].owner = "wrong";
    assert!(validate_browser_primitive_descriptors(&conflict).is_err());
}

fn assert_descriptor_shape<T>(
    value: &T,
    descriptor: &BrowserPrimitiveDescriptor,
    discriminator: &str,
) where
    T: serde::Serialize,
{
    let mut object = serde_json::to_value(value)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    object.remove(discriminator);
    let actual = object.keys().cloned().collect::<BTreeSet<_>>();
    let expected = descriptor
        .options
        .iter()
        .map(|option| option.key.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "descriptor {} option drift",
        descriptor.key
    );
}

fn assert_rejected<T>(value: Value)
where
    T: serde::de::DeserializeOwned,
{
    assert!(
        serde_json::from_value::<T>(value.clone()).is_err(),
        "unexpectedly admitted {value}"
    );
}
