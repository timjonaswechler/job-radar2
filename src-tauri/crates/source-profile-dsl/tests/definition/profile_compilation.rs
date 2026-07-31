use std::{fs, path::PathBuf};

use source_profile_dsl::test_support::{
    compile_source, CompileSourceOutcome, DiagnosticSeverity, ProfileCompilerInput, SourceDocument,
    SourceProfileDocument,
};

#[test]
fn profile_selected_source_compiles_through_public_core_interface() {
    let profile: SourceProfileDocument = fixture("valid/simple-source-profile.json");
    let source: SourceDocument = fixture("valid/source-selecting-access-path.json");
    let profiles = [profile];

    let CompileSourceOutcome::Compiled {
        source: compiled,
        diagnostics,
    } = compile_source(&source, &ProfileCompilerInput::new(&profiles))
    else {
        panic!("valid profile-selected Source should compile");
    };

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error));
    assert_eq!(
        source_profile_dsl::test_support::test_execution_plan(&compiled)
            .source
            .key,
        source.key
    );
    serde_json::to_value(&source_profile_dsl::test_support::test_execution_plan(
        &compiled,
    ))
    .expect("Execution Plan should serialize");
}

#[test]
fn source_owned_access_path_uses_the_same_public_compile_interface() {
    let source: SourceDocument = fixture("valid/source-owned-access-path.json");

    let CompileSourceOutcome::Compiled {
        source: compiled,
        diagnostics,
    } = compile_source(&source, &ProfileCompilerInput::new(&[]))
    else {
        panic!("valid Source-owned Access Path should compile");
    };

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error));
    assert_eq!(
        source_profile_dsl::test_support::test_execution_plan(&compiled)
            .source
            .key,
        source.key
    );
}

#[test]
fn compilation_owned_primitive_inventory_is_complete() {
    use source_profile_dsl::test_support::{
        production_primitive_inventories, validate_primitive_completeness,
    };

    validate_primitive_completeness(&production_primitive_inventories())
        .expect("compilation-owned Primitive inventory should be complete");
}

#[test]
fn direct_core_compilation_validates_detection_definitions() {
    let mut profile_value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../sources/resources/profiles/successfactors.json"
    ))
    .expect("valid built-in profile fixture");
    profile_value["detection"]["strategies"][1]["key"] =
        profile_value["detection"]["strategies"][0]["key"].clone();
    let profile: SourceProfileDocument =
        serde_json::from_value(profile_value).expect("authored Detection document");
    let mut source_value: serde_json::Value = fixture("valid/source-selecting-access-path.json");
    source_value
        .as_object_mut()
        .expect("Source object")
        .remove("accessPaths");
    source_value["selectedAccessPath"]["profileKey"] = serde_json::json!("successfactors");
    source_value["selectedAccessPath"]["pathKey"] = serde_json::json!("rmk_sitemap_html");
    let source: SourceDocument = serde_json::from_value(source_value).expect("valid Source");
    let profiles = [profile];

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &ProfileCompilerInput::new(&profiles))
    else {
        panic!("invalid Detection definition must reject direct Core compilation");
    };

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "invalid_detection_strategy_key")
        .unwrap_or_else(|| panic!("Detection compilation Diagnostic: {diagnostics:#?}"));
    assert_eq!(diagnostic.path, "/detection/strategies/1/key");
}

#[test]
fn missing_profile_is_a_closed_rejected_outcome_with_structured_diagnostic() {
    let source: SourceDocument = fixture("valid/source-selecting-access-path.json");

    let CompileSourceOutcome::Rejected { diagnostics } =
        compile_source(&source, &ProfileCompilerInput::new(&[]))
    else {
        panic!("missing Source Profile should reject compilation");
    };

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "source_profile_not_found")
        .expect("missing profile Diagnostic");
    assert_eq!(diagnostic.path, "/selectedAccessPath/profileKey");
}

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = fixture_root().join(name);
    let json = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("failed to deserialize {}: {error}", path.display()))
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/source-behavior")
}
