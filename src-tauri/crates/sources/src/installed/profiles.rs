use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde_json::Value;
use source_profile_dsl::definition::{
    prepare_source_profile_document, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    Diagnostics, SourceProfileDocument,
};

use super::{
    limits::{
        MAX_AGGREGATE_PROFILE_BYTES, MAX_CUSTOM_PROFILE_DOCUMENTS, MAX_DIAGNOSTICS_PER_DOCUMENT,
        MAX_DIAGNOSTICS_PER_SNAPSHOT, MAX_PROFILE_BYTES,
    },
    snapshot::{
        Admission, AdmittedProfile, LoadError, Origin, ProfileDefinitionView, ProfileView,
        Profiles, ProfilesView,
    },
};

const BUILTINS: &[(&str, &str)] = &[
    (
        "greenhouse.json",
        include_str!("../../resources/profiles/greenhouse.json"),
    ),
    (
        "successfactors.json",
        include_str!("../../resources/profiles/successfactors.json"),
    ),
    (
        "workday.json",
        include_str!("../../resources/profiles/workday.json"),
    ),
];

struct Candidate {
    origin: Origin,
    file_name: String,
    contents: String,
}

pub(super) fn load(app_data_dir: &Path) -> Result<Profiles, LoadError> {
    load_with_builtins(app_data_dir, BUILTINS)
}

fn load_with_builtins(
    app_data_dir: &Path,
    builtins: &[(&str, &str)],
) -> Result<Profiles, LoadError> {
    let mut view = ProfilesView::default();
    let mut admitted = Vec::new();
    let mut prepared_detection = Vec::new();
    let mut seen = HashMap::<String, (Origin, String)>::new();
    let mut aggregate_bytes = 0usize;
    let mut builtin_failed = false;

    for (file_name, contents) in builtins {
        aggregate_bytes += contents.len();
        let candidate = Candidate {
            origin: Origin::BuiltIn,
            file_name: (*file_name).to_string(),
            contents: (*contents).to_string(),
        };
        if contents.len() > MAX_PROFILE_BYTES {
            push_rejected(
                candidate,
                None,
                vec![registry_error(
                    "profile_bytes_limit_exceeded",
                    "Built-in Source Profile exceeds the per-Profile byte limit",
                    "",
                    serde_json::json!({
                        "maximumBytes": MAX_PROFILE_BYTES,
                        "actualBytes": contents.len(),
                    }),
                )],
                &mut view,
            );
            builtin_failed = true;
            continue;
        }
        if !admit_candidate(
            candidate,
            &mut seen,
            &mut admitted,
            &mut prepared_detection,
            &mut view,
        ) {
            builtin_failed = true;
        }
    }

    if aggregate_bytes > MAX_AGGREGATE_PROFILE_BYTES {
        push_snapshot_diagnostic(
            &mut view.diagnostics,
            registry_error(
                "aggregate_profile_bytes_limit_exceeded",
                "Built-in Source Profiles exceed the aggregate byte limit",
                "",
                serde_json::json!({
                    "maximumBytes": MAX_AGGREGATE_PROFILE_BYTES,
                    "actualBytes": aggregate_bytes,
                }),
            ),
        );
        builtin_failed = true;
    }

    if builtin_failed {
        return Err(LoadError::invalid_builtin(view.diagnostics));
    }

    let custom_dir = app_data_dir.join("source-profiles");
    let mut custom_paths = match custom_profile_paths(&custom_dir) {
        Ok(paths) => paths,
        Err(diagnostic) => {
            push_snapshot_diagnostic(&mut view.diagnostics, diagnostic);
            Vec::new()
        }
    };
    custom_paths.sort();

    if custom_paths.len() > MAX_CUSTOM_PROFILE_DOCUMENTS {
        push_snapshot_diagnostic(
            &mut view.diagnostics,
            registry_error(
                "custom_profile_document_limit_exceeded",
                "Custom Source Profile document limit exceeded",
                "",
                serde_json::json!({
                    "maximum": MAX_CUSTOM_PROFILE_DOCUMENTS,
                    "actual": custom_paths.len(),
                }),
            ),
        );
        custom_paths.truncate(MAX_CUSTOM_PROFILE_DOCUMENTS);
    }

    for path in custom_paths {
        let file_name = file_name(&path);
        let contents = match read_bounded(&path) {
            Ok(contents) => contents,
            Err(ReadFailure::TooLarge { actual }) => {
                reject_unreadable(
                    &mut view,
                    file_name,
                    registry_error(
                        "profile_bytes_limit_exceeded",
                        "Custom Source Profile exceeds the per-Profile byte limit",
                        "",
                        serde_json::json!({
                            "maximumBytes": MAX_PROFILE_BYTES,
                            "actualBytes": actual,
                        }),
                    ),
                );
                continue;
            }
            Err(ReadFailure::Io(error)) => {
                reject_unreadable(
                    &mut view,
                    file_name.clone(),
                    registry_error(
                        "registry_document_read_error",
                        format!("Could not read Custom Source Profile: {error}"),
                        "",
                        serde_json::json!({ "fileName": file_name.clone() }),
                    ),
                );
                continue;
            }
        };

        if aggregate_bytes.saturating_add(contents.len()) > MAX_AGGREGATE_PROFILE_BYTES {
            reject_unreadable(
                &mut view,
                file_name,
                registry_error(
                    "aggregate_profile_bytes_limit_exceeded",
                    "Installed Source Profiles exceed the aggregate byte limit",
                    "",
                    serde_json::json!({
                        "maximumBytes": MAX_AGGREGATE_PROFILE_BYTES,
                        "acceptedBytes": aggregate_bytes,
                        "documentBytes": contents.len(),
                    }),
                ),
            );
            continue;
        }
        aggregate_bytes += contents.len();

        admit_candidate(
            Candidate {
                origin: Origin::Custom,
                file_name,
                contents,
            },
            &mut seen,
            &mut admitted,
            &mut prepared_detection,
            &mut view,
        );
    }

    Ok(Profiles {
        admitted,
        prepared_detection,
        view,
    })
}

fn admit_candidate(
    candidate: Candidate,
    seen: &mut HashMap<String, (Origin, String)>,
    admitted: &mut Vec<AdmittedProfile>,
    prepared_detection: &mut Vec<source_profile_dsl::detection::CompiledDetectionPlan>,
    view: &mut ProfilesView,
) -> bool {
    let parsed_value = match serde_json::from_str::<Value>(&candidate.contents) {
        Ok(value) => value,
        Err(error) => {
            let diagnostics = vec![schema_error(
                "invalid_document_shape",
                format!("Source Profile document shape is invalid: {error}"),
                "",
                serde_json::json!({
                    "documentKind": "source_profile",
                    "origin": candidate.origin,
                    "fileName": candidate.file_name,
                }),
            )];
            push_rejected(candidate, None, diagnostics, view);
            return false;
        }
    };

    let key = parsed_value
        .get("key")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut diagnostics = validate_basics(&candidate, &parsed_value, key.as_deref());
    if diagnostics.is_empty() {
        if let Some(key) = &key {
            if let Some((existing_origin, existing_file)) = seen.get(key) {
                diagnostics.push(collision_diagnostic(
                    &candidate,
                    key,
                    *existing_origin,
                    existing_file,
                ));
            }
        }
    }

    if !diagnostics.is_empty() {
        push_rejected(candidate, Some(parsed_value), diagnostics, view);
        return false;
    }

    let document = match serde_json::from_value::<SourceProfileDocument>(parsed_value.clone()) {
        Ok(document) => document,
        Err(error) => {
            let diagnostics = vec![schema_error(
                "invalid_document_shape",
                format!("Source Profile document shape is invalid: {error}"),
                "",
                serde_json::json!({
                    "documentKind": "source_profile",
                    "origin": candidate.origin,
                    "fileName": candidate.file_name,
                }),
            )];
            push_rejected(candidate, Some(parsed_value), diagnostics, view);
            return false;
        }
    };

    seen.insert(
        document.key.clone(),
        (candidate.origin, candidate.file_name.clone()),
    );

    diagnostics.extend(authored_diagnostics(&document));
    let plan = match prepare_source_profile_document(&document) {
        Ok(plan) => plan,
        Err(profile_diagnostics) => {
            diagnostics.extend(profile_diagnostics);
            None
        }
    };
    truncate_document_diagnostics(&mut diagnostics, &candidate.file_name);

    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        let profile_view = project_document(
            candidate.origin,
            Admission::Rejected,
            candidate.file_name,
            &document,
        );
        append_profile(view, profile_view);
        append_snapshot_diagnostics(&mut view.diagnostics, diagnostics);
        return false;
    }

    if let Some(plan) = plan {
        prepared_detection.push(plan);
    }
    let profile_view = project_document(
        candidate.origin,
        Admission::Admitted,
        candidate.file_name,
        &document,
    );
    append_profile(view, profile_view);
    append_snapshot_diagnostics(&mut view.diagnostics, diagnostics);
    admitted.push(AdmittedProfile { document });
    true
}

fn validate_basics(candidate: &Candidate, value: &Value, key: Option<&str>) -> Diagnostics {
    let mut diagnostics = Vec::new();
    let schema_version = value.get("schemaVersion").and_then(Value::as_u64);
    if schema_version != Some(3) {
        diagnostics.push(registry_error(
            "unsupported_schema_version",
            "Source Profile uses an unsupported schemaVersion",
            "/schemaVersion",
            serde_json::json!({
                "documentKind": "source_profile",
                "origin": candidate.origin,
                "fileName": candidate.file_name,
                "schemaVersion": schema_version,
                "expectedSchemaVersion": 3,
            }),
        ));
    }
    match key {
        Some(key) if is_technical_key(key) => {}
        _ => diagnostics.push(registry_error(
            "invalid_document_key",
            "Source Profile document key is missing or invalid",
            "/key",
            serde_json::json!({
                "documentKind": "source_profile",
                "origin": candidate.origin,
                "fileName": candidate.file_name,
                "key": key,
                "expectedPattern": "^[a-z0-9_]+$",
            }),
        )),
    }
    if candidate.file_name.strip_suffix(".json") != key {
        diagnostics.push(registry_error(
            "filename_key_mismatch",
            "Source Profile file name must match its key",
            "/key",
            serde_json::json!({
                "documentKind": "source_profile",
                "origin": candidate.origin,
                "fileName": candidate.file_name,
                "key": key,
            }),
        ));
    }
    truncate_document_diagnostics(&mut diagnostics, &candidate.file_name);
    diagnostics
}

fn collision_diagnostic(
    candidate: &Candidate,
    key: &str,
    existing_origin: Origin,
    existing_file: &str,
) -> Diagnostic {
    registry_error(
        "duplicate_source_profile_key",
        format!("Source Profile key `{key}` is already installed"),
        "/key",
        serde_json::json!({
            "sourceProfileKey": key,
            "origin": candidate.origin,
            "fileName": candidate.file_name,
            "existingOrigin": existing_origin,
            "existingFileName": existing_file,
        }),
    )
}

fn custom_profile_paths(directory: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(registry_error(
                "registry_directory_read_error",
                format!("Could not read Custom Source Profile directory: {error}"),
                "",
                serde_json::json!({ "documentKind": "source_profile", "origin": Origin::Custom }),
            ))
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            registry_error(
                "registry_directory_entry_read_error",
                format!("Could not read Custom Source Profile directory entry: {error}"),
                "",
                serde_json::json!({ "documentKind": "source_profile", "origin": Origin::Custom }),
            )
        })?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

enum ReadFailure {
    TooLarge { actual: usize },
    Io(io::Error),
}

fn read_bounded(path: &Path) -> Result<String, ReadFailure> {
    let mut file = File::open(path).map_err(ReadFailure::Io)?;
    let mut bytes = Vec::with_capacity(MAX_PROFILE_BYTES.min(16 * 1024));
    file.by_ref()
        .take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(ReadFailure::Io)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err(ReadFailure::TooLarge {
            actual: bytes.len(),
        });
    }
    String::from_utf8(bytes)
        .map_err(|error| ReadFailure::Io(io::Error::new(io::ErrorKind::InvalidData, error)))
}

fn reject_unreadable(view: &mut ProfilesView, file_name: String, diagnostic: Diagnostic) {
    append_profile(
        view,
        ProfileView {
            origin: Origin::Custom,
            admission: Admission::Rejected,
            file_name,
            definition: None,
        },
    );
    push_snapshot_diagnostic(&mut view.diagnostics, diagnostic);
}

fn push_rejected(
    candidate: Candidate,
    parsed: Option<Value>,
    mut diagnostics: Diagnostics,
    view: &mut ProfilesView,
) {
    truncate_document_diagnostics(&mut diagnostics, &candidate.file_name);
    let projected = parsed
        .and_then(|value| serde_json::from_value::<SourceProfileDocument>(value).ok())
        .map(|document| {
            project_document(
                candidate.origin,
                Admission::Rejected,
                candidate.file_name.clone(),
                &document,
            )
        })
        .unwrap_or_else(|| ProfileView {
            origin: candidate.origin,
            admission: Admission::Rejected,
            file_name: candidate.file_name,
            definition: None,
        });
    append_profile(view, projected);
    append_snapshot_diagnostics(&mut view.diagnostics, diagnostics);
}

fn authored_diagnostics(document: &SourceProfileDocument) -> Diagnostics {
    let mut diagnostics = document.diagnostics.clone().unwrap_or_default();
    for path in &document.access_paths {
        diagnostics.extend(path.diagnostics.clone().unwrap_or_default());
        diagnostics.extend(
            path.discovery
                .strategies
                .iter()
                .flat_map(|strategy| strategy.diagnostics.clone().unwrap_or_default()),
        );
        if let Some(detail) = &path.detail {
            diagnostics.extend(
                detail
                    .strategies
                    .iter()
                    .flat_map(|strategy| strategy.diagnostics.clone().unwrap_or_default()),
            );
        }
    }
    diagnostics
}

fn project_document(
    origin: Origin,
    admission: Admission,
    file_name: String,
    document: &SourceProfileDocument,
) -> ProfileView {
    let mut access_paths = document.access_paths.clone();
    for path in &mut access_paths {
        path.diagnostics = None;
        for strategy in &mut path.discovery.strategies {
            strategy.diagnostics = None;
        }
        if let Some(detail) = &mut path.detail {
            for strategy in &mut detail.strategies {
                strategy.diagnostics = None;
            }
        }
    }
    ProfileView {
        origin,
        admission,
        file_name,
        definition: Some(ProfileDefinitionView {
            key: document.key.clone(),
            name: document.name.clone(),
            kind: document.kind,
            description: document.description.clone(),
            support: document.support.clone(),
            detection: document.detection.clone(),
            source_config_schema: document.source_config_schema.clone(),
            access_paths,
        }),
    }
}

fn append_profile(view: &mut ProfilesView, profile: ProfileView) {
    view.profiles.push(profile);
}

fn append_snapshot_diagnostics(target: &mut Diagnostics, diagnostics: Diagnostics) {
    for diagnostic in diagnostics {
        push_snapshot_diagnostic(target, diagnostic);
    }
}

fn push_snapshot_diagnostic(target: &mut Diagnostics, diagnostic: Diagnostic) {
    if target.len() < MAX_DIAGNOSTICS_PER_SNAPSHOT {
        target.push(diagnostic);
    } else if target
        .last()
        .is_some_and(|last| last.code != "snapshot_diagnostics_truncated")
    {
        target[MAX_DIAGNOSTICS_PER_SNAPSHOT - 1] = registry_error(
            "snapshot_diagnostics_truncated",
            "Installed Source Profile Diagnostics were truncated",
            "",
            serde_json::json!({ "maximum": MAX_DIAGNOSTICS_PER_SNAPSHOT }),
        );
    }
}

fn truncate_document_diagnostics(diagnostics: &mut Diagnostics, file_name: &str) {
    if diagnostics.len() <= MAX_DIAGNOSTICS_PER_DOCUMENT {
        return;
    }
    diagnostics.truncate(MAX_DIAGNOSTICS_PER_DOCUMENT);
    diagnostics[MAX_DIAGNOSTICS_PER_DOCUMENT - 1] = registry_error(
        "profile_diagnostics_truncated",
        "Source Profile Diagnostics were truncated",
        "",
        serde_json::json!({
            "fileName": file_name,
            "maximum": MAX_DIAGNOSTICS_PER_DOCUMENT,
        }),
    );
}

fn registry_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Registry,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(details),
    }
}

fn schema_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Schema,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(details),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.json".to_string())
}

fn is_technical_key(key: &str) -> bool {
    !key.is_empty()
        && key.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_builtin_is_a_typed_fatal_error() {
        let directory = tempfile::tempdir().unwrap();
        let error = load_with_builtins(directory.path(), &[("broken.json", "{}")]).unwrap_err();

        assert_eq!(
            error.kind(),
            super::super::snapshot::LoadErrorKind::InvalidBuiltIn
        );
        assert!(!error.diagnostics().is_empty());
    }
}
