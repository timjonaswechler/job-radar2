use search_resolution::{
    Requirements, RequirementsCompilationFailure, ResolutionCompletion, ResolutionError,
    ResolutionFailure, Resolver, SearchRule, SearchRuleKind, SearchRuleTarget,
};
use serde_json::{json, Value};
use source_engine::test_support::{
    compile_source, BrowserAcquisitionRequestSnapshot, CompileSourceOutcome, CompiledSource,
    PhaseLimits, ProfileCompilerInput, ProfileHttpFailureKind, RuntimeCancellation,
    ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient, SourceBehavior, SourceProfileDocument,
};

struct NeverCancelled;
impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct Cancelled;
impl RuntimeCancellation for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

struct CancelAfterRequest<'a>(&'a ScriptedProfileHttpClient);
impl RuntimeCancellation for CancelAfterRequest<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.request_count() > 0
    }
}

struct SamePlace;
impl geo::GeoResolver for SamePlace {
    fn resolve<'a>(&'a self, input: &'a str) -> geo::GeoResolveFuture<'a> {
        Box::pin(async move {
            Ok(vec![geo::ResolvedLocation {
                input: input.into(),
                label: input.into(),
                point: geo::GeoPoint {
                    latitude: 52.52,
                    longitude: 13.405,
                },
            }])
        })
    }
}

fn rule(kind: SearchRuleKind, value: &str) -> SearchRule {
    SearchRule {
        target: SearchRuleTarget::Title,
        kind,
        value: value.into(),
    }
}

fn requirements(include: &[SearchRule], exclude: &[SearchRule]) -> Requirements<'static> {
    Requirements::compile(include, exclude, &[], None).unwrap()
}

fn engineer_requirements() -> Requirements<'static> {
    requirements(&[rule(SearchRuleKind::Text, "engineer")], &[])
}

fn compiled_source(
    discovery_extract: Value,
    pagination: Option<Value>,
    detail_extract: Option<Value>,
) -> CompiledSource {
    let mut discovery_strategy = json!({
        "key": "discovery",
        "fetch": {
            "mode": "http",
            "method": "GET",
            "url": "{{sourceConfig:feedUrl}}",
            "timeoutMs": 10000
        },
        "parse": { "type": "json" },
        "select": { "type": "json_path", "jsonPath": "$.jobs" },
        "extract": discovery_extract
    });
    if let Some(pagination) = pagination {
        discovery_strategy["pagination"] = pagination;
    }

    let mut access_path = json!({
        "key": "default",
        "name": "Default",
        "discovery": {
            "policy": { "type": "first_accepted" },
            "strategies": [discovery_strategy]
        }
    });
    if let Some(fields) = detail_extract {
        access_path["detail"] = json!({
            "policy": { "type": "first_accepted" },
            "strategies": [{
                "key": "detail",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/detail",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "extract": { "fields": fields }
            }]
        });
    }

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "fixture_profile",
        "name": "Fixture",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Candidate Resolution fixture"
        },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [access_path]
    }))
    .unwrap();
    let source: SourceBehavior = serde_json::from_value(json!({
        "key": "fixture_source",
        "name": "Fixture source",
        "sourceConfig": { "feedUrl": "https://example.test/feed" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "fixture_profile",
            "pathKey": "default"
        }
    }))
    .unwrap();

    match compile_source(&source, &ProfileCompilerInput::new(&[profile])) {
        CompileSourceOutcome::Compiled { source, .. } => source,
        other => panic!("fixture did not compile: {other:?}"),
    }
}

fn compiled_conflicting_source() -> CompiledSource {
    let extract = json!({
        "reference": { "url": { "type": "json_path", "jsonPath": "$.url" } },
        "providerValues": {
            "title": { "type": "json_path", "jsonPath": "$.title" },
            "company": { "type": "json_path", "jsonPath": "$.company" }
        }
    });
    let strategy = |key: &str, config_key: &str| {
        json!({
            "key": key,
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": format!("{{{{sourceConfig:{config_key}}}}}"),
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "json_path", "jsonPath": "$.jobs" },
            "extract": extract.clone()
        })
    };
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "conflict_profile",
        "name": "Conflict fixture",
        "kind": "generic",
        "support": { "level": "experimental", "summary": "Conflict fixture" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedA", "feedB"],
            "properties": {
                "feedA": { "type": "string" },
                "feedB": { "type": "string" }
            },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "default",
            "name": "Default",
            "discovery": {
                "policy": { "type": "collect_all", "minAccepted": 1 },
                "strategies": [strategy("a", "feedA"), strategy("b", "feedB")]
            }
        }]
    }))
    .unwrap();
    let source: SourceBehavior = serde_json::from_value(json!({
        "key": "conflict_source",
        "name": "Conflict source",
        "sourceConfig": {
            "feedA": "https://example.test/a",
            "feedB": "https://example.test/b"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "conflict_profile",
            "pathKey": "default"
        }
    }))
    .unwrap();
    match compile_source(&source, &ProfileCompilerInput::new(&[profile])) {
        CompileSourceOutcome::Compiled { source, .. } => source,
        other => panic!("conflict fixture did not compile: {other:?}"),
    }
}

fn compiled_browser_source() -> CompiledSource {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "browser_profile",
        "name": "Browser fixture",
        "kind": "generic",
        "support": { "level": "experimental", "summary": "Browser fixture" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["startUrl"],
            "properties": { "startUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "default",
            "name": "Default",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "browser",
                    "fetch": {
                        "mode": "browser",
                        "url": "{{sourceConfig:startUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "html" },
                    "select": { "type": "css", "selector": "article" },
                    "extract": {
                        "reference": {
                            "url": {
                                "type": "css_attribute",
                                "selector": "a",
                                "attribute": "href",
                                "cardinality": "one"
                            }
                        },
                        "providerValues": {
                            "title": {
                                "type": "css_text",
                                "selector": ".title",
                                "cardinality": "one"
                            },
                            "company": {
                                "type": "css_text",
                                "selector": ".company",
                                "cardinality": "one"
                            }
                        }
                    }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceBehavior = serde_json::from_value(json!({
        "key": "browser_source",
        "name": "Browser source",
        "sourceConfig": { "startUrl": "https://example.test/jobs" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "browser_profile",
            "pathKey": "default"
        }
    }))
    .unwrap();
    match compile_source(&source, &ProfileCompilerInput::new(&[profile])) {
        CompileSourceOutcome::Compiled { source, .. } => source,
        other => panic!("browser fixture did not compile: {other:?}"),
    }
}

fn canonical_extract() -> Value {
    json!({
        "reference": {
            "url": { "type": "json_path", "jsonPath": "$.url" },
            "providerPostingId": {
                "type": "json_path",
                "jsonPath": "$.id",
                "cardinality": "optional"
            }
        },
        "providerValues": {
            "title": {
                "type": "json_path",
                "jsonPath": "$.title",
                "cardinality": "optional"
            },
            "company": {
                "type": "json_path",
                "jsonPath": "$.company",
                "cardinality": "optional"
            },
            "locations": {
                "type": "json_path",
                "jsonPath": "$.locations",
                "cardinality": "all"
            }
        }
    })
}

fn response(url: &str, body: Value) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: url.into(),
        headers: vec![],
        body: vec![ScriptedHttpBodyEvent::Chunk(body.to_string().into_bytes())],
        content_length: None,
    }
}

fn failed_response(url: &str) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: url.into(),
        headers: vec![],
        body: vec![ScriptedHttpBodyEvent::Failure(
            ProfileHttpFailureKind::BodyStream,
        )],
        content_length: None,
    }
}

#[test]
fn requirements_reject_invalid_regex_and_radius_without_geo() {
    assert_eq!(
        Requirements::compile(&[rule(SearchRuleKind::Regex, "(")], &[], &[], None,).unwrap_err(),
        RequirementsCompilationFailure::InvalidRegex
    );
    assert_eq!(
        Requirements::compile(
            &[rule(SearchRuleKind::Text, "engineer")],
            &[],
            &["Berlin".into()],
            Some(25),
        )
        .unwrap_err(),
        RequirementsCompilationFailure::RadiusRequiresGeoResolver
    );
}

#[tokio::test]
async fn no_radius_is_observable_while_prepared_geo_matching_remains_final() {
    let source = compiled_source(canonical_extract(), None, None);
    let candidate = json!({ "jobs": [{
        "id": "1",
        "title": "Engineer",
        "company": "ACME",
        "locations": ["Berlin"],
        "url": "https://example.test/jobs/1"
    }]});

    let no_radius = Requirements::compile(
        &[rule(SearchRuleKind::Text, "engineer")],
        &[],
        &["Berlin".into()],
        None,
    )
    .unwrap();
    let no_radius_http =
        ScriptedProfileHttpClient::new([response("https://example.test/feed", candidate.clone())]);
    let no_radius_browser = ScriptedBrowserAcquisition::new([]);
    let no_radius_resolution = Resolver::new(&no_radius_http, &no_radius_browser)
        .resolve(&source, &no_radius, &NeverCancelled)
        .await
        .unwrap();
    assert_eq!(no_radius_resolution.finalized.len(), 1);
    assert!(no_radius_resolution
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "location_filter_not_applied_missing_radius_km"));

    let geo = SamePlace;
    let prepared = Requirements::compile_with_geo(
        &[rule(SearchRuleKind::Text, "engineer")],
        &[],
        &["Berlin".into()],
        Some(10),
        &geo,
    )
    .await
    .unwrap();
    let geo_http =
        ScriptedProfileHttpClient::new([response("https://example.test/feed", candidate)]);
    let geo_browser = ScriptedBrowserAcquisition::new([]);
    let geo_resolution = Resolver::new(&geo_http, &geo_browser)
        .resolve(&source, &prepared, &NeverCancelled)
        .await
        .unwrap();
    assert_eq!(geo_resolution.finalized.len(), 1);
}

#[tokio::test]
async fn resolver_materializes_paginated_discovery_and_returns_only_normalized_final_candidates() {
    let source = compiled_source(
        canonical_extract(),
        Some(json!({
            "type": "page",
            "pageParam": "page",
            "firstPage": 1,
            "pageSizeParam": "per_page",
            "pageSize": 1,
            "totalPath": "$.total",
            "limits": { "maxRequests": 2 }
        })),
        None,
    );
    let http = ScriptedProfileHttpClient::new([
        response(
            "https://example.test/feed?page=1&per_page=1",
            json!({
                "total": 2,
                "jobs": [{
                    "id": "1",
                    "title": "  Software   Engineer ",
                    "company": " ACME ",
                    "locations": ["  Berlin  "],
                    "url": "https://example.test/jobs/1"
                }]
            }),
        ),
        response(
            "https://example.test/feed?page=2&per_page=1",
            json!({
                "total": 2,
                "jobs": [{
                    "id": "2",
                    "title": "Platform Engineer",
                    "company": "ACME",
                    "locations": ["Remote"],
                    "url": "https://example.test/jobs/2"
                }]
            }),
        ),
    ]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.completion, ResolutionCompletion::Complete);
    assert_eq!(
        (
            resolution.counts.discovered,
            resolution.counts.processed,
            resolution.counts.finalized,
        ),
        (2, 2, 2)
    );
    assert_eq!(resolution.finalized[0].title(), "Software Engineer");
    assert_eq!(resolution.finalized[0].company(), "ACME");
    assert_eq!(resolution.finalized[0].locations(), ["Berlin"]);
    assert_eq!(http.requests().len(), 2);
    assert_eq!(resolution.usage.requests, 2);
    assert!(browser.requests().is_empty());
}

#[tokio::test]
async fn conflicting_provider_values_cannot_become_final_candidates() {
    let source = compiled_conflicting_source();
    let http = ScriptedProfileHttpClient::new([
        response(
            "https://example.test/a",
            json!({ "jobs": [{
                "title": "Engineer",
                "company": "ACME",
                "url": "https://example.test/jobs/1"
            }]}),
        ),
        response(
            "https://example.test/b",
            json!({ "jobs": [{
                "title": "Accountant",
                "company": "ACME",
                "url": "https://example.test/jobs/1"
            }]}),
        ),
    ]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.counts.discovered, 1);
    assert_eq!(resolution.counts.unresolved, 1);
    assert!(resolution.finalized.is_empty());
}

#[tokio::test]
async fn resolver_executes_browser_backed_discovery_through_the_productive_adapter() {
    let source = compiled_browser_source();
    let http = ScriptedProfileHttpClient::new([]);
    let browser = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            target: "https://example.test/jobs".into(),
            timeout_ms: 10000,
            waits: vec![],
            interactions: vec![],
            browser_rendered_bytes_remaining: PhaseLimits::BACKEND.max_browser_rendered_bytes,
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content(
                "<article><span class='title'>Engineer</span><span class='company'>ACME</span><a href='https://example.test/1'></a></article>".into(),
            ),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.finalized.len(), 1);
    assert_eq!(browser.requests().len(), 1);
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn include_and_exclusion_text_and_regex_meaning_is_applied_to_final_titles() {
    let source = compiled_source(canonical_extract(), None, None);
    let jobs = json!({ "jobs": [
        { "id": "1", "title": "Engineer", "company": "ACME", "locations": [], "url": "https://example.test/1" },
        { "id": "2", "title": "engineer", "company": "ACME", "locations": [], "url": "https://example.test/2" },
        { "id": "3", "title": "Engineer Accountant", "company": "ACME", "locations": [], "url": "https://example.test/3" },
        { "id": "4", "title": "Engineer Manager", "company": "ACME", "locations": [], "url": "https://example.test/4" }
    ]});
    let http = ScriptedProfileHttpClient::new([response("https://example.test/feed", jobs)]);
    let browser = ScriptedBrowserAcquisition::new([]);
    let requirements = requirements(
        &[rule(SearchRuleKind::Regex, "^Engineer")],
        &[
            rule(SearchRuleKind::Regex, "accountant"),
            rule(SearchRuleKind::Text, "manager"),
        ],
    );

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &requirements, &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.finalized.len(), 1);
    assert_eq!(resolution.finalized[0].title(), "Engineer");
    assert_eq!(resolution.counts.rejected, 3);
}

#[tokio::test]
async fn authorized_title_hints_only_reject_when_the_canonical_title_is_missing() {
    let mut extract = canonical_extract();
    extract["hints"] = json!({
        "title": {
            "value": {
                "type": "json_path",
                "jsonPath": "$.titleHint",
                "cardinality": "optional"
            },
            "hintUse": "search_prefilter"
        }
    });
    let source = compiled_source(extract, None, None);
    let http = ScriptedProfileHttpClient::new([response(
        "https://example.test/feed",
        json!({ "jobs": [
            {
                "id": "canonical",
                "title": "Engineer",
                "company": "ACME",
                "locations": [],
                "titleHint": "Accountant",
                "url": "https://example.test/canonical"
            },
            {
                "id": "rejected",
                "company": "ACME",
                "locations": [],
                "titleHint": "Accountant",
                "url": "https://example.test/rejected"
            }
        ]}),
    )]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.finalized.len(), 1);
    assert_eq!(resolution.counts.rejected, 1);
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn inert_title_hints_cannot_reject_and_missing_values_are_loaded_lazily() {
    let mut extract = canonical_extract();
    extract["hints"] = json!({
        "title": {
            "value": {
                "type": "json_path",
                "jsonPath": "$.titleHint",
                "cardinality": "optional"
            }
        }
    });
    let source = compiled_source(
        extract,
        None,
        Some(json!({
            "title": { "type": "json_path", "jsonPath": "$.title" },
            "company": { "type": "json_path", "jsonPath": "$.company" }
        })),
    );
    let http = ScriptedProfileHttpClient::new([
        response(
            "https://example.test/feed",
            json!({ "jobs": [{
                "id": "detail",
                "company": "ACME",
                "locations": [],
                "titleHint": "Accountant",
                "url": "https://example.test/detail-candidate"
            }]}),
        ),
        response(
            "https://example.test/detail",
            json!({ "title": "Engineer", "company": "ACME" }),
        ),
    ]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.finalized.len(), 1);
    assert_eq!(resolution.counts.rejected, 0);
    assert_eq!(http.requests().len(), 2);
}

#[tokio::test]
async fn discovery_allowance_exhaustion_returns_a_bounded_partial_resolution() {
    let source = compiled_source(
        canonical_extract(),
        Some(json!({
            "type": "page",
            "pageParam": "page",
            "firstPage": 1,
            "pageSizeParam": "per_page",
            "pageSize": 1,
            "totalPath": "$.total",
            "limits": { "maxRequests": 1 }
        })),
        None,
    );
    let http = ScriptedProfileHttpClient::new([response(
        "https://example.test/feed?page=1&per_page=1",
        json!({
            "total": 2,
            "jobs": [{
                "id": "1",
                "title": "Engineer",
                "company": "ACME",
                "locations": [],
                "url": "https://example.test/1"
            }]
        }),
    )]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert!(matches!(
        resolution.completion,
        ResolutionCompletion::Partial { .. }
    ));
    assert_eq!(resolution.counts.discovered, 0);
    assert_eq!(resolution.usage.requests, 1);
}

#[tokio::test]
async fn candidate_detail_failures_are_counted_and_diagnostics_are_sampled_at_the_bound() {
    let source = compiled_source(
        canonical_extract(),
        None,
        Some(json!({
            "title": { "type": "json_path", "jsonPath": "$.title" },
            "company": { "type": "json_path", "jsonPath": "$.company" }
        })),
    );
    let jobs = (0..12)
        .map(|index| {
            json!({
                "id": index.to_string(),
                "company": "ACME",
                "locations": [],
                "url": format!("https://example.test/{index}")
            })
        })
        .collect::<Vec<_>>();
    let events = std::iter::once(response(
        "https://example.test/feed",
        json!({ "jobs": jobs }),
    ))
    .chain((0..12).map(|_| failed_response("https://example.test/detail")));
    let http = ScriptedProfileHttpClient::new(events);
    let browser = ScriptedBrowserAcquisition::new([]);

    let resolution = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await
        .unwrap();

    assert_eq!(resolution.counts.failed, 12);
    assert_eq!(
        resolution.candidate_diagnostics.counts_by_code["candidate_detail_execution_failed"],
        12
    );
    assert_eq!(resolution.candidate_diagnostics.samples.len(), 10);
    assert_eq!(
        resolution
            .candidate_diagnostics
            .candidate_diagnostics_omitted,
        2
    );
}

#[tokio::test]
async fn cancellation_after_an_effect_returns_no_partial_resolution() {
    let source = compiled_source(canonical_extract(), None, None);
    let http = ScriptedProfileHttpClient::new([response(
        "https://example.test/feed",
        json!({ "jobs": [{
            "id": "1",
            "title": "Engineer",
            "company": "ACME",
            "locations": [],
            "url": "https://example.test/1"
        }]}),
    )]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let result = Resolver::new(&http, &browser)
        .resolve(
            &source,
            &engineer_requirements(),
            &CancelAfterRequest(&http),
        )
        .await;

    assert_eq!(result, Err(ResolutionError::Cancelled));
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn cancellation_returns_no_partial_resolution_and_starts_no_effects() {
    let source = compiled_source(canonical_extract(), None, None);
    let http = ScriptedProfileHttpClient::new([]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let result = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &Cancelled)
        .await;

    assert_eq!(result, Err(ResolutionError::Cancelled));
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn source_engine_execution_failure_is_typed_and_keeps_diagnostics_bounded() {
    let source = compiled_source(canonical_extract(), None, None);
    let http = ScriptedProfileHttpClient::new([failed_response("https://example.test/feed")]);
    let browser = ScriptedBrowserAcquisition::new([]);

    let result = Resolver::new(&http, &browser)
        .resolve(&source, &engineer_requirements(), &NeverCancelled)
        .await;

    assert!(matches!(
        result,
        Err(ResolutionError::Failed {
            failure: ResolutionFailure::DiscoveryExecution,
            ..
        })
    ));
}
