use std::fs;

use serde_json::Value;
use source_profile_dsl::definition::DiagnosticCategory;
use sources::installed::{
    Admission, Origin, Profiles, MAX_AGGREGATE_PROFILE_BYTES, MAX_CUSTOM_PROFILE_DOCUMENTS,
    MAX_DIAGNOSTICS_PER_DOCUMENT, MAX_DIAGNOSTICS_PER_SNAPSHOT, MAX_PROFILE_BYTES,
};

#[test]
fn real_bundled_profiles_are_admitted_through_the_installed_interface() {
    let app_data = tempfile::tempdir().unwrap();

    let profiles = Profiles::load(app_data.path()).expect("bundled Profiles must be valid");
    let view = profiles.view();

    assert_eq!(view.profiles.len(), 3);
    assert!(view.profiles.iter().all(|profile| {
        profile.origin == Origin::BuiltIn && profile.admission == Admission::Admitted
    }));
    assert_eq!(
        view.profiles
            .iter()
            .map(|profile| profile.definition.as_ref().unwrap().key.as_str())
            .collect::<Vec<_>>(),
        ["greenhouse", "successfactors", "workday"]
    );
    assert!(view.diagnostics.is_empty());
}

#[test]
fn custom_profiles_are_ordered_and_cannot_replace_a_builtin_key() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    write_profile(&directory, "zeta", |_| {});
    write_profile(&directory, "alpha", |_| {});
    fs::write(
        directory.join("greenhouse.json"),
        include_str!("../resources/profiles/greenhouse.json"),
    )
    .unwrap();

    let profiles = Profiles::load(app_data.path()).unwrap();
    let customs = profiles
        .view()
        .profiles
        .iter()
        .filter(|profile| profile.origin == Origin::Custom)
        .collect::<Vec<_>>();

    assert_eq!(
        customs
            .iter()
            .map(|profile| profile.file_name.as_str())
            .collect::<Vec<_>>(),
        ["alpha.json", "greenhouse.json", "zeta.json"]
    );
    assert_eq!(customs[1].admission, Admission::Rejected);
    assert!(profiles
        .view()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "duplicate_source_profile_key"));
    assert!(profiles.view().profiles.iter().any(|profile| {
        profile.origin == Origin::BuiltIn
            && profile.admission == Admission::Admitted
            && profile
                .definition
                .as_ref()
                .is_some_and(|definition| definition.key == "greenhouse")
    }));
}

#[test]
fn semantic_invalid_custom_is_quarantined_from_productive_installed_state() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    write_profile(&directory, "broken_detection", |profile| {
        profile["detection"]["strategies"][0]["input"]["alternatives"][0]["pattern"] =
            Value::String("(".to_string());
    });

    let profiles = Profiles::load(app_data.path()).unwrap();
    let rejected = profiles
        .view()
        .profiles
        .iter()
        .find(|profile| {
            profile
                .definition
                .as_ref()
                .map(|definition| definition.key.as_str())
                == Some("broken_detection")
        })
        .unwrap();

    assert_eq!(rejected.admission, Admission::Rejected);
    assert!(profiles
        .view()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.category == DiagnosticCategory::Compiler));
    assert!(profiles
        .view()
        .profiles
        .iter()
        .find(|profile| profile
            .definition
            .as_ref()
            .is_some_and(|definition| definition.key == "broken_detection"))
        .is_some_and(|profile| profile.admission == Admission::Rejected));
}

#[test]
fn schema_and_filename_failures_report_version_three_and_are_not_admitted() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    write_profile(&directory, "wrong_filename", |profile| {
        profile["schemaVersion"] = Value::from(2);
        profile["key"] = Value::String("different_key".to_string());
    });

    let profiles = Profiles::load(app_data.path()).unwrap();
    let rejected = profiles
        .view()
        .profiles
        .iter()
        .find(|profile| profile.file_name == "wrong_filename.json")
        .unwrap();

    assert_eq!(rejected.admission, Admission::Rejected);
    let version = profiles
        .view()
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "unsupported_schema_version")
        .unwrap();
    assert_eq!(
        version.details.as_ref().unwrap()["expectedSchemaVersion"],
        3
    );
    assert!(profiles
        .view()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "filename_key_mismatch"));
}

#[test]
fn custom_document_count_and_per_profile_bytes_are_bounded() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    for index in 0..=MAX_CUSTOM_PROFILE_DOCUMENTS {
        fs::write(directory.join(format!("profile_{index:03}.json")), "{}").unwrap();
    }

    let profiles = Profiles::load(app_data.path()).unwrap();
    assert_eq!(
        profiles
            .view()
            .profiles
            .iter()
            .filter(|profile| profile.origin == Origin::Custom)
            .count(),
        MAX_CUSTOM_PROFILE_DOCUMENTS
    );
    assert!(profiles
        .view()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "custom_profile_document_limit_exceeded"));

    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("oversized.json"),
        vec![b' '; MAX_PROFILE_BYTES + 1],
    )
    .unwrap();
    let profiles = Profiles::load(app_data.path()).unwrap();
    let oversized = profiles
        .view()
        .profiles
        .iter()
        .find(|profile| profile.file_name == "oversized.json")
        .unwrap();
    assert_eq!(oversized.admission, Admission::Rejected);
    assert!(profiles
        .view()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "profile_bytes_limit_exceeded"));
}

#[test]
fn aggregate_profile_bytes_are_bounded() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    let padding = "x".repeat(1_000_000);
    for index in 0..34 {
        write_profile(&directory, &format!("large_{index:02}"), |profile| {
            profile["description"] = Value::String(padding.clone());
        });
    }

    let profiles = Profiles::load(app_data.path()).unwrap();

    assert!(profiles.view().diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "aggregate_profile_bytes_limit_exceeded"
            && diagnostic.details.as_ref().unwrap()["maximumBytes"] == MAX_AGGREGATE_PROFILE_BYTES
    }));
    assert!(profiles
        .view()
        .profiles
        .iter()
        .any(|profile| profile.admission == Admission::Rejected));
}

#[test]
fn document_and_snapshot_diagnostics_are_deterministically_truncated() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    for index in 0..42 {
        write_profile(&directory, &format!("invalid_{index:02}"), |profile| {
            let access_path = profile["accessPaths"][0].clone();
            profile["accessPaths"] = Value::Array(vec![access_path; 102]);
        });
    }

    let profiles = Profiles::load(app_data.path()).unwrap();
    let first_document_diagnostics = &profiles.view().diagnostics[..MAX_DIAGNOSTICS_PER_DOCUMENT];
    assert_eq!(
        first_document_diagnostics.last().unwrap().code,
        "profile_diagnostics_truncated"
    );
    assert_eq!(
        profiles.view().diagnostics.len(),
        MAX_DIAGNOSTICS_PER_SNAPSHOT
    );
    assert_eq!(
        profiles.view().diagnostics.last().unwrap().code,
        "snapshot_diagnostics_truncated"
    );
}

#[test]
fn host_view_serialization_omits_raw_paths_documents_and_detection_plans() {
    let app_data = tempfile::tempdir().unwrap();
    let directory = app_data.path().join("source-profiles");
    fs::create_dir_all(&directory).unwrap();
    write_profile(&directory, "custom_profile", |_| {});

    let serialized =
        serde_json::to_string(Profiles::load(app_data.path()).unwrap().view()).unwrap();

    assert!(!serialized.contains(&app_data.path().display().to_string()));
    assert!(!serialized.contains("compiledDetection"));
    assert!(!serialized.contains("preparedDetection"));
    let value: Value = serde_json::from_str(&serialized).unwrap();
    assert!(value["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .all(|profile| { profile.get("path").is_none() && profile.get("document").is_none() }));
    assert!(serialized.contains("\"fileName\":\"custom_profile.json\""));
}

fn write_profile(directory: &std::path::Path, key: &str, change: impl FnOnce(&mut Value)) {
    let mut profile: Value =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    profile["key"] = Value::String(key.to_string());
    profile["name"] = Value::String(format!("{key} Profile"));
    change(&mut profile);
    fs::write(
        directory.join(format!("{key}.json")),
        serde_json::to_vec_pretty(&profile).unwrap(),
    )
    .unwrap();
}
