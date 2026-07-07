use std::path::{Component, Path, PathBuf};

use serde_json::json;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};

pub const SOURCE_PROFILE_FIXTURES_DIR: &str = "source-profile-fixtures";
pub const DEFAULT_FIXTURE_MANIFEST_REFERENCE: &str = "fixture.json";

#[derive(Clone, Debug, PartialEq)]
pub struct FixturePathResolution {
    pub fixture_root: PathBuf,
    pub resolved_path: Option<PathBuf>,
    pub diagnostics: Diagnostics,
}

impl FixturePathResolution {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

pub fn fixture_pack_root(app_data_dir: impl AsRef<Path>, profile_key: impl AsRef<str>) -> PathBuf {
    app_data_dir
        .as_ref()
        .join(SOURCE_PROFILE_FIXTURES_DIR)
        .join(profile_key.as_ref())
}

pub fn resolve_fixture_manifest_reference(
    app_data_dir: impl AsRef<Path>,
    profile_key: impl AsRef<str>,
    reference: impl AsRef<str>,
) -> FixturePathResolution {
    let profile_key = profile_key.as_ref();
    let reference = reference.as_ref();
    let fixture_root = fixture_pack_root(app_data_dir, profile_key);

    if !fixture_root.is_dir() {
        return FixturePathResolution {
            fixture_root: fixture_root.clone(),
            resolved_path: None,
            diagnostics: vec![directory_missing_diagnostic(profile_key, &fixture_root)],
        };
    }

    let Some(resolved_path) = normalize_fixture_reference(&fixture_root, reference) else {
        let diagnostic =
            reference_path_traversal_diagnostic(profile_key, reference, &fixture_root, None);
        return FixturePathResolution {
            fixture_root,
            resolved_path: None,
            diagnostics: vec![diagnostic],
        };
    };

    let diagnostics = if resolved_path.is_file() {
        Vec::new()
    } else {
        vec![manifest_missing_diagnostic(
            profile_key,
            reference,
            &resolved_path,
        )]
    };

    FixturePathResolution {
        fixture_root,
        resolved_path: Some(resolved_path),
        diagnostics,
    }
}

pub fn resolve_fixture_file_reference(
    app_data_dir: impl AsRef<Path>,
    profile_key: impl AsRef<str>,
    manifest_reference: impl AsRef<str>,
    reference: impl AsRef<str>,
) -> FixturePathResolution {
    let profile_key = profile_key.as_ref();
    let manifest_reference = manifest_reference.as_ref();
    let reference = reference.as_ref();
    let fixture_root = fixture_pack_root(app_data_dir, profile_key);

    if !fixture_root.is_dir() {
        return FixturePathResolution {
            fixture_root: fixture_root.clone(),
            resolved_path: None,
            diagnostics: vec![directory_missing_diagnostic(profile_key, &fixture_root)],
        };
    }

    let Some(resolved_path) = normalize_fixture_reference(&fixture_root, reference) else {
        let diagnostic = reference_path_traversal_diagnostic(
            profile_key,
            reference,
            &fixture_root,
            Some(manifest_reference),
        );
        return FixturePathResolution {
            fixture_root,
            resolved_path: None,
            diagnostics: vec![diagnostic],
        };
    };

    let diagnostics = if resolved_path.is_file() {
        Vec::new()
    } else {
        vec![file_missing_diagnostic(
            profile_key,
            manifest_reference,
            reference,
            &resolved_path,
        )]
    };

    FixturePathResolution {
        fixture_root,
        resolved_path: Some(resolved_path),
        diagnostics,
    }
}

fn normalize_fixture_reference(fixture_root: &Path, reference: &str) -> Option<PathBuf> {
    if reference.is_empty()
        || is_home_style_reference(reference)
        || is_windows_absolute_reference(reference)
    {
        return None;
    }

    let reference_path = Path::new(reference);
    let mut normalized_reference = PathBuf::new();

    for component in reference_path.components() {
        match component {
            Component::Normal(part) => normalized_reference.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if normalized_reference.as_os_str().is_empty() {
        return None;
    }

    let resolved_path = fixture_root.join(normalized_reference);
    if resolved_path.starts_with(fixture_root) {
        Some(resolved_path)
    } else {
        None
    }
}

fn is_home_style_reference(reference: &str) -> bool {
    reference == "~" || reference.starts_with("~/") || reference.starts_with("~\\")
}

fn is_windows_absolute_reference(reference: &str) -> bool {
    let bytes = reference.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\'))
        || reference.starts_with("\\\\")
}

fn directory_missing_diagnostic(profile_key: &str, fixture_root: &Path) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.directory_missing".to_string(),
        message: format!(
            "Fixture Pack directory for Source Profile `{profile_key}` does not exist"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(json!({
            "profileKey": profile_key,
            "fixtureRoot": fixture_root.display().to_string()
        })),
    }
}

fn manifest_missing_diagnostic(
    profile_key: &str,
    reference: &str,
    resolved_path: &Path,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.manifest_missing".to_string(),
        message: format!(
            "Fixture Manifest `{reference}` for Source Profile `{profile_key}` does not exist"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(json!({
            "profileKey": profile_key,
            "reference": reference,
            "resolvedPath": resolved_path.display().to_string()
        })),
    }
}

fn file_missing_diagnostic(
    profile_key: &str,
    manifest_reference: &str,
    reference: &str,
    resolved_path: &Path,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.file_missing".to_string(),
        message: format!(
            "Fixture file `{reference}` referenced by Fixture Manifest `{manifest_reference}` for Source Profile `{profile_key}` does not exist"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(json!({
            "profileKey": profile_key,
            "manifestReference": manifest_reference,
            "reference": reference,
            "resolvedPath": resolved_path.display().to_string()
        })),
    }
}

fn reference_path_traversal_diagnostic(
    profile_key: &str,
    reference: &str,
    fixture_root: &Path,
    manifest_reference: Option<&str>,
) -> Diagnostic {
    let mut details = json!({
        "profileKey": profile_key,
        "reference": reference,
        "fixtureRoot": fixture_root.display().to_string()
    });

    if let Some(manifest_reference) = manifest_reference {
        details["manifestReference"] = json!(manifest_reference);
    }

    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.reference_path_traversal".to_string(),
        message: format!(
            "Fixture reference `{reference}` for Source Profile `{profile_key}` is not a safe Fixture-Pack-relative path"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(details),
    }
}
