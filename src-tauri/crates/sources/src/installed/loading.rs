use std::{
    collections::{BinaryHeap, HashMap},
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use source_engine::definition::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics, SelectedAccessPath,
};

use super::{
    limits::{
        MAX_AGGREGATE_SOURCE_BYTES, MAX_CUSTOM_SOURCE_DOCUMENTS, MAX_SOURCE_BYTES,
        MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT, MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT,
    },
    preparation,
    snapshot::{Generation, Origin, PreparedSource, Profiles, Snapshot, SnapshotView, SourceView},
    sources::{authored_violations, is_key, SourceDocument},
};

pub(super) fn load(app_data_dir: &Path, profiles: Profiles) -> Snapshot {
    let mut diagnostics = Vec::new();
    let mut prepared = Vec::new();
    let mut aggregate = 0usize;
    let directory = app_data_dir.join("sources");
    let selection = match json_paths(&directory) {
        Ok(selection) => selection,
        Err(diagnostic) => {
            push(&mut diagnostics, diagnostic);
            PathSelection::default()
        }
    };
    if selection.total > MAX_CUSTOM_SOURCE_DOCUMENTS {
        push(
            &mut diagnostics,
            registry_error(
                "custom_source_document_limit_exceeded",
                "Custom Source document limit exceeded",
                "",
                serde_json::json!({"maximum": MAX_CUSTOM_SOURCE_DOCUMENTS, "actual": selection.total}),
            ),
        );
    }
    let mut seen = HashMap::<String, String>::new();
    for path in selection.paths {
        let file_name = file_name(&path);
        let remaining = MAX_AGGREGATE_SOURCE_BYTES - aggregate;
        if remaining == 0 {
            push(
                &mut diagnostics,
                aggregate_limit_diagnostic(&file_name, aggregate, None),
            );
            break;
        }
        let contents = match read_bounded(&path, remaining) {
            Ok(read) => {
                aggregate = aggregate.saturating_add(read.charged_bytes);
                read.contents
            }
            Err(ReadFailure::TooLarge {
                observed_at_least,
                charged_bytes,
            }) => {
                aggregate = aggregate.saturating_add(charged_bytes);
                push(
                    &mut diagnostics,
                    registry_error(
                        "source_bytes_limit_exceeded",
                        "Custom Source exceeds the per-Source byte limit",
                        "",
                        serde_json::json!({"fileName": file_name, "maximumBytes": MAX_SOURCE_BYTES, "actualBytesAtLeast": observed_at_least}),
                    ),
                );
                continue;
            }
            Err(ReadFailure::AggregateExceeded {
                observed_at_least,
                charged_bytes,
            }) => {
                aggregate = aggregate.saturating_add(charged_bytes);
                push(
                    &mut diagnostics,
                    aggregate_limit_diagnostic(&file_name, aggregate, Some(observed_at_least)),
                );
                break;
            }
            Err(ReadFailure::Io {
                error,
                charged_bytes,
            }) => {
                aggregate = aggregate.saturating_add(charged_bytes);
                push(
                    &mut diagnostics,
                    registry_error(
                        "registry_document_read_error",
                        format!("Could not read Custom Source: {error}"),
                        "",
                        serde_json::json!({"fileName": file_name}),
                    ),
                );
                continue;
            }
        };
        let mut document_diagnostics = Vec::new();
        let value = match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(value) => value,
            Err(error) => {
                document_diagnostics.push(schema_error(
                    "invalid_document_shape",
                    format!("Source document shape is invalid: {error}"),
                    "",
                    serde_json::json!({"fileName": file_name}),
                ));
                append_document(&mut diagnostics, document_diagnostics, &file_name);
                continue;
            }
        };
        let key = value.get("key").and_then(|value| value.as_str());
        validate_basics(
            key,
            &file_name,
            value.get("schemaVersion").and_then(|value| value.as_u64()),
            &mut document_diagnostics,
        );
        if let Some(key) = key {
            if let Some(existing) = seen.get(key) {
                document_diagnostics.push(registry_error("duplicate_source_key",
                    format!("Source key `{key}` is already installed"), "/key",
                    serde_json::json!({"sourceKey": key, "fileName": file_name, "existingFileName": existing})));
            }
        }
        if !document_diagnostics.is_empty() {
            append_document(&mut diagnostics, document_diagnostics, &file_name);
            continue;
        }
        let document = match serde_json::from_value::<SourceDocument>(value) {
            Ok(document) => document,
            Err(error) => {
                append_document(
                    &mut diagnostics,
                    vec![schema_error(
                        "invalid_document_shape",
                        format!("Source document shape is invalid: {error}"),
                        "",
                        serde_json::json!({"fileName": file_name}),
                    )],
                    &file_name,
                );
                continue;
            }
        };
        let authored = authored_violations(&document)
            .into_iter()
            .map(|violation| {
                schema_error(
                    violation.code,
                    violation.message,
                    violation.path,
                    serde_json::json!({"fileName": file_name}),
                )
            })
            .collect::<Vec<_>>();
        if !authored.is_empty() {
            append_document(&mut diagnostics, authored, &file_name);
            continue;
        }
        seen.insert(document.key.clone(), file_name.clone());
        let (outcome, validation, resolved) = preparation::prepare(&document, &profiles);
        let generation = generation(&document, &profiles);
        let mut owned = document.diagnostics.clone().unwrap_or_default();
        owned.extend(validation.diagnostics.clone());
        append_document(&mut diagnostics, owned, &file_name);
        prepared.push(PreparedSource {
            origin: Origin::Custom,
            file_name,
            path,
            document,
            validation,
            outcome,
            generation,
            resolved,
        });
    }
    let views = prepared
        .iter()
        .map(|source| {
            let mut document = source.document.clone();
            document.diagnostics = None;
            SourceView {
                origin: source.origin,
                file_name: source.file_name.clone(),
                document,
                validation_state: source.validation.clone(),
                resolved: source.resolved.clone(),
            }
        })
        .collect();
    Snapshot {
        view: SnapshotView {
            profiles: profiles.view().clone(),
            sources: views,
            diagnostics,
        },
        profiles,
        sources: prepared,
    }
}

pub(crate) fn generation(document: &SourceDocument, profiles: &Profiles) -> Generation {
    let profile = match &document.selected_access_path {
        SelectedAccessPath::ProfileAccessPath { profile_key, .. } => profiles.profile(profile_key),
        SelectedAccessPath::SourceOwnedAccessPath { .. } => None,
    };
    let bytes = serde_json::to_vec(&(document, profile))
        .expect("Source generation material must serialize");
    Generation(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_basics(
    key: Option<&str>,
    file_name: &str,
    version: Option<u64>,
    diagnostics: &mut Diagnostics,
) {
    if version != Some(3) {
        diagnostics.push(registry_error("unsupported_schema_version", "Source uses an unsupported schemaVersion", "/schemaVersion",
        serde_json::json!({"fileName": file_name, "schemaVersion": version, "expectedSchemaVersion": 3})));
    }
    if !key.is_some_and(is_key) {
        diagnostics.push(registry_error("invalid_document_key", "Source key is missing or invalid", "/key",
        serde_json::json!({"fileName": file_name, "key": key, "expectedPattern": "^[a-z0-9][a-z0-9_]*$"})));
    }
    if file_name.strip_suffix(".json") != key {
        diagnostics.push(registry_error(
            "filename_key_mismatch",
            "Source file name must match its key",
            "/key",
            serde_json::json!({"fileName": file_name, "key": key}),
        ));
    }
}

#[derive(Default)]
struct PathSelection {
    paths: Vec<PathBuf>,
    total: usize,
}

pub(super) struct StorageUsage {
    pub document_count: usize,
    pub bytes_excluding_replaced: usize,
}

pub(super) fn storage_usage(
    directory: &Path,
    replacing: Option<&Path>,
) -> Result<StorageUsage, Diagnostic> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StorageUsage {
                document_count: 0,
                bytes_excluding_replaced: 0,
            })
        }
        Err(error) => {
            return Err(registry_error(
                "registry_directory_read_error",
                format!("Could not read Custom Source directory: {error}"),
                "",
                serde_json::json!({}),
            ))
        }
    };
    let mut document_count = 0usize;
    let mut bytes_excluding_replaced = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| {
            registry_error(
                "registry_directory_entry_read_error",
                format!("Could not read Source directory entry: {error}"),
                "",
                serde_json::json!({}),
            )
        })?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| {
            registry_error(
                "registry_document_metadata_error",
                format!("Could not inspect Custom Source metadata: {error}"),
                "",
                serde_json::json!({"fileName": file_name(&path)}),
            )
        })?;
        if !metadata.is_file() {
            continue;
        }
        document_count = document_count.saturating_add(1);
        if replacing != Some(path.as_path()) {
            let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
            bytes_excluding_replaced = bytes_excluding_replaced.saturating_add(length);
        }
    }
    Ok(StorageUsage {
        document_count,
        bytes_excluding_replaced,
    })
}

fn json_paths(directory: &Path) -> Result<PathSelection, Diagnostic> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PathSelection::default())
        }
        Err(error) => {
            return Err(registry_error(
                "registry_directory_read_error",
                format!("Could not read Custom Source directory: {error}"),
                "",
                serde_json::json!({}),
            ))
        }
    };
    let mut paths = BinaryHeap::with_capacity(MAX_CUSTOM_SOURCE_DOCUMENTS);
    let mut total = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| {
            registry_error(
                "registry_directory_entry_read_error",
                format!("Could not read Source directory entry: {error}"),
                "",
                serde_json::json!({}),
            )
        })?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("json"))
        {
            total = total.saturating_add(1);
            if paths.len() < MAX_CUSTOM_SOURCE_DOCUMENTS {
                paths.push(path);
            } else if paths.peek().is_some_and(|largest| path < *largest) {
                paths.pop();
                paths.push(path);
            }
        }
    }
    let mut paths = paths.into_vec();
    paths.sort();
    Ok(PathSelection { paths, total })
}

struct BoundedRead {
    contents: String,
    charged_bytes: usize,
}

enum ReadFailure {
    TooLarge {
        observed_at_least: usize,
        charged_bytes: usize,
    },
    AggregateExceeded {
        observed_at_least: usize,
        charged_bytes: usize,
    },
    Io {
        error: io::Error,
        charged_bytes: usize,
    },
}

fn read_bounded(path: &Path, remaining_aggregate: usize) -> Result<BoundedRead, ReadFailure> {
    let limit = MAX_SOURCE_BYTES.min(remaining_aggregate);
    let mut file = File::open(path).map_err(|error| ReadFailure::Io {
        error,
        charged_bytes: 0,
    })?;
    let mut bytes = Vec::with_capacity(limit.min(16 * 1024));
    if let Err(error) = file
        .by_ref()
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
    {
        return Err(ReadFailure::Io {
            error,
            charged_bytes: bytes.len(),
        });
    }
    let charged_bytes = bytes.len();
    if charged_bytes > limit {
        return if limit < MAX_SOURCE_BYTES {
            Err(ReadFailure::AggregateExceeded {
                observed_at_least: charged_bytes,
                charged_bytes,
            })
        } else {
            Err(ReadFailure::TooLarge {
                observed_at_least: charged_bytes,
                charged_bytes,
            })
        };
    }
    match String::from_utf8(bytes) {
        Ok(contents) => Ok(BoundedRead {
            contents,
            charged_bytes,
        }),
        Err(error) => Err(ReadFailure::Io {
            error: io::Error::new(io::ErrorKind::InvalidData, error),
            charged_bytes,
        }),
    }
}
fn aggregate_limit_diagnostic(
    file_name: &str,
    accepted_bytes: usize,
    document_bytes_at_least: Option<usize>,
) -> Diagnostic {
    registry_error(
        "aggregate_source_bytes_limit_exceeded",
        "Installed Sources exceed the aggregate byte limit",
        "",
        serde_json::json!({
            "fileName": file_name,
            "maximumBytes": MAX_AGGREGATE_SOURCE_BYTES,
            "acceptedBytes": accepted_bytes,
            "documentBytesAtLeast": document_bytes_at_least,
        }),
    )
}

fn append_document(target: &mut Diagnostics, mut items: Diagnostics, file_name: &str) {
    if items.len() > MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT {
        items.truncate(MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT);
        items[MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT - 1] = registry_error(
            "source_diagnostics_truncated",
            "Source Diagnostics were truncated",
            "",
            serde_json::json!({"fileName": file_name, "maximum": MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT}),
        );
    }
    for item in items {
        push(target, item);
    }
}
fn push(target: &mut Diagnostics, item: Diagnostic) {
    if target.len() < MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT {
        target.push(item);
    } else if target
        .last()
        .is_some_and(|last| last.code != "source_snapshot_diagnostics_truncated")
    {
        target[MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT - 1] = registry_error(
            "source_snapshot_diagnostics_truncated",
            "Installed Source Diagnostics were truncated",
            "",
            serde_json::json!({"maximum": MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT}),
        );
    }
}
fn registry_error(
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
fn schema_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
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
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.json".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_prepares_each_admitted_source_exactly_once() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("source-profiles")).unwrap();
        fs::create_dir_all(root.path().join("sources")).unwrap();
        fs::write(
            root.path().join("source-profiles/example_jobs.json"),
            include_str!(
                "../../../../tests/fixtures/source-behavior/valid/simple-source-profile.json"
            ),
        )
        .unwrap();
        fs::write(
            root.path().join("sources/example_source.json"),
            include_str!("../../tests/fixtures/sources/valid/source-selecting-access-path.json"),
        )
        .unwrap();
        fs::write(
            root.path().join("sources/owned_source.json"),
            include_str!("../../tests/fixtures/sources/valid/source-owned-access-path.json"),
        )
        .unwrap();

        super::super::preparation::reset_preparation_calls();
        let profiles = Profiles::load(root.path()).unwrap();
        let snapshot = load(root.path(), profiles);

        assert_eq!(snapshot.sources.len(), 2);
        assert_eq!(super::super::preparation::preparation_calls(), 2);
    }
}
