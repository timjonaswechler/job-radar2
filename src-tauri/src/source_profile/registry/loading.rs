use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use source_profile_dsl::definition::{
    compile_source_with_admitted_profiles, CompileSourceOutcome, Diagnostic, DiagnosticCategory,
    DiagnosticSeverity, Diagnostics, SourceDocument,
};
use sources::installed::Profiles;

use crate::source::validation::derive_source_validation_state;

use super::snapshot::{RegistrySource, SourceProfileRegistrySnapshot};

const BUILT_IN_ORIGIN: &str = "built_in";
const CUSTOM_ORIGIN: &str = "custom";
const BUILTIN_SOURCE_JSON_FILES: &[(&str, &str)] = &[];

#[derive(Clone, Debug)]
struct RawSourceDocument {
    origin: &'static str,
    path: String,
    contents: String,
}

pub fn load_snapshot(app_data_dir: impl AsRef<Path>) -> SourceProfileRegistrySnapshot {
    let app_data_dir = app_data_dir.as_ref();
    let profiles = Profiles::load(app_data_dir)
        .expect("embedded Built-in Source Profiles must pass installed admission");
    let mut diagnostics = Vec::new();

    let mut source_documents = embedded_source_documents(BUILTIN_SOURCE_JSON_FILES);
    source_documents.extend(custom_source_documents(
        app_data_dir.join("sources"),
        &mut diagnostics,
    ));

    let source_documents = load_source_documents(source_documents, &mut diagnostics);
    let mut sources = Vec::new();
    for source in source_documents {
        let compile_outcome = compile_source_with_admitted_profiles(&source.document, &profiles);
        let validation_state = derive_source_validation_state(&source.document, &compile_outcome);
        let effective_profile = match &compile_outcome {
            CompileSourceOutcome::Compiled { source, .. } => source.effective_profile().cloned(),
            CompileSourceOutcome::Rejected { .. } => None,
        };
        diagnostics.extend(validation_state.diagnostics.clone());
        sources.push(RegistrySource {
            origin: source.origin,
            path: source.path,
            document: source.document,
            validation_state,
            effective_profile,
            compile_outcome: Some(compile_outcome),
        });
    }

    SourceProfileRegistrySnapshot::new(profiles, sources, diagnostics)
}

fn embedded_source_documents(documents: &[(&str, &str)]) -> Vec<RawSourceDocument> {
    documents
        .iter()
        .map(|(path, contents)| RawSourceDocument {
            origin: BUILT_IN_ORIGIN,
            path: (*path).to_string(),
            contents: (*contents).to_string(),
        })
        .collect()
}

fn custom_source_documents(
    directory: PathBuf,
    diagnostics: &mut Diagnostics,
) -> Vec<RawSourceDocument> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            diagnostics.push(registry_diagnostic(
                "registry_directory_read_error",
                format!("Could not read Source registry directory: {error}"),
                "",
                serde_json::json!({
                    "documentKind": "source",
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
                format!("Could not read Source registry directory entry: {error}"),
                "",
                serde_json::json!({
                    "documentKind": "source",
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
                Ok(contents) => Some(RawSourceDocument {
                    origin: CUSTOM_ORIGIN,
                    path: path_label,
                    contents,
                }),
                Err(error) => {
                    diagnostics.push(registry_diagnostic(
                        "registry_document_read_error",
                        format!("Could not read Source document: {error}"),
                        "",
                        serde_json::json!({
                            "documentKind": "source",
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

#[derive(Clone, Debug)]
struct SourceDocumentEntry {
    origin: String,
    path: String,
    document: SourceDocument,
}

fn load_source_documents(
    documents: Vec<RawSourceDocument>,
    diagnostics: &mut Diagnostics,
) -> Vec<SourceDocumentEntry> {
    let mut sources = Vec::new();
    let mut seen_keys = HashMap::<String, (&'static str, String)>::new();

    for document in documents {
        let Some(parsed) = parse_source_document::<SourceDocument>(&document, diagnostics) else {
            continue;
        };
        if !validate_document_basics(&document, parsed.schema_version, &parsed.key, diagnostics) {
            continue;
        }
        if let Some((first_origin, first_path)) = seen_keys.get(&parsed.key) {
            diagnostics.push(registry_diagnostic(
                "duplicate_source_key",
                format!("Source key `{}` is already defined", parsed.key),
                "/key",
                serde_json::json!({
                    "documentKind": "source",
                    "key": parsed.key,
                    "origin": document.origin,
                    "path": document.path,
                    "existingOrigin": first_origin,
                    "existingPath": first_path,
                }),
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

fn parse_source_document<T>(
    document: &RawSourceDocument,
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
                message: format!("source document shape is invalid: {error}"),
                severity: DiagnosticSeverity::Error,
                path: "".to_string(),
                strategy_key: None,
                details: Some(serde_json::json!({
                    "documentKind": "source",
                    "origin": document.origin,
                    "path": document.path,
                })),
            });
            None
        }
    }
}

fn validate_document_basics(
    document: &RawSourceDocument,
    schema_version: u64,
    key: &str,
    diagnostics: &mut Diagnostics,
) -> bool {
    let mut valid = true;
    if schema_version != 3 {
        diagnostics.push(registry_diagnostic(
            "unsupported_schema_version",
            format!("Source `{key}` uses unsupported schemaVersion `{schema_version}`"),
            "/schemaVersion",
            serde_json::json!({
                "documentKind": "source",
                "origin": document.origin,
                "path": document.path,
                "key": key,
                "schemaVersion": schema_version,
                "expectedSchemaVersion": 3,
            }),
        ));
        valid = false;
    }
    if !is_technical_key(key) {
        diagnostics.push(registry_diagnostic(
            "invalid_document_key",
            format!("Source key `{key}` is invalid"),
            "/key",
            serde_json::json!({
                "documentKind": "source",
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
            format!("Source file name must match key `{key}`"),
            "/key",
            serde_json::json!({
                "documentKind": "source",
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
