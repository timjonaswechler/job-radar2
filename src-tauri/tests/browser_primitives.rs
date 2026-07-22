use job_radar_lib::{
    BrowserInteraction, BrowserWait, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
    Fetch,
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

fn assert_rejected<T>(value: Value)
where
    T: serde::de::DeserializeOwned,
{
    assert!(
        serde_json::from_value::<T>(value.clone()).is_err(),
        "unexpectedly admitted {value}"
    );
}
