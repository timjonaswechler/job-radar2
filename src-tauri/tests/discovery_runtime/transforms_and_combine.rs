use super::*;

#[test]
fn compiled_discovery_runtime_applies_explicit_whitespace_transforms() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.title",
        "cardinality": "one",
        "transforms": [{ "type": "to_string" }, { "type": "trim" }, { "type": "normalize_whitespace" }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "\n\tStaff    Platform\nEngineer\t",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Staff Platform Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_applies_url_decode_and_slug_to_title_transforms_in_order() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.titleSlug",
        "cardinality": "one",
        "transforms": [{ "type": "to_string" }, { "type": "url_decode" }, { "type": "slug_to_title" }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "titleSlug": "senior%20rust-engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_dedupes_string_arrays_after_all_cardinality() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.titles",
        "cardinality": "all",
        "transforms": [{ "type": "to_string" }, { "type": "dedupe" }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "titles": ["Rust Engineer", "Rust Engineer"],
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_joins_arrays_after_all_cardinality() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.titleParts",
        "cardinality": "all",
        "transforms": [{ "type": "to_string" }, { "type": "join", "separator": " " }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "titleParts": ["Senior", "Rust", "Engineer"],
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_applies_regex_replace_transforms() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.title",
        "cardinality": "one",
        "transforms": [{ "type": "to_string" }, { "type": "regex_replace", "pattern": "\\s*\\(m/f/d\\)$", "replacement": "" }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Senior Rust Engineer (m/f/d)",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_combines_parts_in_declared_order() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " / ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "Senior",
                "role": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior / Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_fails_combine_when_required_part_is_missing() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "Senior",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].provider_values.title, None);
    assert_runtime_diagnostic(&result.diagnostics[0], "required_combine_part_missing");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/extract/providerValues/title/parts/1/value"
    );
}

#[test]
fn compiled_discovery_runtime_allows_missing_optional_combine_part() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" }, "optional": true },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "role": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_preserves_empty_combine_join() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": "",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.prefix", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.suffix", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "prefix": "Data",
                "suffix": "Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "DataEngineer"
    );
}

#[test]
fn compiled_discovery_runtime_applies_final_transforms_after_combine() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": "-",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ],
        "transforms": [{ "type": "to_string" }, { "type": "slug_to_title" }]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "senior",
                "role": "rust-engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior Rust Engineer"
    );
}

#[test]
fn source_owned_discovery_runtime_uses_same_combine_behavior() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = source_owned_json_discovery_plan(fields);
    let fetcher = fake_fetcher([(
        "https://example.test/source-owned.json",
        json!({
            "jobs": [{
                "level": "Staff",
                "role": "Platform Engineer",
                "company": "Owned Example GmbH",
                "url": "https://example.test/jobs/owned-1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Staff Platform Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_normalizes_single_location_expression_without_implicit_splitting() {
    let mut fields = default_fields();
    fields["locations"] = json!({
        "type": "json_path",
        "jsonPath": "$.locations",
        "cardinality": "all"
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "locations": ["  Berlin  ", "", "Berlin", "Remote, München", " Remote, München "]
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].provider_values.locations,
        vec!["Berlin", "Berlin", "Remote, München", "Remote, München"]
    );
}

#[test]
fn compiled_discovery_runtime_normalizes_list_style_locations_in_order() {
    let mut fields = default_fields();
    fields["locations"] = json!([
        { "type": "json_path", "jsonPath": "$.primaryLocation", "cardinality": "one" },
        { "type": "json_path", "jsonPath": "$.otherLocations", "cardinality": "all" }
    ]);
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "primaryLocation": " Remote ",
                "otherLocations": ["Berlin", "Remote", " München ", ""]
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].provider_values.locations,
        vec!["Remote", "Berlin", "Remote", "München"]
    );
}

#[test]
fn compiled_discovery_runtime_splits_and_dedupes_location_arrays_in_order() {
    let mut fields = default_fields();
    fields["locations"] = json!({
        "type": "json_path",
        "jsonPath": "$.locationsText",
        "cardinality": "one",
        "transforms": [{ "type": "to_string" },
            { "type": "split", "separator": ";" },
            { "type": "trim" },
            { "type": "dedupe" }
        ]
    });
    let plan = compiled_json_discovery_plan(fields, default_select());
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "locationsText": " Berlin ;Remote; Berlin; München "
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].provider_values.locations,
        vec!["Berlin", "Remote", "München"]
    );
}
