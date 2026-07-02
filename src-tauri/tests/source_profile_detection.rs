use std::{collections::HashMap, future::Future, pin::Pin};

use job_radar_lib::{
    detect_source_proposal_with_http_client, DetectionHttpClient, DetectionHttpError,
    DetectionHttpResponse, DiagnosticCategory, DiagnosticSeverity, SourceProfileDocument,
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
        let result = block_on(detect_source_proposal_with_http_client(
            input_url,
            &[profile],
            &FakeHttpClient::default(),
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
