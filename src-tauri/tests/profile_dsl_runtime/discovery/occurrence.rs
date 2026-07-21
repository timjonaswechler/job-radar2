use super::*;

#[test]
fn discovery_emits_minimal_occurrences_and_keeps_later_complete_items() {
    let mut fields = default_fields();
    fields.as_object_mut().unwrap().remove("title");
    fields.as_object_mut().unwrap().remove("company");
    fields.as_object_mut().unwrap().remove("locations");
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({ "jobs": [
            { "url": "https://example.test/jobs/minimal" },
            { "url": "https://example.test/jobs/complete" }
        ]})
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 2);
    assert_eq!(
        result.candidates[0].provider_values,
        job_radar_lib::ProviderValues::default()
    );
    assert_eq!(
        result.candidates[1].reference.provider_url,
        "https://example.test/jobs/complete"
    );
    assert!(result.diagnostics.is_empty());
}

#[test]
fn discovery_keeps_provider_values_hints_and_posting_meta_disjoint() {
    let mut fields = default_fields();
    fields["providerPostingId"] =
        json!({ "type": "json_path", "jsonPath": "$.id", "cardinality": "optional" });
    fields["locations"] =
        json!({ "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" });
    fields["hints"] = json!({
        "title": {
            "value": { "type": "json_path", "jsonPath": "$.hint", "cardinality": "optional" },
            "hintUse": "search_prefilter"
        },
        "company": {
            "value": { "type": "const", "value": "hint-company" }
        }
    });
    fields["postingMeta"] = json!({
        "title": { "type": "json_path", "jsonPath": "$.secret", "cardinality": "optional" }
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({ "jobs": [{
            "id": "Case-Sensitive-42",
            "url": "https://example.test/jobs/42#provider-fragment",
            "title": " Provider Title ",
            "company": "Provider Company",
            "locations": ["Berlin", "Berlin", "Remote"],
            "hint": "guessed title",
            "secret": "source-local"
        }]})
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));
    let occurrence = &result.candidates[0];

    assert_eq!(
        occurrence.provider_values.title.as_deref(),
        Some("Provider Title")
    );
    assert_eq!(
        occurrence.provider_values.locations,
        ["Berlin", "Berlin", "Remote"]
    );
    assert_eq!(occurrence.hints["title"].value, "guessed title");
    assert_eq!(
        occurrence.hints["title"].hint_use,
        Some(job_radar_lib::HintUse::SearchPrefilter)
    );
    assert_eq!(occurrence.hints["company"].hint_use, None);
    assert_eq!(occurrence.posting_meta["title"], "source-local");
    assert_eq!(
        occurrence.identity,
        job_radar_lib::PostingOccurrenceIdentity::ProviderPostingId {
            source_key: "example_source".to_string(),
            provider_posting_id: "Case-Sensitive-42".to_string(),
        }
    );
}

#[test]
fn invalid_references_suppress_only_the_item_and_do_not_leak_urls() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let forbidden = "https://user:super-secret@example.test/private?token=do-not-leak";
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({ "jobs": [
            { "title": "Bad", "company": "Example", "url": forbidden },
            { "title": "Good", "company": "Example", "url": "https://example.test/jobs/good" }
        ]})
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0].provider_values.title.as_deref(),
        Some("Good")
    );
    let serialized = serde_json::to_string(&result.diagnostics).unwrap();
    assert!(serialized.contains("occurrence_reference_invalid"));
    assert!(!serialized.contains("super-secret"));
    assert!(!serialized.contains("do-not-leak"));
    assert!(!serialized.contains("/private"));
}

#[test]
fn empty_provider_id_suppresses_the_item_with_an_item_scoped_diagnostic() {
    let mut fields = default_fields();
    fields["providerPostingId"] =
        json!({ "type": "json_path", "jsonPath": "$.id", "cardinality": "optional" });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({ "jobs": [{
            "id": "   ", "title": "Bad", "company": "Example",
            "url": "https://example.test/jobs/42"
        }]})
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(result.diagnostics[0].code, "occurrence_provider_id_empty");
    assert_eq!(
        result.diagnostics[0].details.as_ref().unwrap()["itemIndex"],
        0
    );
}
