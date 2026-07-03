use std::{collections::HashMap, future::Future, pin::Pin};

use job_radar_lib::{
    detect_source_proposal_with_clients, detect_source_proposal_with_http_client,
    DetectionHttpClient, DetectionHttpError, DetectionHttpResponse, DiagnosticCategory,
    DiagnosticSeverity, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
    ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, SourceProfileDocument,
    SourceProposalDetectionStatus, SupportLevel,
};
use serde_json::{json, Value};

#[test]
fn source_profile_detection_returns_source_proposal_from_named_url_captures() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{
            "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$"
        }],
        "sourceConfig": {
            "boardSlug": "{{capture:boardSlug}}",
            "startUrl": "{{inputUrl}}"
        },
        "keyCandidates": ["{{capture:boardSlug|technicalKey}}"],
        "nameCandidates": ["{{capture:boardSlug|slugToTitle}}"],
        "evidence": [{
            "kind": "url",
            "message": "Example job board URL exposes a board slug."
        }]
    }));

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme_corp",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty());
    let proposal = result.proposal.expect("matched detection returns proposal");
    assert_eq!(proposal.profile_key, "example_jobs");
    assert_eq!(proposal.recommended_access_path_key, "api");
    assert_eq!(proposal.source_config["boardSlug"], "acme_corp");
    assert_eq!(
        proposal.source_config["startUrl"],
        "https://jobs.example.test/acme_corp"
    );
    assert_eq!(proposal.key_candidates, vec!["acme_corp"]);
    assert_eq!(proposal.name_candidates, vec!["Acme Corp"]);
    assert_eq!(
        proposal.captures.get("boardSlug"),
        Some(&"acme_corp".to_string())
    );
    assert_eq!(proposal.support_level, SupportLevel::Experimental);

    let serialized = serde_json::to_value(proposal).unwrap();
    assert_no_adapter_key(&serialized);
}

#[test]
fn source_profile_detection_http_checks_contribute_evidence_and_captures() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{
            "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$"
        }],
        "httpChecks": [{
            "key": "metadata_endpoint",
            "url": "https://api.example.test/{{capture:boardSlug}}/metadata",
            "expectStatus": 200,
            "contains": "ExampleJobs",
            "regex": "company=\\\"(?<organizationName>[^\\\"]+)\\\"",
            "evidence": "ExampleJobs metadata endpoint is reachable."
        }],
        "sourceConfig": { "boardSlug": "{{capture:boardSlug}}" },
        "nameCandidates": ["{{capture:organizationName}}"]
    }));
    let client = FakeHttpClient::with_response(
        "https://api.example.test/acme/metadata",
        DetectionHttpResponse {
            status: 200,
            body: "vendor=ExampleJobs company=\"ACME GmbH\"".to_string(),
        },
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme",
        &[profile],
        &client,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty());
    let proposal = result.proposal.unwrap();
    assert_eq!(
        proposal.captures.get("organizationName"),
        Some(&"ACME GmbH".to_string())
    );
    assert_eq!(proposal.name_candidates, vec!["ACME GmbH"]);
    assert!(proposal.evidence.iter().any(|evidence| {
        evidence.probe_key.as_deref() == Some("metadata_endpoint")
            && evidence.message == "ExampleJobs metadata endpoint is reachable."
    }));
}

#[test]
fn source_profile_detection_reports_ambiguous_when_multiple_profiles_match() {
    let first = fixture_profile_with_key(
        "first_jobs",
        "First Jobs",
        SupportLevel::Experimental,
        json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<slug>[a-z0-9_-]+)$" }]
        }),
    );
    let second = fixture_profile_with_key(
        "second_jobs",
        "Second Jobs",
        SupportLevel::Experimental,
        json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<slug>[a-z0-9_-]+)$" }]
        }),
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme",
        &[first, second],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Ambiguous);
    assert!(result.proposal.is_none());
    assert_eq!(result.proposals.len(), 2);
}

#[test]
fn source_profile_detection_identifies_known_unsupported_profile_without_source_proposal() {
    let profile = fixture_profile_with_key(
        "known_jobs",
        "Known Jobs",
        SupportLevel::Unsupported,
        json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://known\\.example\\.test/(?<tenant>[a-z0-9_-]+)$" }],
            "evidence": [{ "kind": "url", "message": "Known unsupported ATS URL." }]
        }),
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://known.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Unsupported);
    assert!(result.proposal.is_none());
    assert_eq!(result.unsupported_profiles.len(), 1);
    assert_eq!(result.unsupported_profiles[0].profile_key, "known_jobs");
    assert_eq!(
        result.unsupported_profiles[0].support_level,
        SupportLevel::Unsupported
    );
}

#[test]
fn builtin_acceptance_profiles_produce_source_proposals_without_adapter_key() {
    let cases = [
        (
            "greenhouse.json",
            "https://boards.greenhouse.io/acme",
            json!({ "boardSlug": "acme" }),
        ),
        (
            "workday.json",
            "https://acme.wd1.myworkdayjobs.com/External",
            json!({
                "workdayHost": "acme.wd1.myworkdayjobs.com",
                "tenant": "acme",
                "site": "External",
                "startUrl": "https://acme.wd1.myworkdayjobs.com/External"
            }),
        ),
        (
            "successfactors.json",
            "https://jobs.example.com/job/Berlin-Engineer-1001",
            json!({
                "baseUrl": "https://jobs.example.com",
                "sitemapUrl": "https://jobs.example.com/sitemap.xml"
            }),
        ),
    ];

    for (profile_file, input_url, expected_config) in cases {
        let profile = read_builtin_profile(profile_file);
        let client = if profile_file == "successfactors.json" {
            FakeHttpClient::with_response(
                "https://jobs.example.com/sitemap.xml",
                DetectionHttpResponse {
                    status: 200,
                    body: r#"<?xml version="1.0"?><urlset><url><loc>https://jobs.example.com/job/Berlin-Engineer-1001</loc></url></urlset>"#
                        .to_string(),
                },
            )
        } else {
            FakeHttpClient::default()
        };
        let result = block_on(detect_source_proposal_with_http_client(
            input_url,
            &[profile],
            &client,
        ));

        assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
        assert!(
            result.diagnostics.is_empty(),
            "{profile_file} diagnostics: {:?}",
            result.diagnostics
        );
        let proposal = result.proposal.unwrap();
        for (key, expected_value) in expected_config.as_object().unwrap() {
            assert_eq!(
                &proposal.source_config[key], expected_value,
                "{profile_file} {key}"
            );
        }
        assert_no_adapter_key(&serde_json::to_value(proposal).unwrap());
    }
}

#[test]
fn successfactors_detection_requires_sitemap_job_url_evidence_for_sitemap_inputs() {
    let profile = read_builtin_profile("successfactors.json");
    let client = FakeHttpClient::with_response(
        "https://openai.com/sitemap.xml",
        DetectionHttpResponse {
            status: 200,
            body: r#"<?xml version="1.0"?><urlset><url><loc>https://openai.com/careers</loc></url></urlset>"#
                .to_string(),
        },
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://openai.com/sitemap.xml",
        &[profile],
        &client,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Unsupported);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "http_check_regex_mismatch"
            && diagnostic.severity == DiagnosticSeverity::Warning
    }));
}

#[test]
fn successfactors_detection_accepts_sitemap_with_successfactors_job_urls() {
    let profile = read_builtin_profile("successfactors.json");
    let client = FakeHttpClient::with_response(
        "https://join.schott.com/sitemap.xml",
        DetectionHttpResponse {
            status: 200,
            body: r#"<?xml version="1.0"?><urlset><url><loc>https://join.schott.com/job/St_-Gallen-Product-Engineer-%28mwd%29-SG/1405371733/</loc></url></urlset>"#
                .to_string(),
        },
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://join.schott.com/sitemap.xml",
        &[profile],
        &client,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(
        result.diagnostics.is_empty(),
        "diagnostics: {:?}",
        result.diagnostics
    );
    let proposal = result.proposal.unwrap();
    assert_eq!(proposal.profile_key, "successfactors");
    assert_eq!(proposal.source_config["baseUrl"], "https://join.schott.com");
    assert_eq!(
        proposal.source_config["sitemapUrl"],
        "https://join.schott.com/sitemap.xml"
    );
}

#[test]
fn source_profile_detection_reports_failed_http_check_as_structured_diagnostic() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "httpChecks": [{
            "key": "metadata_endpoint",
            "url": "https://api.example.test/{{capture:boardSlug}}/metadata",
            "expectStatus": 200
        }]
    }));
    let client = FakeHttpClient::with_error(
        "https://api.example.test/acme/metadata",
        DetectionHttpError::new("connection refused"),
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme",
        &[profile],
        &client,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "http_check_failed"
            && diagnostic.path == "/profiles/0/detect/httpChecks/0/url"
            && diagnostic.strategy_key.as_deref() == Some("metadata_endpoint")
    }));
}

#[test]
fn source_profile_detection_browser_probe_contributes_evidence_captures_and_bounded_request() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}?board={{capture:boardSlug}}",
            "timeoutMs": 10000,
            "waits": [
                { "type": "selector", "selector": ".jobs", "timeoutMs": 5000 },
                { "type": "network_idle", "timeoutMs": 250 }
            ],
            "interactions": [{
                "type": "click_if_visible",
                "selector": "button.load-more",
                "maxCount": 2,
                "waitAfterMs": 100
            }],
            "htmlContains": "ExampleJobs",
            "htmlRegex": "data-org=\\\"(?<organizationName>[^\\\"]+)\\\"",
            "evidence": "Rendered jobs page identifies ExampleJobs."
        }],
        "sourceConfig": { "boardSlug": "{{capture:boardSlug}}" },
        "nameCandidates": ["{{capture:organizationName}}"]
    }));
    let browser = FakeBrowser::new([(
        "https://jobs.example.test/acme?board=acme",
        "<main class=\"jobs\" data-org=\"ACME GmbH\">ExampleJobs</main>".to_string(),
    )]);

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let proposal = result.proposal.unwrap();
    assert_eq!(
        proposal.captures.get("organizationName"),
        Some(&"ACME GmbH".to_string())
    );
    assert_eq!(proposal.name_candidates, vec!["ACME GmbH"]);
    assert!(proposal.evidence.iter().any(|evidence| {
        let serialized = serde_json::to_value(evidence).unwrap();
        serialized["kind"] == "browser"
            && evidence.probe_key.as_deref() == Some("rendered_jobs_page")
            && evidence.message == "Rendered jobs page identifies ExampleJobs."
    }));

    let requests = browser.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, "https://jobs.example.test/acme?board=acme");
    assert_eq!(requests[0].timeout_ms, 10000);
    assert_eq!(
        requests[0].waits,
        vec![
            ExecutionPlanBrowserWait::Selector {
                selector: Some(".jobs".to_string()),
                timeout_ms: 5000,
            },
            ExecutionPlanBrowserWait::NetworkIdle {
                selector: None,
                timeout_ms: 250,
            },
        ]
    );
    assert_eq!(
        requests[0].interactions,
        vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: "button.load-more".to_string(),
            max_count: 2,
            wait_after_ms: Some(100),
        }]
    );
}

#[test]
fn source_profile_detection_browser_probe_url_can_use_proposed_source_config() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^(?<baseUrl>https://jobs\\.example\\.test)/(?<boardSlug>[a-z0-9_-]+)$" }],
        "sourceConfig": {
            "baseUrl": "{{capture:baseUrl}}",
            "boardSlug": "{{capture:boardSlug}}"
        },
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{sourceConfig:baseUrl}}/rendered/{{sourceConfig:boardSlug}}",
            "timeoutMs": 10000,
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::new([(
        "https://jobs.example.test/rendered/acme",
        "<main>ExampleJobs</main>".to_string(),
    )]);

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        browser.requests()[0].url,
        "https://jobs.example.test/rendered/acme"
    );
}

#[test]
fn source_profile_detection_reports_browser_wait_timeout_as_structured_diagnostic() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 10000,
            "waits": [{ "type": "selector", "selector": ".jobs", "timeoutMs": 5000 }],
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::WaitTimeout {
            wait_index: Some(0),
        },
        "selector .jobs did not appear",
    ));

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "browser_wait_timeout"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0/waits/0"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[test]
fn source_profile_detection_maps_browser_runtime_failures_to_stable_diagnostics() {
    let cases = [
        (
            ProfileBrowserFetchErrorKind::RuntimeUnavailable,
            "browser runtime is not installed",
            "browser_runtime_unavailable",
            "/profiles/0/detect/browserProbes/0",
        ),
        (
            ProfileBrowserFetchErrorKind::NavigationFailed,
            "navigation failed",
            "browser_navigation_failed",
            "/profiles/0/detect/browserProbes/0/url",
        ),
        (
            ProfileBrowserFetchErrorKind::InteractionFailed {
                interaction_index: Some(0),
            },
            "click failed",
            "browser_interaction_failed",
            "/profiles/0/detect/browserProbes/0/interactions/0",
        ),
        (
            ProfileBrowserFetchErrorKind::RenderTimeout,
            "render timed out",
            "browser_render_timeout",
            "/profiles/0/detect/browserProbes/0/timeoutMs",
        ),
        (
            ProfileBrowserFetchErrorKind::ContentReadFailed,
            "content could not be read",
            "browser_content_read_failed",
            "/profiles/0/detect/browserProbes/0",
        ),
    ];

    for (kind, message, expected_code, expected_path) in cases {
        let profile = fixture_profile(json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
            "browserProbes": [{
                "key": "rendered_jobs_page",
                "url": "{{inputUrl}}",
                "timeoutMs": 10000,
                "interactions": [{
                    "type": "click_if_visible",
                    "selector": "button.load-more",
                    "maxCount": 1
                }],
                "htmlContains": "ExampleJobs"
            }]
        }));
        let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(kind, message));

        let result = block_on(detect_source_proposal_with_clients(
            "https://jobs.example.test/acme",
            &[profile],
            &FakeHttpClient::default(),
            &browser,
        ));

        assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
        assert!(result.proposal.is_none());
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.category == DiagnosticCategory::Detection
                    && diagnostic.severity == DiagnosticSeverity::Error
                    && diagnostic.code == expected_code
                    && diagnostic.path == expected_path
                    && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
            }),
            "missing {expected_code} at {expected_path}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn source_profile_detection_reports_browser_non_match_without_proposal() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 10000,
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::new([(
        "https://jobs.example.test/acme",
        "<main>Different renderer</main>".to_string(),
    )]);

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Unsupported);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.code == "browser_probe_html_contains_mismatch"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0/htmlContains"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[test]
fn source_profile_detection_rejects_unbounded_browser_probe_render_timeout() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 0,
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::new(std::iter::empty());

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(browser.requests().is_empty());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "browser_probe_timeout_required"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0/timeoutMs"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[test]
fn source_profile_detection_rejects_unbounded_browser_probe_waits_and_interactions() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 10000,
            "waits": [{ "type": "selector", "selector": ".jobs" }],
            "interactions": [{ "type": "click_until_gone", "selector": "button.load-more" }],
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::new(std::iter::empty());

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(browser.requests().is_empty());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "browser_wait_timeout_required"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0/waits/0/timeoutMs"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[test]
fn source_profile_detection_rejects_unbounded_browser_probe_interactions_before_rendering() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 10000,
            "interactions": [{ "type": "click_until_gone", "selector": "button.load-more" }],
            "htmlContains": "ExampleJobs"
        }]
    }));
    let browser = FakeBrowser::new(std::iter::empty());

    let result = block_on(detect_source_proposal_with_clients(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
        &browser,
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(browser.requests().is_empty());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "browser_interaction_max_count_required"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0/interactions/0/maxCount"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[test]
fn source_profile_detection_reports_browser_probe_required_when_executor_is_unavailable() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
            "timeoutMs": 10000,
            "waits": [{ "type": "selector", "selector": ".jobs", "timeoutMs": 5000 }],
            "htmlContains": "ExampleJobs"
        }]
    }));

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "browser_probe_executor_unavailable"
            && diagnostic.path == "/profiles/0/detect/browserProbes/0"
            && diagnostic.strategy_key.as_deref() == Some("rendered_jobs_page")
    }));
}

#[derive(Default)]
struct FakeHttpClient {
    responses: HashMap<String, Result<DetectionHttpResponse, DetectionHttpError>>,
}

impl FakeHttpClient {
    fn with_response(url: &str, response: DetectionHttpResponse) -> Self {
        Self {
            responses: HashMap::from([(url.to_string(), Ok(response))]),
        }
    }

    fn with_error(url: &str, error: DetectionHttpError) -> Self {
        Self {
            responses: HashMap::from([(url.to_string(), Err(error))]),
        }
    }
}

struct FakeBrowser {
    responses: HashMap<String, Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>,
    requests: std::sync::Mutex<Vec<ProfileBrowserFetchRequest>>,
}

impl FakeBrowser {
    fn new(responses: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), Ok(ProfileBrowserFetchResponse { body })))
                .collect(),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn failing(error: ProfileBrowserFetchError) -> Self {
        Self {
            responses: HashMap::from([("*".to_string(), Err(error))]),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<ProfileBrowserFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ProfileBrowserClient for FakeBrowser {
    fn render<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            self.responses
                .get(&request.url)
                .or_else(|| self.responses.get("*"))
                .cloned()
                .unwrap_or_else(|| {
                    Err(ProfileBrowserFetchError::new(
                        ProfileBrowserFetchErrorKind::NavigationFailed,
                        format!("missing fake browser response for {}", request.url),
                    ))
                })
        })
    }
}

impl DetectionHttpClient for FakeHttpClient {
    fn get_text<'a>(
        &'a self,
        url: String,
        _timeout_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<DetectionHttpResponse, DetectionHttpError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.responses.get(&url).cloned().unwrap_or_else(|| {
                Err(DetectionHttpError::new(format!(
                    "missing fixture for {url}"
                )))
            })
        })
    }
}

fn read_builtin_profile(file_name: &str) -> SourceProfileDocument {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/profiles")
        .join(file_name);
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn fixture_profile(detect: Value) -> SourceProfileDocument {
    fixture_profile_with_key(
        "example_jobs",
        "Example Jobs",
        SupportLevel::Experimental,
        detect,
    )
}

fn fixture_profile_with_key(
    key: &str,
    name: &str,
    support_level: SupportLevel,
    detect: Value,
) -> SourceProfileDocument {
    let support_level = serde_json::to_value(support_level)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": key,
        "name": name,
        "kind": "generic",
        "support": { "level": support_level },
        "detect": detect,
        "sourceConfigSchema": {
            "type": "object",
            "properties": {
                "boardSlug": { "type": "string" },
                "slug": { "type": "string" },
                "tenant": { "type": "string" },
                "startUrl": { "type": "string" }
            }
        },
        "accessPaths": [{
            "key": "api",
            "name": "API",
            "postingDiscovery": {
                "strategies": [{
                    "key": "jobs_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "https://example.test/jobs",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "fields": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "const", "value": "Example" },
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" }
                        }
                    }
                }]
            }
        }]
    }))
    .unwrap()
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn assert_no_adapter_key(value: &Value) {
    match value {
        Value::Object(map) => {
            assert!(
                !map.contains_key("adapterKey"),
                "serialized value contains adapterKey: {value}"
            );
            for nested in map.values() {
                assert_no_adapter_key(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_no_adapter_key(nested);
            }
        }
        _ => {}
    }
}
