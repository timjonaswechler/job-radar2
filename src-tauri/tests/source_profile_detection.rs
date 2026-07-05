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
        "keyCandidates": ["{{capture:boardSlug}}"],
        "nameCandidates": ["{{capture:boardSlug}}"],
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
    assert_eq!(proposal.name_candidates, vec!["acme_corp"]);
    assert_eq!(
        proposal.captures.get("boardSlug"),
        Some(&"acme_corp".to_string())
    );
    assert_eq!(proposal.support_level, SupportLevel::Experimental);

    let serialized = serde_json::to_value(proposal).unwrap();
    assert_no_adapter_key(&serialized);
}

#[test]
fn source_profile_detection_rejects_template_transform_pipes_in_detection_candidates() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{
            "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$"
        }],
        "sourceConfig": {
            "boardSlug": "{{capture:boardSlug}}"
        },
        "keyCandidates": ["{{capture:boardSlug|technicalKey}}"]
    }));

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/acme_corp",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
    assert!(result.proposal.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Detection
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "invalid_detection_template"
            && diagnostic.path == "/profiles/0/detect/keyCandidates/0"
            && diagnostic
                .message
                .contains("template transform pipes are not supported")
    }));
}

#[test]
fn source_profile_detection_normalizes_rendered_key_candidates_to_source_key_pattern() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{
            "pattern": "^https://jobs\\.example\\.test/(?<slug>[A-Za-z0-9_.-]+)$"
        }],
        "sourceConfig": {
            "slug": "{{capture:slug}}"
        },
        "keyCandidates": ["{{capture:slug}}", "jobs.{{capture:slug}}"]
    }));

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/Acme-Careers",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let proposal = result.proposal.expect("matched detection returns proposal");
    assert_eq!(
        proposal.key_candidates,
        vec!["acme_careers".to_string(), "jobs_acme_careers".to_string()]
    );
    for key_candidate in &proposal.key_candidates {
        assert_valid_source_key_candidate("fixture", key_candidate);
    }
}

#[test]
fn source_profile_detection_reports_invalid_input_url_pattern_regex_as_structured_diagnostic() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^(?<boardSlug>[a-z0-9_-]+$" }]
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
            && diagnostic.code == "invalid_input_url_pattern_regex"
            && diagnostic.path == "/profiles/0/detect/inputUrlPatterns/0/pattern"
    }));
}

#[test]
fn source_profile_detection_requires_explicit_http_check_timeout_before_fetching() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "httpChecks": [{
            "key": "metadata_endpoint",
            "url": "https://api.example.test/{{capture:boardSlug}}/metadata"
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
            && diagnostic.code == "http_check_timeout_required"
            && diagnostic.path == "/profiles/0/detect/httpChecks/0/timeoutMs"
            && diagnostic.strategy_key.as_deref() == Some("metadata_endpoint")
    }));
}

#[test]
fn source_profile_detection_requires_explicit_browser_probe_timeout_before_rendering() {
    let profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<boardSlug>[a-z0-9_-]+)$" }],
        "browserProbes": [{
            "key": "rendered_jobs_page",
            "url": "{{inputUrl}}",
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
fn source_profile_detection_uses_access_path_schema_for_default_source_config() {
    let mut profile = fixture_profile(json!({
        "recommendedAccessPathKey": "api",
        "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/(?<region>eu)$" }]
    }));
    profile.access_paths[0].source_config_schema = Some(
        json!({
            "type": "object",
            "required": ["region"],
            "properties": {
                "region": { "type": "string", "enum": ["eu"], "pattern": "^eu$" }
            },
            "additionalProperties": false
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = block_on(detect_source_proposal_with_http_client(
        "https://jobs.example.test/eu",
        &[profile],
        &FakeHttpClient::default(),
    ));

    assert_eq!(result.status, SourceProposalDetectionStatus::Matched);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.proposal.unwrap().source_config["region"], "eu");
}

#[test]
fn source_profile_detection_validates_proposed_source_config_required_forbidden_and_unknown_properties(
) {
    let cases = [
        (
            json!({ "region": "eu" }),
            "missing_source_config_required_property",
            "/profiles/0/detect/sourceConfig/boardSlug",
        ),
        (
            json!({ "boardSlug": "acme", "keyword": "engineer" }),
            "forbidden_search_criteria_in_source_config",
            "/profiles/0/detect/sourceConfig/keyword",
        ),
        (
            json!({ "boardSlug": "acme", "unexpected": "value" }),
            "unknown_source_config_property",
            "/profiles/0/detect/sourceConfig/unexpected",
        ),
    ];

    for (source_config, expected_code, expected_path) in cases {
        let mut profile = fixture_profile(json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/acme$" }],
            "sourceConfig": source_config
        }));
        profile.source_config_schema = Some(
            json!({
                "type": "object",
                "required": ["boardSlug"],
                "properties": {
                    "boardSlug": { "type": "string" }
                },
                "additionalProperties": false
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let result = block_on(detect_source_proposal_with_http_client(
            "https://jobs.example.test/acme",
            &[profile],
            &FakeHttpClient::default(),
        ));

        assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
        assert!(result.proposal.is_none());
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.category == DiagnosticCategory::Detection
                    && diagnostic.severity == DiagnosticSeverity::Error
                    && diagnostic.code == expected_code
                    && diagnostic.path == expected_path
            }),
            "missing {expected_code} at {expected_path}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn source_profile_detection_validates_proposed_source_config_types_enums_and_patterns() {
    let cases = [
        (
            json!({ "boardSlug": 42, "region": "eu" }),
            "invalid_source_config_property_type",
        ),
        (
            json!({ "boardSlug": "acme", "region": "us" }),
            "invalid_source_config_property_enum",
        ),
        (
            json!({ "boardSlug": "ACME", "region": "eu" }),
            "invalid_source_config_property_pattern",
        ),
    ];

    for (source_config, expected_code) in cases {
        let mut profile = fixture_profile(json!({
            "recommendedAccessPathKey": "api",
            "inputUrlPatterns": [{ "pattern": "^https://jobs\\.example\\.test/acme$" }],
            "sourceConfig": source_config
        }));
        profile.source_config_schema = Some(
            json!({
                "type": "object",
                "required": ["boardSlug"],
                "properties": {
                    "boardSlug": { "type": "string", "pattern": "^[a-z0-9_-]+$" }
                },
                "additionalProperties": false
            })
            .as_object()
            .unwrap()
            .clone(),
        );
        profile.access_paths[0].source_config_schema = Some(
            json!({
                "type": "object",
                "required": ["region"],
                "properties": {
                    "region": { "type": "string", "enum": ["eu"] }
                }
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let result = block_on(detect_source_proposal_with_http_client(
            "https://jobs.example.test/acme",
            &[profile],
            &FakeHttpClient::default(),
        ));

        assert_eq!(result.status, SourceProposalDetectionStatus::Failed);
        assert!(result.proposal.is_none());
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.category == DiagnosticCategory::Detection
                    && diagnostic.severity == DiagnosticSeverity::Error
                    && diagnostic.code == expected_code
            }),
            "missing {expected_code}: {:?}",
            result.diagnostics
        );
    }
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
            "timeoutMs": 10000,
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
            "https://boards.greenhouse.io/acme-careers",
            json!({ "boardSlug": "acme-careers" }),
            "acme_careers",
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
            "workday_acme_external",
        ),
        (
            "successfactors.json",
            "https://jobs.example.com/job/Berlin-Engineer-1001",
            json!({
                "baseUrl": "https://jobs.example.com",
                "sitemapUrl": "https://jobs.example.com/sitemap.xml"
            }),
            "successfactors_jobs_example_com",
        ),
    ];

    for (profile_file, input_url, expected_config, expected_key_candidate) in cases {
        let profile_text = read_builtin_profile_text(profile_file);
        assert_no_template_transform_pipes(&profile_text);
        let profile: SourceProfileDocument = serde_json::from_str(&profile_text)
            .unwrap_or_else(|error| panic!("failed to parse {profile_file}: {error}"));
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
        assert_eq!(
            proposal.key_candidates,
            vec![expected_key_candidate.to_string()]
        );
        for key_candidate in &proposal.key_candidates {
            assert_valid_source_key_candidate(profile_file, key_candidate);
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
            "timeoutMs": 10000,
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
    let contents = read_builtin_profile_text(file_name);
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {file_name}: {error}"))
}

fn read_builtin_profile_text(file_name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/profiles")
        .join(file_name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
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

fn assert_no_template_transform_pipes(profile_text: &str) {
    let mut remainder = profile_text;
    while let Some(start) = remainder.find("{{") {
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let reference = after_start[..end].trim();
        assert!(
            !reference.contains('|'),
            "built-in profiles must not hide transforms in template pipes: {{{{{reference}}}}}"
        );
        remainder = &after_start[end + 2..];
    }
}

fn assert_valid_source_key_candidate(context: &str, candidate: &str) {
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        panic!("{context} produced an empty Source key candidate");
    };
    assert!(
        first.is_ascii_lowercase() || first.is_ascii_digit(),
        "{context} Source key candidate `{candidate}` must start with [a-z0-9]"
    );
    assert!(
        chars.all(|character| character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '_'),
        "{context} Source key candidate `{candidate}` must match ^[a-z0-9][a-z0-9_]*$"
    );
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
