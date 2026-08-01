use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::Serialize;

use super::{
    limits::{MAX_AGGREGATE_SOURCE_BYTES, MAX_CUSTOM_SOURCE_DOCUMENTS, MAX_SOURCE_BYTES},
    loading, persistence,
    snapshot::{Generation, Origin, Snapshot, SourceView},
    sources::{
        authored_violations, is_key, CreateDraft, InactiveStatus, Revision, SourceDocument,
        SourceStatus,
    },
};

#[derive(Clone)]
pub struct Store {
    app_data_dir: PathBuf,
    coordinator: Arc<Mutex<()>>,
}

impl Store {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        Self {
            app_data_dir: app_data_dir.into(),
            coordinator: Arc::new(Mutex::new(())),
        }
    }

    /// Loads fresh disk state for one operation. Store does not cache documents.
    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        let _guard = self
            .coordinator
            .lock()
            .map_err(|_| Error::storage("installed Source coordinator is poisoned"))?;
        self.load()
    }

    pub fn create(&self, draft: CreateDraft) -> Result<SourceView, Error> {
        validate_key(&draft.key)?;
        let document = draft.into_document();
        validate_authored(&document)?;
        let bytes = serialized_document(&document)?;
        let _guard = self.lock()?;
        self.preflight_write(&bytes, None, true)?;
        let snapshot = self.load()?;
        if snapshot.source(&document.key).is_some() {
            return Err(Error::duplicate(&document.key));
        }
        let path = self.source_path(&document.key);
        if path.exists() {
            return Err(Error::duplicate(&document.key));
        }
        write(&document, &bytes, &path, false)?;
        self.saved(&document.key)
    }

    pub fn revise(&self, revision: Revision) -> Result<SourceView, Error> {
        validate_key(&revision.key)?;
        let _guard = self.lock()?;
        let snapshot = self.load()?;
        let existing = snapshot
            .source(&revision.key)
            .ok_or_else(|| Error::not_found(&revision.key))?;
        ensure_custom(existing.origin(), &revision.key)?;
        let document = revision.into_document(existing.document().status);
        validate_authored(&document)?;
        let bytes = serialized_document(&document)?;
        self.preflight_write(&bytes, Some(&existing.path), false)?;
        write(&document, &bytes, &existing.path, true)?;
        self.saved(&document.key)
    }

    pub fn set_inactive(&self, key: &str, status: InactiveStatus) -> Result<SourceView, Error> {
        validate_key(key)?;
        let _guard = self.lock()?;
        let snapshot = self.load()?;
        let existing = snapshot.source(key).ok_or_else(|| Error::not_found(key))?;
        ensure_custom(existing.origin(), key)?;
        let mut document = existing.document().clone();
        document.status = status.into();
        let bytes = serialized_document(&document)?;
        self.preflight_write(&bytes, Some(&existing.path), false)?;
        write(&document, &bytes, &existing.path, true)?;
        self.saved(key)
    }

    /// Temporary checked admission seam for Desktop Live Check. The exact
    /// Source/Profile generation is compared while holding the same coordinator
    /// used by every application-mediated Source mutation, immediately before
    /// atomic replacement.
    pub fn admit_checked(&self, key: &str, checked: &Generation) -> Result<SourceView, Error> {
        validate_key(key)?;
        let _guard = self.lock()?;
        let snapshot = self.load()?;
        let existing = snapshot.source(key).ok_or_else(|| Error::not_found(key))?;
        ensure_custom(existing.origin(), key)?;
        if existing.generation() != checked {
            return Err(Error::generation_mismatch(key));
        }
        if existing.document().status == SourceStatus::Active {
            return Err(Error::invalid_lifecycle(
                "an active Source cannot be checked-admitted",
            ));
        }
        let mut document = existing.document().clone();
        document.status = SourceStatus::Active;
        let bytes = serialized_document(&document)?;
        self.preflight_write(&bytes, Some(&existing.path), false)?;
        write(&document, &bytes, &existing.path, true)?;
        self.saved(key)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, Error> {
        self.coordinator
            .lock()
            .map_err(|_| Error::storage("installed Source coordinator is poisoned"))
    }
    fn load(&self) -> Result<Snapshot, Error> {
        Snapshot::load(&self.app_data_dir).map_err(Error::load)
    }
    fn saved(&self, key: &str) -> Result<SourceView, Error> {
        self.load()?
            .view()
            .sources
            .iter()
            .find(|source| source.document.key == key)
            .cloned()
            .ok_or_else(|| Error::not_found(key))
    }
    fn source_path(&self, key: &str) -> PathBuf {
        self.app_data_dir
            .join("sources")
            .join(format!("{key}.json"))
    }

    fn preflight_write(
        &self,
        bytes: &[u8],
        replacing: Option<&Path>,
        creating: bool,
    ) -> Result<(), Error> {
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(Error::limit(format!(
                "Source document exceeds the {MAX_SOURCE_BYTES}-byte per-Source limit"
            )));
        }
        let directory = self.app_data_dir.join("sources");
        let usage = loading::storage_usage(&directory, replacing)
            .map_err(|diagnostic| Error::storage(diagnostic.message))?;
        if creating && usage.document_count >= MAX_CUSTOM_SOURCE_DOCUMENTS {
            return Err(Error::limit(format!(
                "Custom Source document limit of {MAX_CUSTOM_SOURCE_DOCUMENTS} is reached"
            )));
        }
        if usage.bytes_excluding_replaced.saturating_add(bytes.len()) > MAX_AGGREGATE_SOURCE_BYTES {
            return Err(Error::limit(format!(
                "Installed Sources would exceed the {MAX_AGGREGATE_SOURCE_BYTES}-byte aggregate limit"
            )));
        }
        Ok(())
    }
}

fn serialized_document(document: &SourceDocument) -> Result<Vec<u8>, Error> {
    let mut bytes =
        serde_json::to_vec_pretty(document).map_err(|error| Error::storage(error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write(document: &SourceDocument, bytes: &[u8], path: &Path, replace: bool) -> Result<(), Error> {
    let directory = path
        .parent()
        .ok_or_else(|| Error::storage("Source document path has no parent"))?;
    std::fs::create_dir_all(directory).map_err(|error| Error::storage(error.to_string()))?;
    if replace && !path.exists() {
        return Err(Error::not_found(&document.key));
    }
    if !replace && path.exists() {
        return Err(Error::duplicate(&document.key));
    }
    persistence::replace(path, bytes).map_err(|error| Error::storage(error.to_string()))
}

fn validate_key(key: &str) -> Result<(), Error> {
    if is_key(key) {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::InvalidKey,
            format!("invalid Source key `{key}`"),
        ))
    }
}

fn validate_authored(document: &SourceDocument) -> Result<(), Error> {
    let violations = authored_violations(document);
    if violations.is_empty() {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::InvalidInput,
        violations
            .into_iter()
            .map(|violation| violation.message)
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

fn ensure_custom(origin: Origin, key: &str) -> Result<(), Error> {
    if origin == Origin::Custom {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::BuiltIn,
            format!("built-in Source `{key}` cannot be mutated"),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidKey,
    InvalidInput,
    Duplicate,
    NotFound,
    BuiltIn,
    InvalidLifecycle,
    GenerationMismatch,
    LimitExceeded,
    Storage,
    Load,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn duplicate(key: &str) -> Self {
        Self::new(
            ErrorKind::Duplicate,
            format!("Source `{key}` already exists"),
        )
    }
    fn not_found(key: &str) -> Self {
        Self::new(ErrorKind::NotFound, format!("Source `{key}` was not found"))
    }
    fn invalid_lifecycle(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidLifecycle, message)
    }
    fn generation_mismatch(key: &str) -> Self {
        Self::new(
            ErrorKind::GenerationMismatch,
            format!("Source `{key}` changed after the checked preparation was captured"),
        )
    }
    fn limit(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::LimitExceeded, message)
    }
    fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, message)
    }
    fn load(error: super::snapshot::LoadError) -> Self {
        Self::new(ErrorKind::Load, error.to_string())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
