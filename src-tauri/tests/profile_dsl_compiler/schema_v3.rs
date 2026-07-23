use job_radar_lib::{SourceDocument, SourceProfileDocument};
use serde_json::{json, Value};

#[test]
fn schema_v3_profile_and_direct_source_specialization_round_trip_with_final_vocabulary() {
    let profile = profile_document();
    let parsed_profile: SourceProfileDocument = serde_json::from_value(profile.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed_profile).unwrap(), profile);

    let source = source_document();
    let parsed_source: SourceDocument = serde_json::from_value(source.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed_source).unwrap(), source);
}

#[test]
fn serde_rejects_v2_old_phase_override_and_retry_shapes_without_conversion() {
    let mut v2_profile = profile_document();
    v2_profile["schemaVersion"] = json!(2);
    assert_rejected::<SourceProfileDocument>(v2_profile, "schema version 2");

    let mut old_phase_profile = profile_document();
    let discovery = old_phase_profile["accessPaths"][0]
        .as_object_mut()
        .unwrap()
        .remove("discovery")
        .unwrap();
    old_phase_profile["accessPaths"][0]["postingDiscovery"] = discovery;
    assert_rejected::<SourceProfileDocument>(old_phase_profile, "postingDiscovery");

    let mut old_override_source = source_document();
    old_override_source["sourceOverrides"] = json!({});
    assert_rejected::<SourceDocument>(old_override_source, "sourceOverrides");

    let mut legacy_discovery_output = profile_document();
    legacy_discovery_output["accessPaths"][0]["discovery"]["strategies"][0]["extract"] = json!({
        "fields": {
            "title": { "type": "json_path", "jsonPath": "$.title" },
            "company": { "type": "json_path", "jsonPath": "$.company" },
            "url": { "type": "json_path", "jsonPath": "$.url" }
        }
    });
    assert_rejected::<SourceProfileDocument>(legacy_discovery_output, "extract.fields");

    let mut retry_profile = profile_document();
    retry_profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["retry"] =
        json!({ "maxAttempts": 2 });
    assert_rejected::<SourceProfileDocument>(retry_profile, "retry");
}

fn assert_rejected<T>(value: Value, case: &str)
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value::<T>(value).unwrap_err_or_else(case);
}

trait UnwrapErrOrElse {
    fn unwrap_err_or_else(self, case: &str);
}

impl<T> UnwrapErrOrElse for Result<T, serde_json::Error> {
    fn unwrap_err_or_else(self, case: &str) {
        assert!(self.is_err(), "{case} must be rejected");
    }
}

fn profile_document() -> Value {
    json!({
        "schemaVersion": 3,
        "key": "example",
        "name": "Example",
        "kind": "generic",
        "support": { "level": "experimental" },
        "detection": {
            "policy": { "type": "all_required" },
            "strategies": [{
                "type": "url",
                "key": "input_url",
                "input": {
                    "type": "pattern_alternatives",
                    "alternatives": [{ "pattern": "^https://example\\.test/" }]
                }
            }],
            "recommendedAccessPathKey": "main"
        },
        "accessPaths": [{
            "key": "main",
            "name": "Main",
            "discovery": discovery_step()
        }]
    })
}

fn source_document() -> Value {
    json!({
        "schemaVersion": 3,
        "key": "example-source",
        "name": "Example Source",
        "status": "draft",
        "sourceConfig": { "feedUrl": "https://example.test/jobs" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example",
            "pathKey": "main"
        },
        "accessPaths": [{
            "key": "main",
            "discovery": {
                "strategies": [{
                    "key": "api",
                    "acceptWhen": { "minResults": 1 }
                }]
            }
        }]
    })
}

fn discovery_step() -> Value {
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [{
            "key": "api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "{{sourceConfig:feedUrl}}",
                "timeoutMs": 1000
            },
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
        }]
    })
}
