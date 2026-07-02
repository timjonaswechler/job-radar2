use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;

use crate::profile_dsl::compiler::ProfileCompilerSnapshot;
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::source::documents::SourceDocument;
use crate::source::validation::derive_source_validation_state;
use crate::source_profile::documents::SourceProfileDocument;

use super::builtins::{
    EmbeddedRegistryDocument, BUILTIN_SOURCE_JSON_FILES, BUILTIN_SOURCE_PROFILE_JSON_FILES,
};
use super::snapshot::{RegistrySource, RegistrySourceProfile, SourceProfileRegistrySnapshot};

const BUILT_IN_ORIGIN: &str = "built_in";
const CUSTOM_ORIGIN: &str = "custom";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistryDocumentKind {
    SourceProfile,
    Source,
}

impl RegistryDocumentKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::SourceProfile => "source_profile",
            Self::Source => "source",
        }
    }
}

#[derive(Clone, Debug)]
struct RawRegistryDocument {
    kind: RegistryDocumentKind,
    origin: &'static str,
    path: String,
    contents: String,
}

pub fn load_snapshot(app_data_dir: impl AsRef<Path>) -> SourceProfileRegistrySnapshot {
    load_snapshot_with_builtins(
        app_data_dir,
        BUILTIN_SOURCE_PROFILE_JSON_FILES,
        BUILTIN_SOURCE_JSON_FILES,
    )
}

pub fn load_snapshot_with_builtins(
    app_data_dir: impl AsRef<Path>,
    builtin_source_profiles: &[EmbeddedRegistryDocument<'_>],
    builtin_sources: &[EmbeddedRegistryDocument<'_>],
) -> SourceProfileRegistrySnapshot {
    let app_data_dir = app_data_dir.as_ref();
    let mut diagnostics = Vec::new();

    let mut profile_documents =
        embedded_documents(RegistryDocumentKind::SourceProfile, builtin_source_profiles);
    profile_documents.extend(custom_documents(
        RegistryDocumentKind::SourceProfile,
        app_data_dir.join("source-profiles"),
        &mut diagnostics,
    ));

    let mut source_documents = embedded_documents(RegistryDocumentKind::Source, builtin_sources);
    source_documents.extend(custom_documents(
        RegistryDocumentKind::Source,
        app_data_dir.join("sources"),
        &mut diagnostics,
    ));

    let profiles = load_profile_documents(profile_documents, &mut diagnostics);
    let source_documents = load_source_documents(source_documents, &mut diagnostics);
    let compiler_snapshot = ProfileCompilerSnapshot {
        profiles: profiles
            .iter()
            .map(|profile| profile.document.clone())
            .collect(),
        sources: source_documents
            .iter()
            .map(|source| source.document.clone())
            .collect(),
    };

    let mut sources = Vec::new();
    for source in source_documents {
        let validation_state =
            derive_source_validation_state(&compiler_snapshot, &source.document.key);
        diagnostics.extend(validation_state.diagnostics.clone());
        sources.push(RegistrySource {
            origin: source.origin,
            path: source.path,
            document: source.document,
            validation_state,
        });
    }

    SourceProfileRegistrySnapshot {
        profiles,
        sources,
        diagnostics,
    }
}

fn embedded_documents(
    kind: RegistryDocumentKind,
    documents: &[EmbeddedRegistryDocument<'_>],
) -> Vec<RawRegistryDocument> {
    documents
        .iter()
        .map(|(path, contents)| RawRegistryDocument {
            kind,
            origin: BUILT_IN_ORIGIN,
            path: (*path).to_string(),
            contents: (*contents).to_string(),
        })
        .collect()
}

fn custom_documents(
    kind: RegistryDocumentKind,
    directory: PathBuf,
    diagnostics: &mut Diagnostics,
) -> Vec<RawRegistryDocument> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            diagnostics.push(registry_diagnostic(
                "registry_directory_read_error",
                format!("Could not read registry directory: {error}"),
                "",
                serde_json::json!({
                    "documentKind": kind.as_str(),
                    "origin": CUSTOM_ORIGIN,
                    "path": directory.display().to_string(),
                }),
            ));
            return Vec::new();
        }
    };

    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() && is_json_file(&path) {
                    paths.push(path);
                }
            }
            Err(error) => diagnostics.push(registry_diagnostic(
                "registry_directory_entry_read_error",
                format!("Could not read registry directory entry: {error}"),
                "",
                serde_json::json!({
                    "documentKind": kind.as_str(),
                    "origin": CUSTOM_ORIGIN,
                    "path": directory.display().to_string(),
                }),
            )),
        }
    }

    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let path_label = path.display().to_string();
            match fs::read_to_string(&path) {
                Ok(contents) => Some(RawRegistryDocument {
                    kind,
                    origin: CUSTOM_ORIGIN,
                    path: path_label,
                    contents,
                }),
                Err(error) => {
                    diagnostics.push(registry_diagnostic(
                        "registry_document_read_error",
                        format!("Could not read registry document: {error}"),
                        "",
                        serde_json::json!({
                            "documentKind": kind.as_str(),
                            "origin": CUSTOM_ORIGIN,
                            "path": path_label,
                        }),
                    ));
                    None
                }
            }
        })
        .collect()
}

fn is_json_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("json")
    )
}

fn load_profile_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Diagnostics,
) -> Vec<RegistrySourceProfile> {
    let mut profiles = Vec::new();
    let mut seen_keys = HashMap::<String, (&'static str, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document::<SourceProfileDocument>(&document, diagnostics)
        else {
            continue;
        };
        if !validate_document_basics(&document, parsed.schema_version, &parsed.key, diagnostics) {
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        profiles.push(RegistrySourceProfile {
            origin: document.origin.to_string(),
            path: document.path,
            document: parsed,
        });
    }

    profiles
}

#[derive(Clone, Debug)]
struct SourceDocumentEntry {
    origin: String,
    path: String,
    document: SourceDocument,
}

fn load_source_documents(
    documents: Vec<RawRegistryDocument>,
    diagnostics: &mut Diagnostics,
) -> Vec<SourceDocumentEntry> {
    let mut sources = Vec::new();
    let mut seen_keys = HashMap::<String, (&'static str, String)>::new();

    for document in documents {
        let Some(parsed) = parse_registry_document::<SourceDocument>(&document, diagnostics) else {
            continue;
        };
        if !validate_document_basics(&document, parsed.schema_version, &parsed.key, diagnostics) {
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(duplicate_key_diagnostic(
                &document,
                &parsed.key,
                *first_origin,
                first_path,
            ));
            continue;
        }

        seen_keys.insert(parsed.key.clone(), (document.origin, document.path.clone()));
        sources.push(SourceDocumentEntry {
            origin: document.origin.to_string(),
            path: document.path,
            document: parsed,
        });
    }

    sources
}

fn parse_registry_document<T>(
    document: &RawRegistryDocument,
    diagnostics: &mut Diagnostics,
) -> Option<T>
where
    T: DeserializeOwned,
{
    match serde_json::from_str::<T>(&document.contents) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            diagnostics.push(Diagnostic {
                category: DiagnosticCategory::Schema,
                code: "invalid_document_shape".to_string(),
                message: format!(
                    "{} document shape is invalid: {error}",
                    document.kind.as_str()
                ),
                severity: DiagnosticSeverity::Error,
                path: "".to_string(),
                strategy_key: None,
                details: Some(serde_json::json!({
                    "documentKind": document.kind.as_str(),
                    "origin": document.origin,
                    "path": document.path,
                })),
            });
            None
        }
    }
}

fn validate_document_basics(
    document: &RawRegistryDocument,
    schema_version: u64,
    key: &str,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    if schema_version != 2 {
        diagnostics.push(registry_diagnostic(
            "unsupported_schema_version",
            format!(
                "{} document `{key}` uses unsupported schemaVersion `{schema_version}`",
                document.kind.as_str()
            ),
            "/schemaVersion",
            serde_json::json!({
                "documentKind": document.kind.as_str(),
                "origin": document.origin,
                "path": document.path,
                "key": key,
                "schemaVersion": schema_version,
                "expectedSchemaVersion": 2,
            }),
        ));
        valid = false;
    }
    if !is_technical_key(key) {
        diagnostics.push(registry_diagnostic(
            "invalid_document_key",
            format!("{} document key `{key}` is invalid", document.kind.as_str()),
            "/key",
            serde_json::json!({
                "documentKind": document.kind.as_str(),
                "origin": document.origin,
                "path": document.path,
                "key": key,
                "expectedPattern": "^[a-z0-9_]+$",
            }),
        ));
        valid = false;
    }
    if filename_key(&document.path).as_deref() != Some(key) {
        diagnostics.push(registry_diagnostic(
            "filename_key_mismatch",
            format!(
                "{} document file name must match key `{key}`",
                document.kind.as_str()
            ),
            "/key",
            serde_json::json!({
                "documentKind": document.kind.as_str(),
                "origin": document.origin,
                "path": document.path,
                "key": key,
                "fileKey": filename_key(&document.path),
            }),
        ));
        valid = false;
    }

    valid
}

fn duplicate_key_diagnostic(
    document: &RawRegistryDocument,
    key: &str,
    first_origin: &'static str,
    first_path: &str,
) -> Diagnostic {
    if document.kind == RegistryDocumentKind::SourceProfile
        && first_origin == BUILT_IN_ORIGIN
        && document.origin == CUSTOM_ORIGIN
    {
        return Diagnostic::duplicate_builtin_custom_source_profile_key(key);
    }

    registry_diagnostic(
        match document.kind {
            RegistryDocumentKind::SourceProfile => "duplicate_source_profile_key",
            RegistryDocumentKind::Source => "duplicate_source_key",
        },
        format!(
            "{} key `{key}` is already defined by another registry document",
            document.kind.as_str()
        ),
        "/key",
        serde_json::json!({
            "documentKind": document.kind.as_str(),
            "key": key,
            "origin": document.origin,
            "path": document.path,
            "existingOrigin": first_origin,
            "existingPath": first_path,
        }),
    )
}

fn registry_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
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

fn filename_key(path: &str) -> Option<String> {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
}

fn is_technical_key(key: &str) -> bool {
    !key.is_empty()
        && key.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}
