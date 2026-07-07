use std::fs;

use job_radar_lib::{
    fixture_pack_root, resolve_fixture_file_reference, resolve_fixture_manifest_reference,
};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn fixture_pack_root_uses_source_profile_fixtures_directory() {
    let app_data_dir = tempdir().unwrap();

    let root = fixture_pack_root(app_data_dir.path(), "example_profile");

    assert_eq!(
        root,
        app_data_dir
            .path()
            .join("source-profile-fixtures")
            .join("example_profile")
    );
}

#[test]
fn safe_manifest_reference_resolves_inside_fixture_pack_root() {
    let app_data_dir = tempdir().unwrap();
    let fixture_root = fixture_pack_root(app_data_dir.path(), "example_profile");
    fs::create_dir_all(&fixture_root).unwrap();
    fs::write(fixture_root.join("fixture.json"), b"{}").unwrap();

    let resolution =
        resolve_fixture_manifest_reference(app_data_dir.path(), "example_profile", "fixture.json");

    assert!(
        resolution.diagnostics.is_empty(),
        "{:#?}",
        resolution.diagnostics
    );
    assert_eq!(resolution.fixture_root, fixture_root);
    assert_eq!(
        resolution.resolved_path,
        Some(fixture_root.join("fixture.json"))
    );
}

#[test]
fn safe_subdirectory_fixture_file_reference_resolves_inside_fixture_pack_root() {
    let app_data_dir = tempdir().unwrap();
    let fixture_root = fixture_pack_root(app_data_dir.path(), "example_profile");
    fs::create_dir_all(fixture_root.join("responses")).unwrap();
    fs::write(fixture_root.join("responses/jobs.json"), b"{}").unwrap();

    let resolution = resolve_fixture_file_reference(
        app_data_dir.path(),
        "example_profile",
        "fixture.json",
        "responses/jobs.json",
    );

    assert!(
        resolution.diagnostics.is_empty(),
        "{:#?}",
        resolution.diagnostics
    );
    assert_eq!(
        resolution.resolved_path,
        Some(fixture_root.join("responses/jobs.json"))
    );
}

#[test]
fn missing_fixture_pack_directory_emits_directory_missing() {
    let app_data_dir = tempdir().unwrap();

    let resolution =
        resolve_fixture_manifest_reference(app_data_dir.path(), "example_profile", "fixture.json");

    assert_eq!(resolution.resolved_path, None);
    assert_eq!(resolution.diagnostics.len(), 1);
    let diagnostic = &resolution.diagnostics[0];
    assert_eq!(diagnostic.code, "fixture.directory_missing");
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["profileKey"],
        json!("example_profile")
    );
    assert_eq!(
        diagnostic.details.as_ref().unwrap()["fixtureRoot"],
        json!(resolution.fixture_root.display().to_string())
    );
}

#[test]
fn missing_manifest_and_fixture_file_emit_specific_missing_diagnostics() {
    let app_data_dir = tempdir().unwrap();
    let fixture_root = fixture_pack_root(app_data_dir.path(), "example_profile");
    fs::create_dir_all(&fixture_root).unwrap();

    let manifest_resolution =
        resolve_fixture_manifest_reference(app_data_dir.path(), "example_profile", "missing.json");
    assert_eq!(manifest_resolution.diagnostics.len(), 1);
    let manifest_diagnostic = &manifest_resolution.diagnostics[0];
    assert_eq!(manifest_diagnostic.code, "fixture.manifest_missing");
    assert_eq!(
        manifest_diagnostic.details.as_ref().unwrap()["reference"],
        json!("missing.json")
    );
    assert_eq!(
        manifest_diagnostic.details.as_ref().unwrap()["resolvedPath"],
        json!(fixture_root.join("missing.json").display().to_string())
    );

    let file_resolution = resolve_fixture_file_reference(
        app_data_dir.path(),
        "example_profile",
        "fixture.json",
        "responses/missing.json",
    );
    assert_eq!(file_resolution.diagnostics.len(), 1);
    let file_diagnostic = &file_resolution.diagnostics[0];
    assert_eq!(file_diagnostic.code, "fixture.file_missing");
    assert_eq!(
        file_diagnostic.details.as_ref().unwrap()["manifestReference"],
        json!("fixture.json")
    );
    assert_eq!(
        file_diagnostic.details.as_ref().unwrap()["reference"],
        json!("responses/missing.json")
    );
    assert_eq!(
        file_diagnostic.details.as_ref().unwrap()["resolvedPath"],
        json!(fixture_root
            .join("responses/missing.json")
            .display()
            .to_string())
    );
}

#[test]
fn invalid_manifest_and_fixture_file_references_emit_path_traversal_diagnostics() {
    let app_data_dir = tempdir().unwrap();
    let fixture_root = fixture_pack_root(app_data_dir.path(), "example_profile");
    fs::create_dir_all(&fixture_root).unwrap();

    for invalid_reference in [
        "/tmp/fixture.json",
        "C:\\fixtures\\fixture.json",
        "\\\\server\\share\\fixture.json",
        "../fixture.json",
        "responses/../../fixture.json",
        "~/fixture.json",
        "~",
    ] {
        let manifest_resolution = resolve_fixture_manifest_reference(
            app_data_dir.path(),
            "example_profile",
            invalid_reference,
        );
        assert_eq!(manifest_resolution.resolved_path, None);
        assert_eq!(manifest_resolution.diagnostics.len(), 1);
        let diagnostic = &manifest_resolution.diagnostics[0];
        assert_eq!(diagnostic.code, "fixture.reference_path_traversal");
        assert_eq!(
            diagnostic.details.as_ref().unwrap()["reference"],
            json!(invalid_reference)
        );

        let file_resolution = resolve_fixture_file_reference(
            app_data_dir.path(),
            "example_profile",
            "fixture.json",
            invalid_reference,
        );
        assert_eq!(file_resolution.resolved_path, None);
        assert_eq!(file_resolution.diagnostics.len(), 1);
        assert_eq!(
            file_resolution.diagnostics[0].code,
            "fixture.reference_path_traversal"
        );
    }
}
