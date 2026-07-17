// This module is integrated by the follow-up authentication/provider tickets.
#![allow(dead_code)]

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STORAGE_DIRECTORY: &str = "agents";
const LEGACY_STORAGE_DIRECTORY: &str = "agent";
const AUTH_FILE: &str = "auth.json";
const LOCK_FILE: &str = "auth.lock";
const TEMP_FILE: &str = "auth.json.tmp";
const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthStatus {
    NotConfigured,
    Configured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthStorageErrorCategory {
    InvalidConfiguration,
    MigrationConflict,
    Unavailable,
    RefreshFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthStorageError {
    pub category: AuthStorageErrorCategory,
    pub message: &'static str,
}

impl AuthStorageError {
    pub(super) fn invalid_configuration() -> Self {
        Self {
            category: AuthStorageErrorCategory::InvalidConfiguration,
            message: "authentication storage is not securely configured",
        }
    }

    fn unavailable() -> Self {
        Self {
            category: AuthStorageErrorCategory::Unavailable,
            message: "authentication storage is unavailable",
        }
    }

    fn migration_conflict() -> Self {
        Self {
            category: AuthStorageErrorCategory::MigrationConflict,
            message: "conflicting authentication storage locations require review",
        }
    }

    pub(super) fn refresh_failed() -> Self {
        Self {
            category: AuthStorageErrorCategory::RefreshFailed,
            message: "authentication refresh failed",
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthCredential {
    #[serde(rename = "type")]
    credential_type: OAuthCredentialType,
    pub(crate) access: String,
    pub(crate) refresh: String,
    #[serde(rename = "expires")]
    pub(crate) expires_at_ms: u64,
    pub(crate) account_id: String,
}

impl OAuthCredential {
    pub(crate) fn new(
        access: String,
        refresh: String,
        expires_at_ms: u64,
        account_id: String,
    ) -> Self {
        Self {
            credential_type: OAuthCredentialType::OAuth,
            access,
            refresh,
            expires_at_ms,
            account_id,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OAuthCredentialType {
    OAuth,
}

#[derive(Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct AuthDocument(BTreeMap<String, OAuthCredential>);

pub(crate) struct AuthStorage {
    directory: PathBuf,
    auth_path: PathBuf,
    lock_path: PathBuf,
}

impl AuthStorage {
    pub(crate) fn for_current_user() -> Result<Self, AuthStorageError> {
        #[cfg(target_os = "macos")]
        {
            let location = crate::app::paths::current_user_app_data_location()
                .map_err(|_| AuthStorageError::invalid_configuration())?;
            Self::in_agents_data_root_from(
                &location.trusted_ancestor,
                &location.root.join(STORAGE_DIRECTORY),
            )
        }

        #[cfg(not(target_os = "macos"))]
        Err(AuthStorageError::invalid_configuration())
    }

    #[cfg(test)]
    pub(super) fn for_test_app_data(app_data_dir: &Path) -> Result<Self, AuthStorageError> {
        Self::in_app_data_dir(app_data_dir)
    }

    fn in_app_data_dir(app_data_dir: &Path) -> Result<Self, AuthStorageError> {
        let trusted_ancestor = app_data_dir
            .parent()
            .ok_or_else(AuthStorageError::invalid_configuration)?;
        Self::in_app_data_dir_from(trusted_ancestor, app_data_dir)
    }

    fn in_app_data_dir_from(
        trusted_ancestor: &Path,
        app_data_dir: &Path,
    ) -> Result<Self, AuthStorageError> {
        Self::in_agents_data_root_from(trusted_ancestor, &app_data_dir.join(STORAGE_DIRECTORY))
    }

    pub(crate) fn in_agents_data_root(agents_data_root: &Path) -> Result<Self, AuthStorageError> {
        let app_data_dir = agents_data_root
            .parent()
            .ok_or_else(AuthStorageError::invalid_configuration)?;
        let trusted_ancestor = app_data_dir
            .parent()
            .ok_or_else(AuthStorageError::invalid_configuration)?;
        Self::in_agents_data_root_from(trusted_ancestor, agents_data_root)
    }

    fn in_agents_data_root_from(
        trusted_ancestor: &Path,
        agents_data_root: &Path,
    ) -> Result<Self, AuthStorageError> {
        let app_data_dir = agents_data_root
            .parent()
            .ok_or_else(AuthStorageError::invalid_configuration)?;
        if agents_data_root.file_name() != Some(std::ffi::OsStr::new(STORAGE_DIRECTORY))
            || !trusted_ancestor.is_absolute()
            || !trusted_directory_is_real(trusted_ancestor)?
            || !app_data_dir.is_absolute()
            || !app_data_dir.starts_with(trusted_ancestor)
            || path_is_inside_repository(app_data_dir)
            || canonical_existing_prefix_is_inside_repository(app_data_dir)
            || path_below_ancestor_contains_symlink(trusted_ancestor, app_data_dir)?
        {
            return Err(AuthStorageError::invalid_configuration());
        }

        create_private_directory(agents_data_root)?;

        let storage = Self {
            auth_path: agents_data_root.join(AUTH_FILE),
            lock_path: agents_data_root.join(LOCK_FILE),
            directory: agents_data_root.to_owned(),
        };
        let lock = storage.open_lock()?;
        lock.lock_exclusive()
            .map_err(|_| AuthStorageError::unavailable())?;
        storage.initialize_or_migrate(app_data_dir)?;
        FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
        Ok(storage)
    }

    fn initialize_or_migrate(&self, app_data_dir: &Path) -> Result<(), AuthStorageError> {
        let legacy_directory = app_data_dir.join(LEGACY_STORAGE_DIRECTORY);
        let legacy_auth_path = legacy_directory.join(AUTH_FILE);
        let canonical_exists = private_regular_file_exists(&self.auth_path)?;
        let legacy_directory_exists = private_directory_exists(&legacy_directory)?;
        let legacy_exists =
            legacy_directory_exists && private_regular_file_exists(&legacy_auth_path)?;

        if canonical_exists && legacy_exists {
            ensure_private_regular_file(&self.auth_path)?;
            validate_private_directory(&legacy_directory)?;
            validate_private_regular_file(&legacy_auth_path)?;
            return Err(AuthStorageError::migration_conflict());
        }
        if canonical_exists {
            return ensure_private_regular_file(&self.auth_path);
        }
        if !legacy_exists {
            return self.write_document(&AuthDocument::default());
        }

        validate_private_directory(&legacy_directory)?;
        validate_private_regular_file(&legacy_auth_path)?;
        let legacy_lock_path = legacy_directory.join(LOCK_FILE);
        let legacy_lock = open_private_file(&legacy_lock_path, false)?;
        legacy_lock
            .lock_exclusive()
            .map_err(|_| AuthStorageError::unavailable())?;

        if private_regular_file_exists(&self.auth_path)? {
            return Err(AuthStorageError::migration_conflict());
        }
        validate_private_regular_file(&legacy_auth_path)?;
        let document = read_document_at(&legacy_auth_path)?;
        self.write_document(&document)?;
        let migrated = self.read_document()?;
        if migrated != document {
            return Err(AuthStorageError::unavailable());
        }
        ensure_private_regular_file(&self.auth_path)?;
        fs::remove_file(&legacy_auth_path).map_err(|_| AuthStorageError::unavailable())?;
        File::open(&legacy_directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| AuthStorageError::unavailable())?;
        FileExt::unlock(&legacy_lock).map_err(|_| AuthStorageError::unavailable())
    }

    pub(crate) fn status(&self, provider: &str) -> Result<AuthStatus, AuthStorageError> {
        validate_provider(provider)?;
        let document = self.read_locked()?;
        Ok(if document.0.contains_key(provider) {
            AuthStatus::Configured
        } else {
            AuthStatus::NotConfigured
        })
    }

    pub(crate) fn load(&self, provider: &str) -> Result<Option<OAuthCredential>, AuthStorageError> {
        validate_provider(provider)?;
        Ok(self.read_locked()?.0.get(provider).cloned())
    }

    pub(crate) fn save(
        &self,
        provider: &str,
        credential: &OAuthCredential,
    ) -> Result<(), AuthStorageError> {
        validate_provider(provider)?;
        let lock = self.open_lock()?;
        lock.lock_exclusive()
            .map_err(|_| AuthStorageError::unavailable())?;
        let mut document = self.read_document()?;
        document.0.insert(provider.to_owned(), credential.clone());
        self.write_document(&document)?;
        FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())
    }

    pub(crate) fn remove(&self, provider: &str) -> Result<(), AuthStorageError> {
        validate_provider(provider)?;
        let lock = self.open_lock()?;
        lock.lock_exclusive()
            .map_err(|_| AuthStorageError::unavailable())?;
        let mut document = self.read_document()?;
        document.0.remove(provider);
        self.write_document(&document)?;
        FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())
    }

    pub(crate) async fn resolve_with_refresh<F, Fut>(
        &self,
        provider: &str,
        refresh: F,
    ) -> Result<Option<OAuthCredential>, AuthStorageError>
    where
        F: FnOnce(OAuthCredential) -> Fut,
        Fut: Future<Output = Result<OAuthCredential, AuthStorageError>>,
    {
        self.resolve_with_refresh_using_clock(
            provider,
            || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| AuthStorageError::invalid_configuration())?
                    .as_millis()
                    .try_into()
                    .map_err(|_| AuthStorageError::invalid_configuration())
            },
            refresh,
        )
        .await
    }

    pub(super) async fn resolve_with_refresh_at<F, Fut>(
        &self,
        provider: &str,
        now_ms: u64,
        refresh: F,
    ) -> Result<Option<OAuthCredential>, AuthStorageError>
    where
        F: FnOnce(OAuthCredential) -> Fut,
        Fut: Future<Output = Result<OAuthCredential, AuthStorageError>>,
    {
        self.resolve_with_refresh_using_clock(provider, || Ok(now_ms), refresh)
            .await
    }

    pub(super) async fn resolve_with_refresh_using_clock<C, F, Fut>(
        &self,
        provider: &str,
        current_time_ms: C,
        refresh: F,
    ) -> Result<Option<OAuthCredential>, AuthStorageError>
    where
        C: FnOnce() -> Result<u64, AuthStorageError>,
        F: FnOnce(OAuthCredential) -> Fut,
        Fut: Future<Output = Result<OAuthCredential, AuthStorageError>>,
    {
        validate_provider(provider)?;
        let lock = self.lock_exclusive_async().await?;
        let mut document = self.read_document()?;
        let now_ms = current_time_ms()?;
        let Some(stored) = document.0.get(provider).cloned() else {
            FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
            return Ok(None);
        };
        if now_ms < stored.expires_at_ms {
            FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
            return Ok(Some(stored));
        }

        let refreshed = match refresh(stored).await {
            Ok(refreshed) => refreshed,
            Err(_) => {
                FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
                let latest = self.load(provider)?;
                return match latest {
                    Some(credential) if now_ms < credential.expires_at_ms => Ok(Some(credential)),
                    _ => Err(AuthStorageError::refresh_failed()),
                };
            }
        };
        document.0.insert(provider.to_owned(), refreshed.clone());
        self.write_document(&document)?;
        FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
        Ok(Some(refreshed))
    }

    async fn lock_exclusive_async(&self) -> Result<File, AuthStorageError> {
        let lock = self.open_lock()?;
        let started = Instant::now();
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => return Ok(lock),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if started.elapsed() >= LOCK_TIMEOUT {
                        return Err(AuthStorageError::unavailable());
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(_) => return Err(AuthStorageError::unavailable()),
            }
        }
    }

    fn read_locked(&self) -> Result<AuthDocument, AuthStorageError> {
        let lock = self.open_lock()?;
        lock.lock_shared()
            .map_err(|_| AuthStorageError::unavailable())?;
        let document = self.read_document()?;
        FileExt::unlock(&lock).map_err(|_| AuthStorageError::unavailable())?;
        Ok(document)
    }

    fn open_lock(&self) -> Result<File, AuthStorageError> {
        open_private_file(&self.lock_path, false)
    }

    fn read_document(&self) -> Result<AuthDocument, AuthStorageError> {
        read_document_at(&self.auth_path)
    }

    fn write_document(&self, document: &AuthDocument) -> Result<(), AuthStorageError> {
        let bytes = serde_json::to_vec_pretty(document)
            .map_err(|_| AuthStorageError::invalid_configuration())?;
        let temp_path = self.directory.join(TEMP_FILE);
        if temp_path.exists() {
            let metadata =
                fs::symlink_metadata(&temp_path).map_err(|_| AuthStorageError::unavailable())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(AuthStorageError::invalid_configuration());
            }
            fs::remove_file(&temp_path).map_err(|_| AuthStorageError::unavailable())?;
        }
        let mut temp = open_private_file(&temp_path, true)?;
        temp.write_all(&bytes)
            .and_then(|_| temp.write_all(b"\n"))
            .and_then(|_| temp.sync_all())
            .map_err(|_| AuthStorageError::unavailable())?;
        fs::rename(&temp_path, &self.auth_path).map_err(|_| AuthStorageError::unavailable())?;
        ensure_private_regular_file(&self.auth_path)?;
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| AuthStorageError::unavailable())
    }
}

fn validate_provider(provider: &str) -> Result<(), AuthStorageError> {
    let valid = !provider.is_empty()
        && provider.len() <= 128
        && provider
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(AuthStorageError::invalid_configuration())
    }
}

fn path_is_inside_repository(path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

fn canonical_existing_prefix_is_inside_repository(path: &Path) -> bool {
    path.ancestors()
        .find(|ancestor| ancestor.exists())
        .and_then(|ancestor| fs::canonicalize(ancestor).ok())
        .is_some_and(|canonical| path_is_inside_repository(&canonical))
}

fn path_below_ancestor_contains_symlink(
    trusted_ancestor: &Path,
    path: &Path,
) -> Result<bool, AuthStorageError> {
    let relative = path
        .strip_prefix(trusted_ancestor)
        .map_err(|_| AuthStorageError::invalid_configuration())?;
    let mut current = trusted_ancestor.to_owned();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AuthStorageError::unavailable()),
        }
    }
    Ok(false)
}

fn create_private_directory(path: &Path) -> Result<(), AuthStorageError> {
    fs::create_dir_all(path).map_err(|_| AuthStorageError::unavailable())?;
    let metadata = fs::symlink_metadata(path).map_err(|_| AuthStorageError::unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AuthStorageError::invalid_configuration());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| AuthStorageError::unavailable())?;
    let mode = fs::metadata(path)
        .map_err(|_| AuthStorageError::unavailable())?
        .permissions()
        .mode()
        & 0o777;
    if mode != 0o700 {
        return Err(AuthStorageError::invalid_configuration());
    }
    Ok(())
}

fn trusted_directory_is_real(path: &Path) -> Result<bool, AuthStorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(_) => Err(AuthStorageError::invalid_configuration()),
    }
}

fn private_directory_exists(path: &Path) -> Result<bool, AuthStorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(AuthStorageError::invalid_configuration())
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AuthStorageError::unavailable()),
    }
}

fn private_regular_file_exists(path: &Path) -> Result<bool, AuthStorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(AuthStorageError::invalid_configuration())
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AuthStorageError::unavailable()),
    }
}

fn read_document_at(path: &Path) -> Result<AuthDocument, AuthStorageError> {
    ensure_private_regular_file(path)?;
    let mut file = open_private_file(path, false)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| AuthStorageError::unavailable())?;
    serde_json::from_slice(&bytes).map_err(|_| AuthStorageError::invalid_configuration())
}

fn validate_private_directory(path: &Path) -> Result<(), AuthStorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| AuthStorageError::unavailable())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(AuthStorageError::invalid_configuration());
    }
    Ok(())
}

fn validate_private_regular_file(path: &Path) -> Result<(), AuthStorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| AuthStorageError::unavailable())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(AuthStorageError::invalid_configuration());
    }
    Ok(())
}

fn ensure_private_regular_file(path: &Path) -> Result<(), AuthStorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| AuthStorageError::unavailable())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AuthStorageError::invalid_configuration());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| AuthStorageError::unavailable())?;
    let mode = fs::metadata(path)
        .map_err(|_| AuthStorageError::unavailable())?
        .permissions()
        .mode()
        & 0o777;
    if mode != 0o600 {
        return Err(AuthStorageError::invalid_configuration());
    }
    Ok(())
}

fn open_private_file(path: &Path, create_new: bool) -> Result<File, AuthStorageError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true);
    }
    let file = options
        .open(path)
        .map_err(|_| AuthStorageError::unavailable())?;
    if !file
        .metadata()
        .map_err(|_| AuthStorageError::unavailable())?
        .is_file()
    {
        return Err(AuthStorageError::invalid_configuration());
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| AuthStorageError::unavailable())?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const PROVIDER: &str = "synthetic-provider";

    fn credential(suffix: &str, expires_at_ms: u64) -> OAuthCredential {
        OAuthCredential::new(
            format!("synthetic-access-{suffix}"),
            format!("synthetic-refresh-{suffix}"),
            expires_at_ms,
            format!("synthetic-account-{suffix}"),
        )
    }

    #[test]
    fn save_load_status_and_remove_use_private_repository_external_storage() {
        let app_data = tempfile::tempdir().unwrap();
        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let expected = credential("alpha", u64::MAX);

        assert_eq!(storage.status(PROVIDER).unwrap(), AuthStatus::NotConfigured);
        storage.save(PROVIDER, &expected).unwrap();
        assert_eq!(storage.status(PROVIDER).unwrap(), AuthStatus::Configured);
        let stored_json: serde_json::Value =
            serde_json::from_slice(&fs::read(app_data.path().join("agents/auth.json")).unwrap())
                .unwrap();
        assert_eq!(stored_json[PROVIDER]["type"], "oauth");
        assert_eq!(stored_json[PROVIDER]["expires"], u64::MAX);
        assert!(stored_json[PROVIDER].get("expiresAtMs").is_none());
        assert!(storage.load(PROVIDER).unwrap().as_ref() == Some(&expected));

        let storage_dir = app_data.path().join("agents");
        let auth_file = storage_dir.join("auth.json");
        assert!(auth_file.starts_with(app_data.path()));
        assert_eq!(
            storage_dir.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            auth_file.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            storage.lock_path.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );

        storage.remove(PROVIDER).unwrap();
        assert_eq!(storage.status(PROVIDER).unwrap(), AuthStatus::NotConfigured);
        assert!(storage.load(PROVIDER).unwrap().is_none());
    }

    #[test]
    fn migrates_legacy_auth_to_the_agents_root_before_removing_the_original() {
        let app_data = tempfile::tempdir().unwrap();
        let legacy_dir = app_data.path().join("agent");
        fs::create_dir(&legacy_dir).unwrap();
        fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let legacy_auth = legacy_dir.join("auth.json");
        let mut document = AuthDocument::default();
        document
            .0
            .insert(PROVIDER.to_owned(), credential("legacy", u64::MAX));
        let bytes = serde_json::to_vec_pretty(&document).unwrap();
        let mut legacy_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&legacy_auth)
            .unwrap();
        legacy_file.write_all(&bytes).unwrap();
        legacy_file.sync_all().unwrap();

        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();

        assert_eq!(storage.status(PROVIDER).unwrap(), AuthStatus::Configured);
        assert!(!legacy_auth.exists());
        let canonical_auth = app_data.path().join("agents/auth.json");
        assert!(canonical_auth.is_file());
        assert_eq!(
            canonical_auth.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            canonical_auth
                .parent()
                .unwrap()
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn existing_legacy_and_canonical_auth_fail_with_a_redacted_conflict() {
        let app_data = tempfile::tempdir().unwrap();
        let canonical = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        canonical
            .save(PROVIDER, &credential("canonical", u64::MAX))
            .unwrap();
        let legacy_dir = app_data.path().join("agent");
        fs::create_dir(&legacy_dir).unwrap();
        fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let legacy_auth = legacy_dir.join("auth.json");
        let mut legacy = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&legacy_auth)
            .unwrap();
        legacy.write_all(b"{}\n").unwrap();
        legacy.sync_all().unwrap();

        let error = match AuthStorage::for_test_app_data(app_data.path()) {
            Ok(_) => panic!("conflicting credential files unexpectedly merged"),
            Err(error) => error,
        };

        assert_eq!(error.category, AuthStorageErrorCategory::MigrationConflict);
        assert_eq!(
            error.message,
            "conflicting authentication storage locations require review"
        );
        assert!(legacy_auth.exists());
        assert_eq!(canonical.status(PROVIDER).unwrap(), AuthStatus::Configured);
        let diagnostic = format!("{error:?}");
        assert!(!diagnostic.contains("canonical"));
        assert!(!diagnostic.contains(app_data.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn insecure_legacy_permissions_fail_closed_without_modifying_the_original() {
        let app_data = tempfile::tempdir().unwrap();
        let legacy_dir = app_data.path().join("agent");
        fs::create_dir(&legacy_dir).unwrap();
        fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o755)).unwrap();
        let legacy_auth = legacy_dir.join("auth.json");
        let mut legacy = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&legacy_auth)
            .unwrap();
        legacy.write_all(b"{}\n").unwrap();
        legacy.sync_all().unwrap();

        let error = match AuthStorage::for_test_app_data(app_data.path()) {
            Ok(_) => panic!("insecure legacy credentials unexpectedly migrated"),
            Err(error) => error,
        };

        assert_eq!(
            error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
        assert!(legacy_auth.exists());
        assert!(!app_data.path().join("agents/auth.json").exists());
        assert_eq!(
            legacy_dir.metadata().unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            legacy_auth.metadata().unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn malformed_legacy_auth_fails_closed_and_keeps_the_original() {
        let app_data = tempfile::tempdir().unwrap();
        let legacy_dir = app_data.path().join("agent");
        fs::create_dir(&legacy_dir).unwrap();
        fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let legacy_auth = legacy_dir.join("auth.json");
        let mut legacy = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&legacy_auth)
            .unwrap();
        legacy.write_all(b"synthetic malformed document").unwrap();
        legacy.sync_all().unwrap();

        let error = match AuthStorage::for_test_app_data(app_data.path()) {
            Ok(_) => panic!("malformed legacy credentials unexpectedly migrated"),
            Err(error) => error,
        };

        assert_eq!(
            error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
        assert!(legacy_auth.exists());
        assert!(!app_data.path().join("agents/auth.json").exists());
        assert!(!format!("{error:?}").contains("synthetic"));
    }

    #[test]
    fn concurrent_provider_mutations_preserve_both_latest_entries() {
        use std::sync::{Arc, Barrier};

        let app_data = tempfile::tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            for (provider, suffix) in [
                ("synthetic-provider-a", "alpha"),
                ("synthetic-provider-b", "beta"),
            ] {
                let app_data_path = app_data.path().to_owned();
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    let storage = AuthStorage::for_test_app_data(&app_data_path).unwrap();
                    barrier.wait();
                    storage
                        .save(provider, &credential(suffix, u64::MAX))
                        .unwrap();
                });
            }
            barrier.wait();
        });

        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        assert!(storage.load("synthetic-provider-a").unwrap().is_some());
        assert!(storage.load("synthetic-provider-b").unwrap().is_some());
    }

    #[test]
    fn concurrent_expired_credential_resolution_refreshes_once_and_persists_before_use() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};

        let app_data = tempfile::tempdir().unwrap();
        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        storage.save(PROVIDER, &credential("expired", 100)).unwrap();
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(3));

        std::thread::scope(|scope| {
            for _ in 0..2 {
                let app_data_path = app_data.path().to_owned();
                let refresh_count = Arc::clone(&refresh_count);
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    let storage = AuthStorage::for_test_app_data(&app_data_path).unwrap();
                    barrier.wait();
                    let resolved = tauri::async_runtime::block_on(storage.resolve_with_refresh_at(
                        PROVIDER,
                        100,
                        move |_| async move {
                            refresh_count.fetch_add(1, Ordering::SeqCst);
                            Ok(credential("rotated", 1_000))
                        },
                    ))
                    .unwrap()
                    .unwrap();
                    assert!(resolved == credential("rotated", 1_000));
                });
            }
            barrier.wait();
        });

        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
        assert!(storage.load(PROVIDER).unwrap().as_ref() == Some(&credential("rotated", 1_000)));
    }

    #[test]
    fn expiry_clock_is_evaluated_after_waiting_for_the_storage_lock() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        use std::sync::{mpsc, Arc};

        let app_data = tempfile::tempdir().unwrap();
        let holder = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let waiter = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        holder.save(PROVIDER, &credential("expiring", 100)).unwrap();
        let held_lock = holder.open_lock().unwrap();
        held_lock.lock_exclusive().unwrap();

        let clock = Arc::new(AtomicU64::new(99));
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let (clock_called, clock_observed) = mpsc::channel();
        std::thread::scope(|scope| {
            let current_time = Arc::clone(&clock);
            let waiter_refresh_count = Arc::clone(&refresh_count);
            let handle = scope.spawn(move || {
                tauri::async_runtime::block_on(waiter.resolve_with_refresh_using_clock(
                    PROVIDER,
                    move || {
                        clock_called.send(()).unwrap();
                        Ok(current_time.load(Ordering::SeqCst))
                    },
                    move |_| async move {
                        waiter_refresh_count.fetch_add(1, Ordering::SeqCst);
                        Ok(credential("rotated-after-wait", 1_000))
                    },
                ))
                .unwrap()
                .unwrap()
            });

            assert!(clock_observed
                .recv_timeout(Duration::from_millis(25))
                .is_err());
            clock.store(100, Ordering::SeqCst);
            FileExt::unlock(&held_lock).unwrap();
            assert!(handle.join().unwrap() == credential("rotated-after-wait", 1_000));
        });

        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn refresh_lock_wait_does_not_block_other_async_work() {
        let app_data = tempfile::tempdir().unwrap();
        let first = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let second = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        first.save(PROVIDER, &credential("expired", 100)).unwrap();

        tauri::async_runtime::block_on(async {
            let first_resolution = first.resolve_with_refresh_at(PROVIDER, 100, |_| async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                Ok(credential("rotated", 1_000))
            });
            let second_resolution = second.resolve_with_refresh_at(PROVIDER, 100, |_| async {
                panic!("second waiter must observe the persisted rotated credential")
            });
            let (first_result, second_result) = tokio::time::timeout(
                Duration::from_secs(1),
                futures_util::future::join(first_resolution, second_resolution),
            )
            .await
            .expect("async refresh coordination timed out");

            assert!(first_result.unwrap().unwrap() == credential("rotated", 1_000));
            assert!(second_result.unwrap().unwrap() == credential("rotated", 1_000));
        });
    }

    #[test]
    fn refresh_failure_keeps_expired_credential_and_returns_only_redacted_diagnostics() {
        let app_data = tempfile::tempdir().unwrap();
        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let expired = credential("expired", 100);
        storage.save(PROVIDER, &expired).unwrap();

        let error = match tauri::async_runtime::block_on(storage.resolve_with_refresh_at(
            PROVIDER,
            100,
            |_| async { Err(AuthStorageError::unavailable()) },
        )) {
            Ok(_) => panic!("expired credential unexpectedly resolved"),
            Err(error) => error,
        };

        assert_eq!(error.category, AuthStorageErrorCategory::RefreshFailed);
        assert_eq!(error.message, "authentication refresh failed");
        assert!(!format!("{error:?}").contains("synthetic-"));
        assert!(storage.load(PROVIDER).unwrap().as_ref() == Some(&expired));
    }

    #[test]
    fn insecure_storage_locations_are_rejected_without_creating_files() {
        let relative = Path::new("relative-app-data");
        let relative_error = match AuthStorage::in_app_data_dir(relative) {
            Ok(_) => panic!("relative storage path unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(
            relative_error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
        assert!(!relative.exists());

        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let repository_error = match AuthStorage::in_app_data_dir(repository_root) {
            Ok(_) => panic!("repository storage path unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(
            repository_error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );

        let app_data_target = tempfile::tempdir().unwrap();
        let symlink_root = tempfile::tempdir().unwrap();
        let linked_app_data = symlink_root.path().join("linked-app-data");
        std::os::unix::fs::symlink(app_data_target.path(), &linked_app_data).unwrap();
        let symlink_error = match AuthStorage::in_app_data_dir(&linked_app_data) {
            Ok(_) => panic!("symlinked app-data path unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(
            symlink_error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );

        let linked_parent = symlink_root.path().join("linked-parent");
        std::os::unix::fs::symlink(app_data_target.path(), &linked_parent).unwrap();
        let nested_app_data = linked_parent.join("nested-app-data");
        let intermediate_error =
            match AuthStorage::in_app_data_dir_from(symlink_root.path(), &nested_app_data) {
                Ok(_) => panic!("intermediate symlink unexpectedly accepted"),
                Err(error) => error,
            };
        assert_eq!(
            intermediate_error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
        assert_eq!(
            repository_error.message,
            "authentication storage is not securely configured"
        );
    }

    #[test]
    fn non_regular_lock_file_is_rejected() {
        let app_data = tempfile::tempdir().unwrap();
        let storage_dir = app_data.path().join("agents");
        fs::create_dir(&storage_dir).unwrap();
        let lock_path = storage_dir.join("auth.lock");
        let lock_path = std::ffi::CString::new(lock_path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(lock_path.as_ptr(), 0o600) }, 0);

        let error = match AuthStorage::for_test_app_data(app_data.path()) {
            Ok(_) => panic!("non-regular lock file unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(
            error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
    }

    #[test]
    fn malformed_storage_and_symlinks_fail_closed_without_leaking_paths_or_contents() {
        let app_data = tempfile::tempdir().unwrap();
        let storage = AuthStorage::for_test_app_data(app_data.path()).unwrap();
        let auth_path = app_data.path().join("agents/auth.json");
        fs::write(&auth_path, b"synthetic malformed credential document").unwrap();
        let error = match storage.load(PROVIDER) {
            Ok(_) => panic!("malformed credential document unexpectedly loaded"),
            Err(error) => error,
        };
        let diagnostic = format!("{error:?}");
        assert_eq!(
            error.category,
            AuthStorageErrorCategory::InvalidConfiguration
        );
        assert!(!diagnostic.contains("synthetic"));
        assert!(!diagnostic.contains(app_data.path().to_string_lossy().as_ref()));

        fs::remove_file(&auth_path).unwrap();
        std::os::unix::fs::symlink(app_data.path().join("elsewhere"), &auth_path).unwrap();
        let symlink_error = storage.status(PROVIDER).unwrap_err();
        assert!(matches!(
            symlink_error.category,
            AuthStorageErrorCategory::InvalidConfiguration | AuthStorageErrorCategory::Unavailable
        ));
        assert!(!format!("{symlink_error:?}").contains(app_data.path().to_string_lossy().as_ref()));
    }
}
