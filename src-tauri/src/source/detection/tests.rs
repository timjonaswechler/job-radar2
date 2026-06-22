use super::*;
use crate::source::registry::{
    RegistrySourceProfile, SourceProfileDocument, SourceRegistryDocumentOrigin,
};
use serde_json::json;
use std::sync::Mutex;

struct FixtureHttpClient {
    responses: HashMap<String, String>,
    requests: Mutex<Vec<String>>,
}

impl FixtureHttpClient {
    fn new(responses: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body.to_string()))
                .collect(),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl DetectionHttpClient for FixtureHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(url.as_str().to_string());
            self.responses
                .get(url.as_str())
                .cloned()
                .ok_or_else(|| format!("{} not found", url.as_str()))
        })
    }
}

#[test]
fn source_profile_detection_does_not_recommend_access_path_without_required_capture() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><main id="example-board-root"></main></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "example-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": {
                        "tenant": "{{capture:tenant}}",
                        "startUrl": "{{inputUrl}}"
                    }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn source_profile_detection_recommends_path_after_required_capture_and_availability_checks_pass() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://example.com/jobs",
                r#"<html><body><script>window.tenant = "acme";</script></body></html>"#,
            ),
            (
                "https://example.com/api/status.json",
                r#"{"jobs":[{"title":"Engineer"}]}"#,
            ),
        ]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{
                    "htmlRegex": "tenant\\s*=\\s*\"([^\"]+)\"",
                    "captureAs": "tenant"
                }]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "checks": [{
                        "fetchJson": {
                            "url": "/api/status.json",
                            "pathExists": "$.jobs"
                        }
                    }],
                    "sourceConfig": {
                        "tenant": "{{capture:tenant}}",
                        "startUrl": "{{origin}}/api/jobs/{{capture:tenant}}"
                    }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.profile_key.as_deref(), Some("example_board"));
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(result.key.as_deref(), Some("acme"));
        assert_eq!(result.name.as_deref(), Some("Acme"));
        assert_eq!(result.key_candidates, vec!["acme"]);
        assert_eq!(result.name_candidates, vec!["Acme"]);
        assert_eq!(result.matches[0].key_candidates, vec!["acme"]);
        assert_eq!(result.matches[0].name_candidates, vec!["Acme"]);
        let source_config = result.source_config.unwrap();
        assert_eq!(source_config["tenant"], "acme");
        assert_eq!(
            source_config["startUrl"],
            "https://example.com/api/jobs/acme"
        );
        assert!(result
            .evidence
            .join("\n")
            .contains("https://example.com/api/status.json"));
    });
}

#[test]
fn source_profile_detection_uses_url_and_endpoint_fast_path_without_fetching_submitted_page() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://acme.example-board.test/xml?language=en",
            r#"<?xml version="1.0" encoding="UTF-8"?><example-jobs></example-jobs>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [],
                "anyOf": [[
                    {
                        "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                        "captureAs": "tenant"
                    },
                    {
                        "fetchText": {
                            "url": "/xml?language=en",
                            "contains": "<example-jobs"
                        }
                    }
                ]]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": { "tenant": "{{capture:tenant}}" }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://acme.example-board.test/").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("acme"));
        assert_eq!(result.source_config.unwrap()["tenant"], "acme");
        assert!(result.warnings.is_empty());
        assert_eq!(
            client.requested_urls(),
            vec!["https://acme.example-board.test/xml?language=en"]
        );
    });
}

#[test]
fn source_profile_detection_uses_url_and_json_endpoint_fast_path_without_fetching_submitted_page() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://acme.example-board.test/status.json",
            r#"{"jobs":[{"title":"Engineer"}]}"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [],
                "anyOf": [[
                    {
                        "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                        "captureAs": "tenant"
                    },
                    {
                        "fetchJson": {
                            "url": "/status.json",
                            "pathExists": "$.jobs"
                        }
                    }
                ]]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": { "tenant": "{{capture:tenant}}" }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://acme.example-board.test/").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("acme"));
        assert_eq!(result.source_config.unwrap()["tenant"], "acme");
        assert!(result.warnings.is_empty());
        assert_eq!(
            client.requested_urls(),
            vec!["https://acme.example-board.test/status.json"]
        );
    });
}

#[test]
fn source_profile_detection_can_capture_from_input_url() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://acme.example-board.test/jobs",
            r#"<html><body><main>Jobs</main></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{
                    "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                    "captureAs": "tenant"
                }]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": { "tenant": "{{capture:tenant}}" }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://acme.example-board.test/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("acme"));
        assert_eq!(result.name.as_deref(), Some("Acme"));
        assert_eq!(result.source_config.unwrap()["tenant"], "acme");
        assert!(result.evidence.join("\n").contains("Eingabe-URL"));
    });
}

#[test]
fn source_profile_detection_uses_required_url_and_endpoint_checks_without_fetching_submitted_page()
{
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://acme.example-board.test/xml?language=en",
            r#"<?xml version="1.0" encoding="UTF-8"?><example-jobs></example-jobs>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [
                    {
                        "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                        "captureAs": "tenant"
                    },
                    {
                        "fetchText": {
                            "url": "/xml?language=en",
                            "contains": "<example-jobs"
                        }
                    }
                ]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": { "tenant": "{{capture:tenant}}" }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://acme.example-board.test/").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("acme"));
        assert_eq!(result.source_config.unwrap()["tenant"], "acme");
        assert!(result.warnings.is_empty());
        assert_eq!(
            client.requested_urls(),
            vec!["https://acme.example-board.test/xml?language=en"]
        );
    });
}

#[test]
fn source_profile_detection_any_of_first_matching_alternative_wins() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body>
                    <main id="example-board-root"></main>
                    <script>
                      window.firstTenant = "alpha";
                      window.secondTenant = "bravo";
                    </script>
                    <span>first-alt-ready</span>
                </body></html>"#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[any_of_profile()],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(result.key.as_deref(), Some("alpha"));
        assert_eq!(result.name.as_deref(), Some("Alpha"));
        assert_eq!(result.source_config.unwrap()["tenant"], "alpha");
    });
}

#[test]
fn source_profile_detection_any_of_later_alternative_can_match() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body>
                    <main id="example-board-root"></main>
                    <script>
                      window.firstTenant = "alpha";
                      window.secondTenant = "bravo";
                    </script>
                </body></html>"#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[any_of_profile()],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("bravo"));
        assert_eq!(result.name.as_deref(), Some("Bravo"));
        assert_eq!(result.source_config.unwrap()["tenant"], "bravo");
    });
}

#[test]
fn source_profile_detection_any_of_without_matching_alternative_is_unsupported() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><main id="example-board-root"></main></body></html>"#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[any_of_profile()],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn source_profile_detection_any_of_does_not_bypass_required_checks() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><script>window.firstTenant = "alpha";</script></body></html>"#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[any_of_profile()],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn source_profile_detection_captures_first_non_empty_regex_group() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><a href="https://second.example/bravo">Jobs</a></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{
                    "htmlRegex": "https://(?:first\\.example/([a-z]+)|second\\.example/([a-z]+))",
                    "captureAs": "tenant"
                }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": {
                        "tenant": "{{capture:tenant}}"
                    }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.source_config.unwrap()["tenant"], "bravo");
    });
}

#[test]
fn html_dependent_detection_fetches_submitted_page_during_fallback() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://html-board.example/jobs",
            r#"<html><body><main id="html-board-root">Jobs</main></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "html_board",
            "name": "HTML Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "html-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://html-board.example/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.profile_key.as_deref(), Some("html_board"));
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(
            client.requested_urls(),
            vec!["https://html-board.example/jobs"]
        );
    });
}

#[test]
fn source_profile_detection_does_not_recommend_path_when_required_schema_config_is_missing() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><main id="example-board-root"></main></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "example-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                },
                "sourceConfigSchema": {
                    "type": "object",
                    "required": ["startUrl", "apiBaseUrl"],
                    "properties": {
                        "startUrl": { "type": "string" },
                        "apiBaseUrl": { "type": "string" }
                    }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn source_profile_detection_does_not_recommend_path_when_availability_check_fails() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://example.com/jobs",
                r#"<html><body><main id="example-board-root"></main></body></html>"#,
            ),
            ("https://example.com/health.txt", "different token"),
        ]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "example-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "checks": [{
                        "fetchText": {
                            "url": "/health.txt",
                            "contains": "requiredApiToken"
                        }
                    }],
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn detection_template_context_uses_shared_renderer_and_filters() {
    let input_url = Url::parse("https://jobs.ashbyhq.com/focused").unwrap();
    let captures = HashMap::from([
        ("boardSlug".to_string(), "focused-energy".to_string()),
        (
            "companyWebsite".to_string(),
            "https://focused-energy.co".to_string(),
        ),
    ]);
    let context = DetectionTemplateContext {
        input_url: &input_url,
        captures: &captures,
    };

    let rendered = render_template(
        "{{origin}}|{{capture:companyWebsite|domainKey}}|{{capture:boardSlug|titleCase}}",
        &context,
    )
    .unwrap();

    assert_eq!(
        rendered,
        "https://jobs.ashbyhq.com|focused_energy|Focused Energy"
    );
}

#[test]
fn detects_greenhouse_ashby_and_lever_with_profile_path_and_creatable_config() {
    tauri::async_runtime::block_on(async {
        let scenarios = [
            (
                builtin_profile("greenhouse"),
                "https://openai.com/careers",
                r#"
                    <html>
                      <body>
                        <h1>OpenAI Careers</h1>
                        <script src="https://boards.greenhouse.io/embed/job_board/js?for=openai"></script>
                        <a href="https://boards.greenhouse.io/openai">Job board</a>
                      </body>
                    </html>
                    "#,
                "greenhouse",
                "endpoint_inventory",
                "declarative_endpoint_inventory",
                "\\.greenhouse\\.io",
                "boardSlug",
                "openai",
            ),
            (
                builtin_profile("ashby"),
                "https://ashby-fixture.test/careers",
                r#"
                    <html>
                      <body>
                        <h1>Example Careers</h1>
                        <iframe src="https://jobs.ashbyhq.com/example"></iframe>
                      </body>
                    </html>
                    "#,
                "ashby",
                "endpoint_inventory",
                "declarative_endpoint_inventory",
                "\\.ashbyhq\\.com",
                "boardSlug",
                "example",
            ),
            (
                builtin_profile("lever"),
                "https://lever-fixture.test/jobs",
                r#"
                    <html>
                      <body>
                        <h1>Example Careers</h1>
                        <a href="https://jobs.lever.co/example/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">
                          Senior Rust Engineer
                        </a>
                      </body>
                    </html>
                    "#,
                "lever",
                "endpoint_inventory",
                "declarative_endpoint_inventory",
                "jobs\\.lever\\.co",
                "boardSlug",
                "example",
            ),
        ];

        for (
            profile,
            input_url,
            html,
            expected_profile_key,
            expected_path_key,
            expected_adapter_key,
            expected_evidence_marker,
            expected_source_config_key,
            expected_source_config_value,
        ) in scenarios
        {
            let client = FixtureHttpClient::new([(input_url, html)]);

            let result =
                detect_with_source_profiles(&client, &Url::parse(input_url).unwrap(), &[profile])
                    .await
                    .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some(expected_profile_key));
            assert_eq!(result.path_key.as_deref(), Some(expected_path_key));
            assert_eq!(result.adapter_key.as_deref(), Some(expected_adapter_key));
            assert!(result
                .evidence
                .join("\n")
                .contains(expected_evidence_marker));
            let source_config = result.source_config.unwrap();
            assert_eq!(
                source_config[expected_source_config_key],
                expected_source_config_value
            );
        }
    });
}

#[test]
fn lever_global_urls_detect_global_access_path_and_api() {
    tauri::async_runtime::block_on(async {
        let profile = builtin_profile("lever");
        let cases = [
            (
                "https://lever-fixture.test/jobs",
                r#"<a href="https://jobs.lever.co/acme/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">Senior Rust Engineer</a>"#,
                "acme",
            ),
            (
                "https://lever-fixture.test/api-link",
                r#"<a href="https://api.lever.co/v0/postings/acme?mode=json">Lever postings API</a>"#,
                "acme",
            ),
        ];

        for (input_url, html, expected_board_slug) in cases {
            let client = FixtureHttpClient::new([(input_url, html)]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse(input_url).unwrap(),
                &[profile.clone()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("lever"));
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], expected_board_slug);
            assert_eq!(
                access_path_inventory_fetch_url(&profile, "endpoint_inventory"),
                "https://api.lever.co/v0/postings/{{sourceConfig:boardSlug}}?mode=json"
            );
        }
    });
}

#[test]
fn lever_eu_urls_detect_eu_access_path_and_api() {
    tauri::async_runtime::block_on(async {
        let profile = builtin_profile("lever");
        let cases = [
            (
                "https://lever-fixture.test/eu-jobs",
                r#"<a href="https://jobs.eu.lever.co/acme-eu/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">Senior Rust Engineer</a>"#,
                "acme-eu",
            ),
            (
                "https://lever-fixture.test/eu-api-link",
                r#"<a href="https://api.eu.lever.co/v0/postings/acme-eu?mode=json">Lever EU postings API</a>"#,
                "acme-eu",
            ),
        ];

        for (input_url, html, expected_board_slug) in cases {
            let client = FixtureHttpClient::new([(input_url, html)]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse(input_url).unwrap(),
                &[profile.clone()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("lever"));
            assert_eq!(result.path_key.as_deref(), Some("eu_endpoint_inventory"));
            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], expected_board_slug);
            assert_eq!(
                access_path_inventory_fetch_url(&profile, "eu_endpoint_inventory"),
                "https://api.eu.lever.co/v0/postings/{{sourceConfig:boardSlug}}?mode=json"
            );
        }
    });
}

#[test]
fn ashby_identity_uses_board_slug_candidates_when_profile_captures_it() {
    tauri::async_runtime::block_on(async {
        let input_url = "https://jobs.ashbyhq.com/focused";
        let client = FixtureHttpClient::new([(
            input_url,
            r#"
                <html>
                  <head>
                    <meta property="og:url" content="https://jobs.ashbyhq.com/focused" />
                  </head>
                  <body>
                    <script>
                      window.__appData = {"organization":{"name":"Focused","publicWebsite":"https://focused-energy.co","hostedJobsPageSlug":"focused"}};
                    </script>
                  </body>
                </html>
                "#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse(input_url).unwrap(),
            &[builtin_profile("ashby")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.key.as_deref(), Some("focused"));
        assert_eq!(result.name.as_deref(), Some("Focused"));
        let source_config = result.source_config.unwrap();
        assert_eq!(source_config["boardSlug"], "focused");
        assert!(source_config.get("startUrl").is_none());
        assert!(source_config.get("companyWebsite").is_none());
    });
}

#[test]
fn greenhouse_ashby_and_lever_ignore_generic_vendor_mentions_without_board_or_api_evidence() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://example.com/greenhouse-mention",
                r#"
                    <html>
                      <body>
                        <p>We use a vendor listed at https://www.greenhouse.io/.</p>
                      </body>
                    </html>
                    "#,
            ),
            (
                "https://example.com/ashby-mention",
                r#"
                    <html>
                      <body>
                        <p>Read about recruiting tools at https://www.ashbyhq.com/.</p>
                      </body>
                    </html>
                    "#,
            ),
            (
                "https://example.com/lever-mention",
                r#"
                    <html>
                      <body>
                        <p>Our old provider lived under https://jobs.lever.co/.</p>
                      </body>
                    </html>
                    "#,
            ),
        ]);
        let profiles = vec![
            builtin_profile("greenhouse"),
            builtin_profile("ashby"),
            builtin_profile("lever"),
        ];

        for input_url in [
            "https://example.com/greenhouse-mention",
            "https://example.com/ashby-mention",
            "https://example.com/lever-mention",
        ] {
            let result =
                detect_with_source_profiles(&client, &Url::parse(input_url).unwrap(), &profiles)
                    .await
                    .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
        }
    });
}

#[test]
fn greenhouse_ashby_and_lever_do_not_detect_company_domain_only_pages() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://openai.com/careers",
                r#"<html><body><h1>OpenAI Careers</h1><p>Come build with us.</p></body></html>"#,
            ),
            (
                "https://helsing.ai/careers",
                r#"<html><body><h1>Helsing Careers</h1><p>Open roles.</p></body></html>"#,
            ),
        ]);
        let profiles = vec![
            builtin_profile("greenhouse"),
            builtin_profile("ashby"),
            builtin_profile("lever"),
        ];

        for input_url in ["https://openai.com/careers", "https://helsing.ai/careers"] {
            let result =
                detect_with_source_profiles(&client, &Url::parse(input_url).unwrap(), &profiles)
                    .await
                    .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
        }
    });
}

#[test]
fn detects_personio_hosted_page_with_xml_feed_config_without_fetching_submitted_page() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://demo.jobs.personio.de/xml?language=en",
            r#"<?xml version="1.0" encoding="UTF-8"?><workzag-jobs></workzag-jobs>"#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://demo.jobs.personio.de/").unwrap(),
            &[builtin_profile("personio")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.profile_key.as_deref(), Some("personio"));
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(
            result.adapter_key.as_deref(),
            Some("declarative_endpoint_inventory")
        );
        assert!(result.evidence.join("\n").contains("workzag-jobs"));

        let source_config = result.source_config.unwrap();
        assert_eq!(source_config["boardSlug"], "demo");
        assert_eq!(source_config["personioHost"], "demo.jobs.personio.de");
        assert_eq!(source_config["language"], "en");
        assert_eq!(source_config["startUrl"], "https://demo.jobs.personio.de/");
        assert!(result.warnings.is_empty());
        assert_eq!(
            client.requested_urls(),
            vec!["https://demo.jobs.personio.de/xml?language=en"]
        );
    });
}

#[test]
fn html_dependent_detection_keeps_initial_fetch_failure_as_non_fatal_warning() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "html_board",
            "name": "HTML Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "html-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://html-board.example/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
        assert_eq!(
            client.requested_urls(),
            vec!["https://html-board.example/jobs"]
        );
        let warnings = result.warnings.join("\n");
        assert!(warnings.contains("https://html-board.example/jobs"));
        assert!(warnings.contains("not found"));
    });
}

#[test]
fn detects_personio_linked_board_without_matching_generic_mentions() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://example.com/careers",
                r#"<html><body><a href="https://demo.jobs.personio.com/">Open roles</a></body></html>"#,
            ),
            (
                "https://example.com/personio-mention",
                r#"<html><body><p>We use Personio in HR.</p></body></html>"#,
            ),
        ]);
        let profile = builtin_profile("personio");

        let detected = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/careers").unwrap(),
            &[profile.clone()],
        )
        .await
        .unwrap();

        assert_eq!(detected.status, SourceDetectionStatus::Detected);
        let source_config = detected.source_config.unwrap();
        assert_eq!(source_config["boardSlug"], "demo");
        assert_eq!(source_config["personioHost"], "demo.jobs.personio.com");

        let unsupported = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/personio-mention").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(unsupported.status, SourceDetectionStatus::Unsupported);
        assert!(unsupported.matches.is_empty());
    });
}

#[test]
fn detects_successfactors_with_sap_rmk_html_and_sitemap_evidence() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://careers.example.com/search/",
                r#"
                    <html>
                      <head>
                        <meta name="generator" content="SAP SuccessFactors Recruiting Marketing">
                        <script src="/platform/js/sap-rmk-careersite.js"></script>
                      </head>
                      <body>
                        <div id="rmk-career-site">Aktuelle Stellen</div>
                      </body>
                    </html>
                    "#,
            ),
            (
                "https://careers.example.com/sitemap.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                      <url>
                        <loc>https://careers.example.com/job/Berlin-Senior-Rust-Engineer-12345/</loc>
                      </url>
                    </urlset>"#,
            ),
        ]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://careers.example.com/search/").unwrap(),
            &[builtin_profile("successfactors")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.profile_key.as_deref(), Some("successfactors"));
        assert_eq!(result.path_key.as_deref(), Some("sitemap_inventory"));
        assert_eq!(
            result.adapter_key.as_deref(),
            Some("declarative_sitemap_inventory")
        );
        let evidence = result.evidence.join("\n");
        assert!(evidence.contains("HTML erfüllt Regex"));
        assert!(evidence.contains("SuccessFactors"));
        assert!(evidence.contains("https://careers.example.com/sitemap.xml"));
        assert!(evidence.contains("<urlset"));

        let source_config = result.source_config.unwrap();
        assert_eq!(
            source_config["url"],
            "https://careers.example.com/sitemap.xml"
        );
        assert_eq!(source_config["recursive"], false);
    });
}

#[test]
fn successfactors_detection_fails_for_matching_hostname_without_technical_evidence() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://successfactors.example.com/jobs",
                r#"
                    <html>
                      <body>
                        <h1>Careers</h1>
                        <p>Current openings from our recruiting team.</p>
                      </body>
                    </html>
                    "#,
            ),
            (
                "https://successfactors.example.com/sitemap.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                      <url>
                        <loc>https://successfactors.example.com/job/Generic-Role-1/</loc>
                      </url>
                    </urlset>"#,
            ),
        ]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://successfactors.example.com/jobs").unwrap(),
            &[builtin_profile("successfactors")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
        assert!(result.evidence.is_empty());
    });
}

#[test]
fn detects_muz_with_source_profile_and_access_path_config() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://jobs.commerzbank.com/index.php?ac=search_result",
                r#"
                    <html>
                      <body>
                        <form class="jobboard-container js-job-search-form" method="get">
                          <div class="jobboard-datatable jobboard-widget"
                               data-widget="jobboardDatatable"
                               data-widget-config="configWidgetDataTable"></div>
                        </form>
                        <script src="/script/gjb_scripts.js"></script>
                      </body>
                    </html>
                    "#,
            ),
            (
                "https://jobs.commerzbank.com/script/gjb_scripts.js",
                r#"
                    var gjb_apiTokenPayload = "";
                    var gjbAddress = "https://api-jobs.commerzbank.com/";
                    "#,
            ),
            (
                "https://jobs.commerzbank.com/assets/js/jobboard.config.json",
                r#"{"configWidgetContainer":{"search":{"endpoint":"placeholder"}}}"#,
            ),
        ]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://jobs.commerzbank.com/index.php?ac=search_result").unwrap(),
            &[builtin_profile("muz_global_jobboard")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(result.profile_key.as_deref(), Some("muz_global_jobboard"));
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(
            result.adapter_key.as_deref(),
            Some("declarative_endpoint_inventory")
        );
        let evidence = result.evidence.join("\n");
        assert!(evidence.contains("HTML"));
        assert!(evidence.contains("Script"));
        assert!(evidence.contains("JSON-Pfad"));

        let source_config = result.source_config.unwrap();
        assert_eq!(
            source_config["startUrl"],
            "https://jobs.commerzbank.com/index.php?ac=search_result"
        );
        assert_eq!(
            source_config["apiBaseUrl"],
            "https://api-jobs.commerzbank.com/"
        );
        assert_eq!(
            source_config["configUrl"],
            "https://jobs.commerzbank.com/assets/js/jobboard.config.json"
        );
    });
}

#[test]
fn muz_detection_fails_for_generic_jobs_page_without_technical_evidence() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://jobs.commerzbank.com/generic-careers",
            r#"
                <html>
                  <body>
                    <h1>Jobs und Karriere</h1>
                    <p>Unsere aktuellen Stellenangebote.</p>
                  </body>
                </html>
                "#,
        )]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://jobs.commerzbank.com/generic-careers").unwrap(),
            &[builtin_profile("muz_global_jobboard")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
        assert!(result.evidence.is_empty());
    });
}

#[test]
fn detects_magnolia_esmp_job_search_through_script_and_json_endpoint() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([
            (
                "https://www.ruv.de/karriere/jobsuche?reqPlace=&reqUmkreis=&jobSearchText=",
                r#"<script type="module" src="/.resources/ruv-magnolia-presse/webresources/js/script.js"></script>"#,
            ),
            (
                "https://www.ruv.de/.resources/ruv-magnolia-presse/webresources/js/script.js",
                r#"fetch(window.location.origin+"/.search?index=job",{method:"GET"})"#,
            ),
            (
                "https://www.ruv.de/.search?index=job&size=1&page=1",
                r#"{"searchResults":[{"title":"Software Engineer","url":"/karriere/stellenanzeigen/ref1"}],"total":1}"#,
            ),
        ]);

        let result = detect_with_source_profiles(
            &client,
            &Url::parse(
                "https://www.ruv.de/karriere/jobsuche?reqPlace=&reqUmkreis=&jobSearchText=",
            )
            .unwrap(),
            &[builtin_profile("magnolia_esmp_job_search")],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Detected);
        assert_eq!(
            result.profile_key.as_deref(),
            Some("magnolia_esmp_job_search")
        );
        assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
        assert_eq!(
            result.source_config.unwrap()["endpointUrl"],
            "https://www.ruv.de/.search?index=job"
        );
    });
}

#[test]
fn unsupported_when_required_evidence_is_missing() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body>No known source profile</body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "example",
            "name": "Example",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "jobboard-widget" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        assert!(result.matches.is_empty());
    });
}

#[test]
fn ambiguous_detection_reports_source_profile_and_path_terms() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><main id="shared-board-root"></main></body></html>"#,
        )]);
        let first = matching_profile("first_profile");
        let second = matching_profile("second_profile");

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[first, second],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Ambiguous);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].profile_key, "first_profile");
        assert_eq!(result.matches[0].path_key, "endpoint_inventory");
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("profileKey"));
        assert!(serialized.contains("pathKey"));
        assert!(!serialized.contains("systemProfile"));
    });
}

#[test]
fn source_profiles_without_detection_blocks_are_not_global_detection_candidates() {
    tauri::async_runtime::block_on(async {
        let client = FixtureHttpClient::new([(
            "https://example.com/jobs",
            r#"<html><body><main>Any page</main></body></html>"#,
        )]);
        let profile = registry_profile(json!({
            "schemaVersion": 1,
            "key": "manual_only",
            "name": "Manual Only",
            "kind": "generic",
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory"
            }]
        }));

        let result = detect_with_source_profiles(
            &client,
            &Url::parse("https://example.com/jobs").unwrap(),
            &[profile],
        )
        .await
        .unwrap();

        assert_eq!(result.status, SourceDetectionStatus::Unsupported);
    });
}

fn any_of_profile() -> RegistrySourceProfile {
    registry_profile(json!({
        "schemaVersion": 1,
        "key": "example_board",
        "name": "Example Board",
        "kind": "recruiting_system",
        "detect": {
            "phases": ["http"],
            "required": [{ "htmlContains": "example-board-root" }],
            "anyOf": [
                [
                    {
                        "htmlRegex": "firstTenant\\s*=\\s*\"([^\"]+)\"",
                        "captureAs": "tenant"
                    },
                    { "htmlContains": "first-alt-ready" }
                ],
                [{
                    "htmlRegex": "secondTenant\\s*=\\s*\"([^\"]+)\"",
                    "captureAs": "tenant"
                }]
            ]
        },
        "identity": {
            "keyCandidates": ["{{capture:tenant|technicalKey}}"],
            "nameCandidates": ["{{capture:tenant|titleCase}}"]
        },
        "accessPaths": [{
            "key": "endpoint_inventory",
            "adapterKey": "declarative_endpoint_inventory",
            "availability": {
                "requiredCaptures": ["tenant"],
                "sourceConfig": { "tenant": "{{capture:tenant}}" }
            }
        }]
    }))
}

fn matching_profile(key: &str) -> RegistrySourceProfile {
    registry_profile(json!({
        "schemaVersion": 1,
        "key": key,
        "name": title_from_key(key),
        "kind": "recruiting_system",
        "detect": {
            "phases": ["http"],
            "required": [{ "htmlContains": "shared-board-root" }]
        },
        "accessPaths": [{
            "key": "endpoint_inventory",
            "adapterKey": "declarative_endpoint_inventory",
            "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
        }]
    }))
}

fn builtin_profile(key: &str) -> RegistrySourceProfile {
    match key {
        "ashby" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/ashby.json"
        )),
        "greenhouse" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/greenhouse.json"
        )),
        "lever" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/lever.json"
        )),
        "magnolia_esmp_job_search" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/magnolia_esmp_job_search.json"
        )),
        "muz_global_jobboard" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/muz_global_jobboard.json"
        )),
        "personio" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/personio.json"
        )),
        "successfactors" => registry_profile_from_str(include_str!(
            "../../../../source-profiles/builtin/successfactors.json"
        )),
        other => panic!("unknown built-in source profile fixture {other}"),
    }
}

fn access_path_inventory_fetch_url<'a>(
    profile: &'a RegistrySourceProfile,
    path_key: &str,
) -> &'a str {
    profile
        .document
        .access_paths
        .iter()
        .find(|access_path| access_path.key == path_key)
        .and_then(|access_path| access_path.inventory.as_ref())
        .and_then(|inventory| inventory.pointer("/fetch/url"))
        .and_then(Value::as_str)
        .unwrap()
}

fn registry_profile(value: Value) -> RegistrySourceProfile {
    let document: SourceProfileDocument = serde_json::from_value(value).unwrap();
    wrap_registry_profile(document)
}

fn registry_profile_from_str(contents: &str) -> RegistrySourceProfile {
    let document: SourceProfileDocument = serde_json::from_str(contents).unwrap();
    wrap_registry_profile(document)
}

fn wrap_registry_profile(document: SourceProfileDocument) -> RegistrySourceProfile {
    RegistrySourceProfile {
        origin: SourceRegistryDocumentOrigin::BuiltIn,
        path: format!("source-profiles/builtin/{}.json", document.key),
        document,
    }
}

fn title_from_key(key: &str) -> String {
    key.split('_')
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
