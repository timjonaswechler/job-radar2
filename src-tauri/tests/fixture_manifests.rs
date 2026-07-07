use job_radar_lib::{FixtureManifest, FixtureManifestPostingField, FixtureManifestRequestMethod};
use serde_json::json;

#[test]
fn representative_fixture_manifest_deserializes_with_requests_and_checks() {
    let manifest: FixtureManifest = serde_json::from_value(representative_manifest()).unwrap();

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.profile_key, "example_profile");
    assert_eq!(manifest.access_path_key, "api");
    assert_eq!(
        manifest.source_config["apiBaseUrl"],
        json!("https://jobs.example.com/api")
    );

    let request = manifest.requests.first().unwrap();
    assert_eq!(request.key, "discovery_jobs");
    assert_eq!(
        request.request_match.method,
        FixtureManifestRequestMethod::Get
    );
    assert_eq!(
        request.request_match.url,
        "https://jobs.example.com/api/jobs"
    );
    assert_eq!(request.response.status, 200);
    assert_eq!(
        request.response.headers.as_ref().unwrap()["content-type"],
        "application/json"
    );
    assert_eq!(request.response.body_file, "responses/jobs.json");

    let discovery = manifest.checks.posting_discovery.unwrap();
    assert_eq!(discovery.expect.min_candidates, Some(1));
    assert_eq!(
        discovery.expect.required_fields.as_ref().unwrap(),
        &vec![
            FixtureManifestPostingField::Title,
            FixtureManifestPostingField::Company,
            FixtureManifestPostingField::Url,
        ]
    );
    assert_eq!(
        discovery.expect.contains_candidates.as_ref().unwrap()[0]
            .title
            .as_deref(),
        Some("Software Engineer")
    );

    let detail_case = &manifest.checks.posting_detail.unwrap().cases[0];
    assert_eq!(detail_case.key, "job_123_detail");
    assert_eq!(
        detail_case.posting.posting_meta.as_ref().unwrap()["jobId"],
        json!("123")
    );
    assert_eq!(detail_case.expect.min_description_length, Some(40));
    assert_eq!(
        detail_case.expect.description_contains.as_ref().unwrap(),
        &vec!["responsibilities".to_string()]
    );
}

#[test]
fn fixture_manifest_deserialization_rejects_unsupported_schema_version() {
    let mut manifest = representative_manifest();
    manifest["schemaVersion"] = json!(2);

    let error = serde_json::from_value::<FixtureManifest>(manifest).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported Fixture Manifest schemaVersion"),
        "unexpected error: {error}"
    );
}

#[test]
fn fixture_manifest_deserialization_rejects_missing_required_fields() {
    let mut missing_profile_key = representative_manifest();
    missing_profile_key
        .as_object_mut()
        .unwrap()
        .remove("profileKey");
    assert!(serde_json::from_value::<FixtureManifest>(missing_profile_key).is_err());

    let mut missing_access_path_key = representative_manifest();
    missing_access_path_key
        .as_object_mut()
        .unwrap()
        .remove("accessPathKey");
    assert!(serde_json::from_value::<FixtureManifest>(missing_access_path_key).is_err());

    let mut missing_body_file = representative_manifest();
    missing_body_file["requests"][0]["response"]
        .as_object_mut()
        .unwrap()
        .remove("bodyFile");
    assert!(serde_json::from_value::<FixtureManifest>(missing_body_file).is_err());
}

#[test]
fn fixture_manifest_deserialization_rejects_non_absolute_request_url() {
    let mut manifest = representative_manifest();
    manifest["requests"][0]["match"]["url"] = json!("/api/jobs");

    let error = serde_json::from_value::<FixtureManifest>(manifest).unwrap_err();
    assert!(
        error.to_string().contains("absolute HTTP(S) URL"),
        "unexpected error: {error}"
    );
}

#[test]
fn fixture_manifest_deserialization_rejects_status_outside_http_range() {
    let mut manifest = representative_manifest();
    manifest["requests"][0]["response"]["status"] = json!(700);

    let error = serde_json::from_value::<FixtureManifest>(manifest).unwrap_err();
    assert!(
        error.to_string().contains("response status"),
        "unexpected error: {error}"
    );
}

fn representative_manifest() -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "profileKey": "example_profile",
        "accessPathKey": "api",
        "sourceConfig": {
            "apiBaseUrl": "https://jobs.example.com/api"
        },
        "requests": [
            {
                "key": "discovery_jobs",
                "match": {
                    "method": "GET",
                    "url": "https://jobs.example.com/api/jobs"
                },
                "response": {
                    "status": 200,
                    "headers": {
                        "content-type": "application/json"
                    },
                    "bodyFile": "responses/jobs.json"
                }
            }
        ],
        "checks": {
            "postingDiscovery": {
                "expect": {
                    "minCandidates": 1,
                    "requiredFields": ["title", "company", "url"],
                    "containsCandidates": [
                        {
                            "title": "Software Engineer",
                            "company": "Example",
                            "url": "https://jobs.example.com/jobs/123"
                        }
                    ]
                }
            },
            "postingDetail": {
                "cases": [
                    {
                        "key": "job_123_detail",
                        "posting": {
                            "title": "Software Engineer",
                            "company": "Example",
                            "url": "https://jobs.example.com/jobs/123",
                            "postingMeta": {
                                "jobId": "123"
                            }
                        },
                        "expect": {
                            "minDescriptionLength": 40,
                            "descriptionContains": ["responsibilities"]
                        }
                    }
                ]
            }
        }
    })
}
