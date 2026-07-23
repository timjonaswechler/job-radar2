use crate::support::{
    accepted_phase, budget_exhausted, cancelled, execution_failed, not_started, policy_unsatisfied,
};
use std::{collections::BTreeMap, future::Future};

fn empty_source_config() -> &'static serde_json::Map<String, serde_json::Value> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, serde_json::Value>> =
        std::sync::OnceLock::new();
    EMPTY.get_or_init(serde_json::Map::new)
}
use job_radar_lib::{
    PhaseBrowser,
    __test_execute_detail_phase as execute_detail, compile_source, execute_discovery,
    AllowanceDimension, CompileSourceOutcome, DetailBrowserAdapter, DiscoveryStep, ScriptedBrowserAcquisition, ExecutionPlanFetch, PhaseCompletion,
    PhaseLimits, PhaseOutcome, PolicyOutcome, PolicyUnsatisfiedCause, PostingOccurrence,
    ProfileHttpFailureKind, RegistrySourceProfile, RequestedDetailFields, RuntimeCancellation,
    RuntimeExecutionContext, ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
    SourceDocument, SourceExecutionPlan, SourceProfileDocument, SourceProfileRegistrySnapshot,
    StrategyPolicy,
};
use serde_json::{json, Value};

#[test]
fn final_strategy_set_requires_an_exact_closed_policy_object() {
    let strategy_set = json!({ "strategies": [] });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("a final Strategy Set without Policy must be rejected");

    let strategy_set = json!({ "policy": "first_accepted", "strategies": [] });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("a raw string Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "unknown" },
        "strategies": []
    });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("an unknown Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "first_accepted", "extra": true },
        "strategies": []
    });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("additional Policy properties must be rejected");

    for invalid_policy in [
        Value::Null,
        json!({ "type": "allRequired" }),
        json!({ "type": "all_required", "count": 1 }),
        json!({ "type": "all_required", "threshold": 1 }),
        json!({ "type": "all_required", "mode": "strict" }),
    ] {
        serde_json::from_value::<DiscoveryStep>(json!({
            "policy": invalid_policy,
            "strategies": []
        }))
        .expect_err("an inexact all_required Policy must be rejected");
    }

    for (policy_type, expected) in [
        ("first_accepted", StrategyPolicy::FirstAccepted),
        ("all_required", StrategyPolicy::AllRequired),
    ] {
        let strategy_set: DiscoveryStep = serde_json::from_value(json!({
            "policy": { "type": policy_type },
            "strategies": []
        }))
        .unwrap();
        assert_eq!(strategy_set.policy, expected);
        assert_eq!(
            serde_json::to_value(strategy_set.policy).unwrap(),
            json!({ "type": policy_type })
        );
    }

    let at_least: DiscoveryStep = serde_json::from_value(json!({
        "policy": { "type": "at_least", "count": 2 },
        "strategies": []
    }))
    .unwrap();
    assert_eq!(at_least.policy, StrategyPolicy::AtLeast { count: 2 });
    assert_eq!(
        serde_json::to_value(at_least.policy).unwrap(),
        json!({ "type": "at_least", "count": 2 })
    );

    for invalid_policy in [
        json!({ "type": "at_least" }),
        json!({ "type": "at_least", "count": 0 }),
        json!({ "type": "at_least", "count": -1 }),
        json!({ "type": "at_least", "count": 1.5 }),
        json!({ "type": "at_least", "count": "1" }),
        json!({ "type": "at_least", "count": null }),
        json!({ "type": "at_least", "count": 1, "extra": true }),
        json!({ "type": "atLeast", "count": 1 }),
    ] {
        serde_json::from_value::<DiscoveryStep>(json!({
            "policy": invalid_policy,
            "strategies": []
        }))
        .expect_err("an inexact at_least Policy must be rejected");
    }

    let collect_all: DiscoveryStep = serde_json::from_value(json!({
        "policy": { "type": "collect_all", "minAccepted": 2 },
        "strategies": []
    }))
    .unwrap();
    assert_eq!(
        collect_all.policy,
        StrategyPolicy::CollectAll { min_accepted: 2 }
    );
    assert_eq!(
        serde_json::to_value(collect_all.policy).unwrap(),
        json!({ "type": "collect_all", "minAccepted": 2 })
    );

    for invalid_policy in [
        json!({ "type": "collect_all" }),
        json!({ "type": "collect_all", "minAccepted": 0 }),
        json!({ "type": "collect_all", "minAccepted": -1 }),
        json!({ "type": "collect_all", "minAccepted": 1.5 }),
        json!({ "type": "collect_all", "minAccepted": "1" }),
        json!({ "type": "collect_all", "minAccepted": null }),
        json!({ "type": "collect_all", "minAccepted": 1, "extra": true }),
        json!({ "type": "collectAll", "minAccepted": 1 }),
        json!({ "type": "collect_all", "min_accepted": 1 }),
    ] {
        serde_json::from_value::<DiscoveryStep>(json!({
            "policy": invalid_policy,
            "strategies": []
        }))
        .expect_err("an inexact collect_all Policy must be rejected");
    }
}

#[test]
fn final_compiler_preserves_policy_for_inherited_specialized_added_and_source_owned_sets() {
    let profile = profile_document();

    let inherited = compile(profile_source(None, "main"), profile.clone());
    assert_plan_policies(&inherited, StrategyPolicy::FirstAccepted);

    let reusable_all_required = compile(
        profile_source(None, "main"),
        all_required_profile_document(),
    );
    assert_plan_policies(&reusable_all_required, StrategyPolicy::AllRequired);

    let mut reusable_at_least = serde_json::to_value(profile_document()).unwrap();
    reusable_at_least["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "at_least", "count": 2 });
    reusable_at_least["accessPaths"][0]["detail"]["policy"] =
        json!({ "type": "at_least", "count": 2 });
    let reusable_at_least = compile(
        profile_source(None, "main"),
        serde_json::from_value(reusable_at_least).unwrap(),
    );
    assert_plan_policies(&reusable_at_least, StrategyPolicy::AtLeast { count: 2 });

    let mut reusable_collect_all = serde_json::to_value(profile_document()).unwrap();
    reusable_collect_all["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": 2 });
    reusable_collect_all["accessPaths"][0]["detail"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": 2 });
    let reusable_collect_all = compile(
        profile_source(None, "main"),
        serde_json::from_value(reusable_collect_all).unwrap(),
    );
    assert_plan_policies(
        &reusable_collect_all,
        StrategyPolicy::CollectAll { min_accepted: 2 },
    );

    let specialized = compile(
        profile_source(
            Some(json!([{
                "key": "main",
                "discovery": {
                    "policy": { "type": "all_required" },
                    "strategies": [discovery_strategy(
                        "source_added",
                        "https://example.test/discovery/source-added"
                    )]
                },
                "detail": { "policy": { "type": "all_required" } }
            }])),
            "main",
        ),
        profile.clone(),
    );
    assert_plan_policies(&specialized, StrategyPolicy::AllRequired);
    assert_eq!(
        specialized.discovery.strategies.len(),
        4,
        "a complete Source-added Strategy must retain the inherited Policy"
    );

    let specialized_collect_all = compile(
        profile_source(
            Some(json!([{
                "key": "main",
                "discovery": {
                    "policy": { "type": "collect_all", "minAccepted": 4 },
                    "strategies": [discovery_strategy(
                        "source_added",
                        "https://example.test/discovery/source-added"
                    )]
                },
                "detail": {
                    "policy": { "type": "collect_all", "minAccepted": 3 }
                }
            }])),
            "main",
        ),
        profile.clone(),
    );
    assert_eq!(
        specialized_collect_all.discovery.policy,
        StrategyPolicy::CollectAll { min_accepted: 4 }
    );
    assert_eq!(
        specialized_collect_all.detail.as_ref().unwrap().policy,
        StrategyPolicy::CollectAll { min_accepted: 3 }
    );

    let added_path = json!({
        "key": "added",
        "name": "Added path",
        "discovery": with_policy(discovery_step(), "all_required"),
        "detail": with_policy(detail_step(), "all_required")
    });
    let added = compile(
        profile_source(Some(json!([added_path])), "added"),
        profile.clone(),
    );
    assert_plan_policies(&added, StrategyPolicy::AllRequired);

    let mut added_discovery = discovery_step();
    added_discovery["policy"] = json!({ "type": "collect_all", "minAccepted": 3 });
    let mut added_detail = detail_step();
    added_detail["policy"] = json!({ "type": "collect_all", "minAccepted": 3 });
    let added_collect_all = compile(
        profile_source(
            Some(json!([{
                "key": "added_collect_all",
                "name": "Added collect all path",
                "discovery": added_discovery,
                "detail": added_detail
            }])),
            "added_collect_all",
        ),
        profile,
    );
    assert_plan_policies(
        &added_collect_all,
        StrategyPolicy::CollectAll { min_accepted: 3 },
    );

    let source_owned: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "owned",
        "name": "Owned",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "owned_path",
            "name": "Owned path",
            "discovery": with_policy(discovery_step(), "all_required"),
            "detail": with_policy(detail_step(), "all_required")
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Deterministic test source."
        }
    }))
    .unwrap();
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(&source_owned, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("valid Source-owned final path must compile");
    };
    assert!(diagnostics.is_empty());
    assert!(matches!(
        source.access,
        job_radar_lib::CompiledSourceAccess::SourceOwned { .. }
    ));
    assert_plan_policies(&source.execution_plan, StrategyPolicy::AllRequired);

    let mut source_owned_at_least = serde_json::to_value(&source_owned).unwrap();
    source_owned_at_least["selectedAccessPath"]["discovery"]["policy"] =
        json!({ "type": "at_least", "count": 2 });
    source_owned_at_least["selectedAccessPath"]["detail"]["policy"] =
        json!({ "type": "at_least", "count": 2 });
    let source_owned_at_least: SourceDocument =
        serde_json::from_value(source_owned_at_least).unwrap();
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(
        &source_owned_at_least,
        &SourceProfileRegistrySnapshot::default(),
    )
    else {
        panic!("valid Source-owned at_least path must compile");
    };
    assert!(diagnostics.is_empty());
    assert_plan_policies(&source.execution_plan, StrategyPolicy::AtLeast { count: 2 });

    let mut source_owned_collect_all = serde_json::to_value(source_owned).unwrap();
    source_owned_collect_all["selectedAccessPath"]["discovery"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": 3 });
    source_owned_collect_all["selectedAccessPath"]["detail"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": 3 });
    let source_owned_collect_all: SourceDocument =
        serde_json::from_value(source_owned_collect_all).unwrap();
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(
        &source_owned_collect_all,
        &SourceProfileRegistrySnapshot::default(),
    )
    else {
        panic!("valid Source-owned collect_all path must compile");
    };
    assert!(diagnostics.is_empty());
    assert_plan_policies(
        &source.execution_plan,
        StrategyPolicy::CollectAll { min_accepted: 3 },
    );
}

#[test]
fn at_least_count_is_validated_after_the_final_strategy_merge() {
    let mut profile_value = serde_json::to_value(profile_document()).unwrap();
    profile_value["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "at_least", "count": 4 });
    let profile: SourceProfileDocument = serde_json::from_value(profile_value).unwrap();
    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile.clone(),
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&profile_source(None, "main"), &registry)
    else {
        panic!("count above the final Strategy cardinality must reject compilation");
    };
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "strategy_policy_at_least_count_exceeds_cardinality"
            })
            .count(),
        1
    );

    let specialized = profile_source(
        Some(json!([{
            "key": "main",
            "discovery": {
                "strategies": [discovery_strategy(
                    "source_added",
                    "https://example.test/discovery/source-added"
                )]
            }
        }])),
        "main",
    );
    let specialized_outcome = compile_source(&specialized, &registry);
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = specialized_outcome
    else {
        panic!("Source-added Strategy must participate in final cardinality validation: {specialized_outcome:?}");
    };
    assert!(diagnostics.is_empty());
    assert_eq!(
        source.execution_plan.discovery.policy,
        StrategyPolicy::AtLeast { count: 4 }
    );
    assert_eq!(source.execution_plan.discovery.strategies.len(), 4);
}

#[test]
fn collect_all_minimum_is_validated_after_the_final_strategy_merge() {
    let mut profile_value = serde_json::to_value(profile_document()).unwrap();
    profile_value["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": 4 });
    let profile: SourceProfileDocument = serde_json::from_value(profile_value).unwrap();
    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile.clone(),
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&profile_source(None, "main"), &registry)
    else {
        panic!("minAccepted above final cardinality must reject compilation");
    };
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "strategy_policy_collect_all_min_accepted_exceeds_cardinality"
                    && diagnostic.path.ends_with("/discovery/policy/minAccepted")
            })
            .count(),
        1
    );

    let specialized = profile_source(
        Some(json!([{
            "key": "main",
            "discovery": {
                "strategies": [discovery_strategy(
                    "source_added",
                    "https://example.test/discovery/source-added"
                )]
            }
        }])),
        "main",
    );
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(&specialized, &registry)
    else {
        panic!("Source-added Strategy must participate in final cardinality validation");
    };
    assert!(diagnostics.is_empty());
    assert_eq!(
        source.execution_plan.discovery.policy,
        StrategyPolicy::CollectAll { min_accepted: 4 }
    );
    assert_eq!(source.execution_plan.discovery.strategies.len(), 4);
}

#[test]
fn first_accepted_execution_is_ordered_and_recovers_for_both_phases() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({
                "jobs": [{
                    "title": "Rejected partial output",
                    "company": "Example",
                    "url": "https://example.test/jobs/rejected"
                }]
            })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({
                "jobs": [{
                    "title": "Platform Engineer",
                    "company": "Example",
                    "url": "https://example.test/jobs/1"
                }]
            })
            .to_string()),
        ),
    ]);

    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Platform Engineer"
    );
    let report = &result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(
        report.usage.produced_items, 1,
        "only the accepted Strategy produces phase output"
    );
    assert_eq!(
        discovery.requests(),
        vec![
            "https://example.test/discovery/empty",
            "https://example.test/discovery/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["acceptance_min_results_not_met",]
    );

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "Rejected partial detail." }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "A complete accepted description." }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        result.payload.patch.description_text.as_deref(),
        Some("A complete accepted description.")
    );
    let report = &result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(report.usage.produced_items, 1);
    assert_eq!(
        detail.requests(),
        vec![
            "https://example.test/detail/failed",
            "https://example.test/detail/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["description_too_short",]
    );
}

#[test]
fn first_accepted_execution_stops_after_an_accepted_first_attempt() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([(
        "https://example.test/discovery/empty",
        Ok(json!({
            "jobs": [
                {
                    "title": "First accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/first"
                },
                {
                    "title": "Also accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/second"
                }
            ]
        })
        .to_string()),
    )]);
    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(discovery.requests().len(), 1);
    assert!(result.diagnostics.is_empty());

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([(
        "https://example.test/detail/failed",
        Ok(json!({ "description": accepted_description }).to_string()),
    )]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert!(result.payload.patch.description_text.is_some());
    assert_eq!(detail.requests().len(), 1);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn all_required_reduces_every_accepted_strategy_once_for_both_phases() {
    let plan = all_required_plan();
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [
                { "title": "Three", "company": "Example", "url": "https://example.test/jobs/3" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [
                { "title": "Four", "company": "Example", "url": "https://example.test/jobs/4" }
            ] })
            .to_string()),
        ),
    ]);

    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 4);
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.produced_items, 4);
    assert!(result.diagnostics.is_empty());
    let mut contributing_strategies = result
        .payload
        .provenance
        .iter()
        .flat_map(|evidence| evidence.contributors.iter())
        .map(|origin| origin.strategy_key.as_str())
        .collect::<Vec<_>>();
    contributing_strategies.sort_unstable();
    contributing_strategies.dedup();
    assert_eq!(contributing_strategies, vec!["accepted", "empty", "unused"]);

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "accepted ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": "accepted ".repeat(20) }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.produced_items, 3);
    assert_eq!(result.payload.provenance.len(), 1);
    assert_eq!(result.payload.provenance[0].contributors.len(), 3);
    assert!(result.payload.conflicts.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn collect_all_reduces_every_accepted_strategy_after_natural_completion_for_both_phases() {
    let plan = collect_all_plan(1);
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [
                { "title": "Three", "company": "Example", "url": "https://example.test/jobs/3" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [
                { "title": "Four", "company": "Example", "url": "https://example.test/jobs/4" }
            ] })
            .to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 4);
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(
        discovery.requests().len(),
        3,
        "minimum success is not terminal"
    );

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": accepted_description }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(detail.requests().len(), 3);
    assert_eq!(result.payload.provenance[0].contributors.len(), 3);
}

#[test]
fn collect_all_can_satisfy_its_minimum_despite_an_ordinary_failure() {
    let plan = collect_all_plan(2);
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Err("ordinary failure".to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [
                { "title": "Three", "company": "Example", "url": "https://example.test/jobs/3" }
            ] })
            .to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 3);
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec!["fetch_failed"]
    );

    let description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("ordinary failure".to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": description }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(result.payload.provenance[0].contributors.len(), 2);
    assert_eq!(result.diagnostics[0].code, "fetch_failed");
}

#[test]
fn collect_all_decides_dissatisfaction_only_after_every_strategy_for_both_phases() {
    let plan = collect_all_plan(3);
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Err("ordinary failure".to_string()),
        ),
    ]);
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap()
    else {
        panic!("unsatisfied collect_all must be payload-free")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert_eq!(
        discovery.requests().len(),
        3,
        "impossibility is not terminal"
    );
    let terminal = diagnostics.last().unwrap();
    assert_eq!(
        terminal.category,
        job_radar_lib::DiagnosticCategory::Runtime
    );
    assert_eq!(terminal.code, "strategy_policy_collect_all_unsatisfied");
    assert_eq!(terminal.message, "collect_all policy was not satisfied");
    assert_eq!(terminal.severity, job_radar_lib::DiagnosticSeverity::Error);
    assert_eq!(terminal.path, "/discovery/policy");
    assert_eq!(terminal.strategy_key, None);
    assert_eq!(
        terminal.details,
        Some(json!({ "policy": "collect_all", "requiredAccepted": 3 }))
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "accepted ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("ordinary failure".to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": "accepted ".repeat(20) }).to_string()),
        ),
    ]);
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ))
    .unwrap()
    else {
        panic!("unsatisfied collect_all detail must be payload-free")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert_eq!(detail.requests().len(), 3);
    assert_eq!(diagnostics.last().unwrap().path, "/detail/policy");
    assert_eq!(
        diagnostics.last().unwrap().code,
        "strategy_policy_collect_all_unsatisfied"
    );
}

#[test]
fn at_least_reduces_the_accepted_prefix_at_the_earliest_threshold_for_both_phases() {
    let plan = at_least_plan(2);
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [
                { "title": "Three", "company": "Example", "url": "https://example.test/jobs/3" }
            ] })
            .to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 3);
    assert_eq!(result.report.usage.strategy_attempts, 2);
    assert_eq!(discovery.requests().len(), 2);
    assert!(result.diagnostics.is_empty());

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": accepted_description }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.report.usage.strategy_attempts, 2);
    assert_eq!(detail.requests().len(), 2);
    assert_eq!(result.payload.provenance[0].contributors.len(), 2);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn at_least_stops_at_earliest_impossibility_without_exposing_a_prefix() {
    let plan = at_least_plan(3);
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
    ]);
    let result = block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.unwrap()
    else {
        panic!("impossible at_least must expose no accepted prefix")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 2);
    assert_eq!(discovery.requests().len(), 2);
    let terminal = diagnostics.last().unwrap();
    assert_eq!(
        terminal.category,
        job_radar_lib::DiagnosticCategory::Runtime
    );
    assert_eq!(terminal.code, "strategy_policy_at_least_unsatisfied");
    assert_eq!(terminal.message, "at_least policy was not satisfied");
    assert_eq!(terminal.severity, job_radar_lib::DiagnosticSeverity::Error);
    assert_eq!(terminal.path, "/discovery/policy");
    assert_eq!(terminal.strategy_key, None);
    assert_eq!(
        terminal.details,
        Some(json!({ "policy": "at_least", "requiredAccepted": 3 }))
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "accepted ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("ordinary failure".to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        diagnostics,
        ..
    } = result.unwrap()
    else {
        panic!("failed at_least must expose no accepted detail prefix")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(detail.requests().len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec!["fetch_failed", "strategy_policy_at_least_unsatisfied"]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/detail/policy");
}

#[test]
fn at_least_final_acceptance_failure_is_payload_free_and_appends_its_terminal() {
    let plan = at_least_plan_with_detail_phase_acceptance(2);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "first ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "second ".repeat(20) }).to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.unwrap()
    else {
        panic!("unsatisfied final acceptance must expose no reduced patch")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "detail_field_conflict",
            "acceptance_required_field_missing",
            "strategy_policy_at_least_unsatisfied"
        ]
    );
}

#[test]
fn at_least_exact_limit_succeeds_and_denial_takes_precedence() {
    let plan = at_least_plan(2);
    let accepted_description = "accepted ".repeat(20);
    let exact_limit = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        exact_limit.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
            max_requests: 2,
            ..PhaseLimits::BACKEND
        }),
    )));
    assert_eq!(result.report.usage.requests, 2);
    assert_eq!(result.report.usage.strategy_attempts, 2);

    let denied = DetailScriptedClient::new([(
        "https://example.test/detail/failed",
        Ok(json!({ "description": accepted_description }).to_string()),
    )]);
    let result = budget_exhausted(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        denied.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
            max_requests: 1,
            ..PhaseLimits::BACKEND
        }),
    )));
    assert_eq!(result.report.usage.requests, 1);
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "strategy_policy_at_least_unsatisfied"
            && diagnostic.code != "fallback_exhausted"
    }));
}

#[test]
fn collect_all_reducer_conflicts_do_not_retroactively_reject_a_satisfied_policy() {
    let responses = || {
        DetailScriptedClient::new([
            (
                "https://example.test/detail/failed",
                Ok(json!({ "description": "first ".repeat(20) }).to_string()),
            ),
            (
                "https://example.test/detail/accepted",
                Ok(json!({ "description": "second ".repeat(20) }).to_string()),
            ),
            (
                "https://example.test/detail/unused",
                Ok(json!({ "description": "third ".repeat(20) }).to_string()),
            ),
        ])
    };
    let detail = responses();
    let result = accepted_phase(block_on(execute_detail(
        &collect_all_plan(2),
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(result.payload.conflicts.len(), 1);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "strategy_policy_collect_all_unsatisfied"));

    let detail = responses();
    let result = policy_unsatisfied(
        block_on(execute_detail(
            &collect_all_plan_with_detail_phase_acceptance(2),
            empty_source_config(),
            &posting(),
            RequestedDetailFields::description_text(),
            detail.client(),
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )),
        PolicyUnsatisfiedCause::RejectedOnly,
    );
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(
        result.diagnostics.last().unwrap().code,
        "strategy_policy_collect_all_unsatisfied"
    );
}

#[test]
fn collect_all_exact_limit_reaches_natural_completion_and_denial_takes_precedence() {
    let plan = collect_all_plan(1);
    let description = "accepted ".repeat(20);
    let exact_limit = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": description.clone() }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        exact_limit.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
            max_requests: 3,
            ..PhaseLimits::BACKEND
        }),
    )));
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.strategy_attempts, 3);

    let denied = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": description }).to_string()),
        ),
    ]);
    let result = budget_exhausted(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        denied.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
            max_requests: 2,
            ..PhaseLimits::BACKEND
        }),
    )));
    assert_eq!(result.report.usage.requests, 2);
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "strategy_policy_collect_all_unsatisfied"
            && diagnostic.code != "fallback_exhausted"
    }));
}

#[test]
fn all_required_fails_fast_without_reducing_or_exposing_an_accepted_prefix() {
    let plan = all_required_plan();
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
    ]);
    let result = block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.unwrap()
    else {
        panic!("required rejection must be payload-free")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 2);
    assert_eq!(complete_budget_report.usage.requests, 2);
    assert_eq!(discovery.requests().len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "acceptance_min_results_not_met",
            "strategy_policy_all_required_unsatisfied"
        ]
    );
    let terminal = diagnostics.last().unwrap();
    assert_eq!(
        terminal.category,
        job_radar_lib::DiagnosticCategory::Runtime
    );
    assert_eq!(terminal.severity, job_radar_lib::DiagnosticSeverity::Error);
    assert_eq!(terminal.path, "/discovery/policy");
    assert_eq!(terminal.strategy_key, None);
    assert_eq!(terminal.message, "all_required policy was not satisfied");
    assert_eq!(terminal.details, Some(json!({ "policy": "all_required" })));
    let serialized = serde_json::to_string(
        &PolicyOutcome::<job_radar_lib::DiscoveryPhasePayload>::PolicyUnsatisfied { cause },
    )
    .unwrap();
    assert!(!serialized.contains("candidates"));

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("required failure".to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.unwrap()
    else {
        panic!("required failure must be payload-free")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 2);
    assert_eq!(detail.requests().len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["fetch_failed", "strategy_policy_all_required_unsatisfied"]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/detail/policy");
}

#[test]
fn all_required_reducer_conflicts_do_not_retroactively_reject_the_policy() {
    let plan = all_required_plan();
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "first ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "second value" }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": "third value" }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(result.payload.patch.description_text, None);
    assert_eq!(result.payload.conflicts.len(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["detail_field_conflict"]
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "strategy_policy_all_required_unsatisfied"));
}

#[test]
fn all_required_final_acceptance_failure_appends_the_policy_terminal() {
    let plan = all_required_plan_with_detail_phase_acceptance();
    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "first ".repeat(20) }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "second value" }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": "third value" }).to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.unwrap()
    else {
        panic!("unsatisfied final acceptance must expose no reduced patch")
    };

    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "detail_field_conflict",
            "acceptance_required_field_missing",
            "strategy_policy_all_required_unsatisfied"
        ]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/detail/policy");
}

#[test]
fn all_required_exact_limit_succeeds_and_one_over_is_budget_exhausted() {
    let plan = all_required_plan();
    let accepted_description = "accepted ".repeat(20);
    let exact_limit = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
    ]);
    let caller = PhaseLimits {
        max_requests: 3,
        ..PhaseLimits::BACKEND
    };
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        exact_limit.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(caller),
    )));
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.strategy_attempts, 3);

    let one_over = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": accepted_description.clone() }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": accepted_description }).to_string()),
        ),
    ]);
    let caller = PhaseLimits {
        max_requests: 2,
        ..PhaseLimits::BACKEND
    };
    let result = budget_exhausted(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        one_over.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(caller),
    )));
    assert_eq!(result.report.usage.requests, 2);
    assert_eq!(result.report.usage.strategy_attempts, 3);
    assert_eq!(one_over.requests().len(), 2);
    assert!(result.diagnostics.iter().all(|d| {
        d.code != "strategy_policy_all_required_unsatisfied" && d.code != "fallback_exhausted"
    }));
}

#[test]
fn first_accepted_exhaustion_adds_one_terminal_after_attempt_diagnostics() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
    ]);

    let result = block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.expect("phase completes without control-flow error")
    else {
        panic!("expected payload-free PolicyUnsatisfied completion")
    };

    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::PolicyUnsatisfied
    );
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert!(!serde_json::to_string(
        &PolicyOutcome::<job_radar_lib::DiscoveryPhasePayload>::PolicyUnsatisfied {
            cause: cause.clone()
        }
    )
    .unwrap()
    .contains("candidates"));
    let codes = diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "fallback_exhausted",
        ]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/discovery/strategies");

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Err("failed one".to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("failed two".to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Err("failed three".to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.expect("phase completes without control-flow error")
    else {
        panic!("expected payload-free PolicyUnsatisfied completion")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::PolicyUnsatisfied
    );
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert_eq!(complete_budget_report.usage.produced_items, 0);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "fetch_failed",
            "fetch_failed",
            "fetch_failed",
            "fallback_exhausted"
        ]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/detail/strategies");
}

#[test]
fn detail_request_one_over_is_budget_exhausted_with_no_patch() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let detail = DetailScriptedClient::new([(
        "https://example.test/detail/failed",
        Err("first failed".to_string()),
    )]);
    let caller = PhaseLimits {
        max_requests: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(caller),
    )));

    let report = result.report;
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("expected Detail budget exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(report.usage.produced_items, 0);
    assert_eq!(detail.requests().len(), 1);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

#[test]
fn detail_browser_1999_ms_compiled_and_caller_limits_are_rejected_without_panic() {
    let mut plan = compile(profile_source(None, "main"), profile_document());
    let detail_plan = plan.detail.as_mut().expect("fixture has Detail");
    detail_plan.strategies[0].fetch = ExecutionPlanFetch::Browser {
        url: job_radar_lib::compile_template(
            "https://example.test/detail/browser",
            &job_radar_lib::TemplateDescriptor::new(),
        )
        .unwrap(),
        timeout_ms: 1_000,
        waits: Vec::new(),
        interactions: Vec::new(),
    };
    detail_plan.limits.max_duration_ms = 1_999;
    let detail = DetailScriptedClient::new([]);
    let acquisition = ScriptedBrowserAcquisition::new([]);

    let invalid_plan_diagnostics = not_started(
        block_on(execute_detail(
            &plan,
            empty_source_config(),
            &posting(),
            RequestedDetailFields::description_text(),
            detail.client(),
            PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
            RuntimeExecutionContext::uncancellable(),
        )),
        job_radar_lib::PhasePreStartFailure::PlanMismatch,
    );

    assert!(invalid_plan_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "invalid_compiled_browser_phase_duration"
            && diagnostic.path == "/detail/limits/maxDurationMs"
    }));

    plan.detail.as_mut().unwrap().limits.max_duration_ms = PhaseLimits::BACKEND.max_duration_ms;
    let caller = PhaseLimits {
        max_duration_ms: 1_999,
        ..PhaseLimits::BACKEND
    };
    let caller_result = execution_failed(
        block_on(execute_detail(
            &plan,
            empty_source_config(),
            &posting(),
            RequestedDetailFields::description_text(),
            detail.client(),
            PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )),
        job_radar_lib::PhaseExecutionFailure::InvalidCallerLimits,
    );

    let report = caller_result.report;
    assert_eq!(report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage, Default::default());
    assert!(caller_result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_caller_phase_limits"));
    assert!(detail.requests().is_empty());
}

#[test]
fn all_required_cancellation_discards_the_accepted_prefix_without_a_policy_terminal() {
    let plan = all_required_plan();
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/discovery/empty".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()
            .into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };

    let result = cancelled(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));

    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(result.complete_budget_report.usage.strategy_attempts, 1);
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "strategy_policy_all_required_unsatisfied"
            && diagnostic.code != "fallback_exhausted"
    }));
}

#[test]
fn at_least_cancellation_discards_the_accepted_prefix_without_a_policy_terminal() {
    let plan = at_least_plan(2);
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/discovery/empty".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()
            .into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };

    let result = cancelled(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));

    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(result.complete_budget_report.usage.strategy_attempts, 1);
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "strategy_policy_at_least_unsatisfied"
            && diagnostic.code != "fallback_exhausted"
    }));
}

#[test]
fn collect_all_cancellation_discards_retained_output_without_a_policy_terminal() {
    let plan = collect_all_plan(1);
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/discovery/empty".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "jobs": [
                { "title": "One", "company": "Example", "url": "https://example.test/jobs/1" },
                { "title": "Two", "company": "Example", "url": "https://example.test/jobs/2" }
            ] })
            .to_string()
            .into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };

    let result = cancelled(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));

    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(result.complete_budget_report.usage.strategy_attempts, 1);
    assert!(result.diagnostics.iter().all(|diagnostic| {
        diagnostic.code != "strategy_policy_collect_all_unsatisfied"
            && diagnostic.code != "fallback_exhausted"
    }));
}

#[test]
fn cancellation_discards_an_accepted_attempt_and_suppresses_later_work_and_exhaustion() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/discovery/empty".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "jobs": [] }).to_string().into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };

    let result = cancelled(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));

    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "fallback_exhausted"));

    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/detail/failed".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "description": "Discarded detail description." })
                .to_string()
                .into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };
    let result = cancelled(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));
    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

fn assert_plan_policies(plan: &SourceExecutionPlan, expected: StrategyPolicy) {
    assert_eq!(plan.discovery.policy, expected);
    assert_eq!(plan.detail.as_ref().unwrap().policy, expected);
}

fn compile(source: SourceDocument, profile: SourceProfileDocument) -> SourceExecutionPlan {
    let outcome = compile_source(
        &source,
        &SourceProfileRegistrySnapshot {
            profiles: vec![RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document: profile,
            }],
            sources: Vec::new(),
            diagnostics: Vec::new(),
        },
    );
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = outcome
    else {
        panic!("valid final source must compile: {outcome:?}");
    };
    assert!(diagnostics.is_empty());
    source.execution_plan
}

fn all_required_plan() -> SourceExecutionPlan {
    compile(
        profile_source(None, "main"),
        all_required_profile_document(),
    )
}

fn at_least_plan(count: usize) -> SourceExecutionPlan {
    let mut profile = serde_json::to_value(profile_document()).unwrap();
    profile["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "at_least", "count": count });
    profile["accessPaths"][0]["detail"]["policy"] = json!({ "type": "at_least", "count": count });
    compile(
        profile_source(None, "main"),
        serde_json::from_value(profile).unwrap(),
    )
}

fn collect_all_plan(min_accepted: usize) -> SourceExecutionPlan {
    let mut profile = serde_json::to_value(profile_document()).unwrap();
    profile["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": min_accepted });
    profile["accessPaths"][0]["detail"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": min_accepted });
    compile(
        profile_source(None, "main"),
        serde_json::from_value(profile).unwrap(),
    )
}

fn collect_all_plan_with_detail_phase_acceptance(min_accepted: usize) -> SourceExecutionPlan {
    let mut profile = serde_json::to_value(profile_document()).unwrap();
    profile["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": min_accepted });
    profile["accessPaths"][0]["detail"]["policy"] =
        json!({ "type": "collect_all", "minAccepted": min_accepted });
    profile["accessPaths"][0]["detail"]["acceptWhen"] =
        json!({ "requiredFields": ["descriptionText"] });
    compile(
        profile_source(None, "main"),
        serde_json::from_value(profile).unwrap(),
    )
}

fn at_least_plan_with_detail_phase_acceptance(count: usize) -> SourceExecutionPlan {
    let mut profile = serde_json::to_value(profile_document()).unwrap();
    profile["accessPaths"][0]["discovery"]["policy"] =
        json!({ "type": "at_least", "count": count });
    profile["accessPaths"][0]["detail"]["policy"] = json!({ "type": "at_least", "count": count });
    profile["accessPaths"][0]["detail"]["acceptWhen"] =
        json!({ "requiredFields": ["descriptionText"] });
    compile(
        profile_source(None, "main"),
        serde_json::from_value(profile).unwrap(),
    )
}

fn all_required_profile_document() -> SourceProfileDocument {
    let mut profile = serde_json::to_value(profile_document()).unwrap();
    profile["accessPaths"][0]["discovery"]["policy"] = json!({ "type": "all_required" });
    profile["accessPaths"][0]["detail"]["policy"] = json!({ "type": "all_required" });
    serde_json::from_value(profile).unwrap()
}

fn all_required_plan_with_detail_phase_acceptance() -> SourceExecutionPlan {
    let mut profile = serde_json::to_value(all_required_profile_document()).unwrap();
    profile["accessPaths"][0]["detail"]["acceptWhen"] =
        json!({ "requiredFields": ["descriptionText"] });
    compile(
        profile_source(None, "main"),
        serde_json::from_value(profile).unwrap(),
    )
}

fn profile_document() -> SourceProfileDocument {
    serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "policy_profile",
        "name": "Policy profile",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Deterministic policy test profile."
        },
        "accessPaths": [{
            "key": "main",
            "name": "Main",
            "discovery": discovery_step(),
            "detail": detail_step()
        }]
    }))
    .unwrap()
}

fn profile_source(access_paths: Option<Value>, path_key: &str) -> SourceDocument {
    let mut value = json!({
        "schemaVersion": 3,
        "key": "policy_source",
        "name": "Policy source",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "policy_profile",
            "pathKey": path_key
        }
    });
    if let Some(access_paths) = access_paths {
        value["accessPaths"] = access_paths;
    }
    serde_json::from_value(value).unwrap()
}

fn discovery_step() -> Value {
    let mut rejected = discovery_strategy("empty", "https://example.test/discovery/empty");
    rejected["acceptWhen"] = json!({ "minResults": 2 });
    json!({
        "policy": { "type": "first_accepted" },
        "acceptWhen": { "minResults": 1 },
        "strategies": [
            rejected,
            discovery_strategy("accepted", "https://example.test/discovery/accepted"),
            discovery_strategy("unused", "https://example.test/discovery/unused")
        ]
    })
}

fn with_policy(mut step: Value, policy_type: &str) -> Value {
    step["policy"] = json!({ "type": policy_type });
    step
}

fn discovery_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
        "parse": { "type": "json" },
        "select": { "type": "json_path", "jsonPath": "$.jobs" },
        "extract": {
            "reference": {
                "url": { "type": "json_path", "jsonPath": "$.url" }
            },
            "providerValues": {
                "title": { "type": "json_path", "jsonPath": "$.title" },
                "company": { "type": "json_path", "jsonPath": "$.company" }
            }
        }
    })
}

fn detail_step() -> Value {
    let mut rejected = detail_strategy("failed", "https://example.test/detail/failed");
    rejected["acceptWhen"] = json!({ "minDescriptionLength": 100 });
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [
            rejected,
            detail_strategy("accepted", "https://example.test/detail/accepted"),
            detail_strategy("unused", "https://example.test/detail/unused")
        ]
    })
}

fn detail_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
            "fields": {
                "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
            }
        }
    })
}

fn posting() -> PostingOccurrence {
    let (reference, identity) =
        job_radar_lib::validate_posting_reference("example", "https://example.test/jobs/1", None)
            .unwrap();
    PostingOccurrence {
        identity,
        reference,
        provider_values: Default::default(),
        hints: Default::default(),
        posting_meta: BTreeMap::new(),
    }
}

struct DiscoveryScriptedClient {
    client: ScriptedProfileHttpClient,
}

impl DiscoveryScriptedClient {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            client: scripted_client(entries),
        }
    }

    fn client(&self) -> &ScriptedProfileHttpClient {
        &self.client
    }

    fn requests(&self) -> Vec<String> {
        self.client
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect()
    }
}

struct DetailScriptedClient {
    client: ScriptedProfileHttpClient,
}

impl DetailScriptedClient {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            client: scripted_client(entries),
        }
    }

    fn client(&self) -> &ScriptedProfileHttpClient {
        &self.client
    }

    fn requests(&self) -> Vec<String> {
        self.client
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect()
    }
}

fn scripted_client<const N: usize>(
    entries: [(&str, Result<String, String>); N],
) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new(entries.into_iter().map(|(url, result)| {
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: url.to_string(),
            headers: Vec::new(),
            body: vec![match result {
                Ok(body) => ScriptedHttpBodyEvent::Chunk(body.into_bytes()),
                Err(_) => ScriptedHttpBodyEvent::Failure(ProfileHttpFailureKind::BodyStream),
            }],
            content_length: None,
        }
    }))
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

struct RequestObservedCancellation<'a> {
    client: &'a ScriptedProfileHttpClient,
}

impl RuntimeCancellation for RequestObservedCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.client.request_count() > 0
    }
}
