use job_radar_lib::{SourceExecutionPlan, StrategyPolicy};
use serde_json::json;

#[test]
fn compiled_plan_uses_only_final_phase_names_and_retains_policy() {
    let plan: SourceExecutionPlan = serde_json::from_value(json!({
        "source": { "key": "source", "name": "Source" },
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "path",
            "name": "Path"
        },
        "sourceConfig": {},
        "discovery": {
            "policy": { "type": "first_accepted" },
            "strategies": []
        },
        "detail": {
            "policy": { "type": "first_accepted" },
            "strategies": []
        }
    }))
    .expect("a compiled plan must deserialize with final phase names");

    assert_eq!(plan.discovery.policy, StrategyPolicy::FirstAccepted);
    assert_eq!(
        plan.detail.as_ref().map(|step| step.policy),
        Some(StrategyPolicy::FirstAccepted)
    );

    let old_discovery_key = format!("posting{}", "Discovery");
    let old_detail_key = format!("posting{}", "Detail");
    let value = serde_json::to_value(&plan).unwrap();
    assert!(value.get("discovery").is_some());
    assert!(value.get("detail").is_some());
    assert!(value.get(&old_discovery_key).is_none());
    assert!(value.get(&old_detail_key).is_none());

    let mut old_names = json!({
        "source": { "key": "source", "name": "Source" },
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "path",
            "name": "Path"
        },
        "sourceConfig": {}
    });
    old_names[old_discovery_key] = json!({
        "policy": { "type": "first_accepted" },
        "strategies": []
    });
    assert!(serde_json::from_value::<SourceExecutionPlan>(old_names).is_err());
}
