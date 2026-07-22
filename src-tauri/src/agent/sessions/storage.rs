use super::reconstruct::reconstruct;
use super::wire;
use super::{
    CompactionRecord, CompletedTurn, ContinuationBlock, RecoveryNotice, Runtime, SessionAccess,
    SessionCheckpoint, SessionError, SessionErrorCode, SessionHandle, SessionId, SessionSnapshot,
    SessionSummary, SystemRuntime,
};
use crate::agent::models::{ModelId, ProviderId, ReasoningLevel};
use fs2::FileExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone)]
pub struct SessionManager {
    root: Arc<PathBuf>,
    runtime: Arc<dyn Runtime>,
}
pub(crate) enum HandleState {
    Draft {
        timestamp: String,
        queued: Vec<Value>,
        leaf: Option<String>,
    },
    Existing {
        path: PathBuf,
        owner: Option<File>,
        expected_len: u64,
        expected_hash: [u8; 32],
        leaf: Option<String>,
    },
}

impl SessionManager {
    pub fn from_agents_data_root(root: &Path) -> Result<Self, SessionError> {
        Self::with_runtime(root, Arc::new(SystemRuntime))
    }
    pub(crate) fn with_runtime(
        root: &Path,
        runtime: Arc<dyn Runtime>,
    ) -> Result<Self, SessionError> {
        validate_root(root)?;
        Ok(Self {
            root: Arc::new(root.join("sessions")),
            runtime,
        })
    }
    pub fn create(&self) -> Result<SessionHandle, SessionError> {
        let timestamp = self.runtime.now();
        validate_timestamp(&timestamp)?;
        let id = generate_session_id(self.runtime.as_ref())?;
        let snapshot =
            SessionSnapshot::empty(id.clone(), timestamp.clone(), SessionAccess::Writable);
        Ok(SessionHandle {
            manager: self.clone(),
            snapshot,
            continuation: Vec::new(),
            state: HandleState::Draft {
                timestamp,
                queued: Vec::new(),
                leaf: None,
            },
            poisoned: false,
        })
    }
    pub fn open(&self, id: &SessionId) -> Result<SessionHandle, SessionError> {
        let path = find_path(&self.root, id)?
            .ok_or_else(|| SessionError::new(SessionErrorCode::NotFound))?;
        open_path(self.clone(), id, path)
    }
    pub fn list(&self) -> Result<Vec<SessionSummary>, SessionError> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&*self.root)
            .map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?
        {
            let entry = entry.map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.ends_with(".jsonl") {
                continue;
            }
            let Some(id_text) = name
                .strip_suffix(".jsonl")
                .and_then(|s| s.rsplit_once('_').map(|x| x.1))
            else {
                continue;
            };
            let Ok(id) = SessionId::from_str(id_text) else {
                continue;
            };
            match self.open(&id) {
                Ok(handle) => out.push(SessionSummary {
                    snapshot: handle.snapshot.clone(),
                }),
                Err(error)
                    if matches!(
                        error.code(),
                        SessionErrorCode::Damaged | SessionErrorCode::IncompleteFinalSuffix
                    ) =>
                {
                    let created = "1970-01-01T00:00:00Z".to_owned();
                    let mut s = SessionSnapshot::empty(id, created, SessionAccess::Damaged);
                    s.display_name = "Damaged session".into();
                    out.push(SessionSummary { snapshot: s });
                }
                Err(_) => {}
            }
        }
        out.sort_by(|a, b| {
            b.modified_at()
                .cmp(a.modified_at())
                .then_with(|| a.id().cmp(&b.id()))
        });
        Ok(out)
    }
    pub fn move_to_trash(&self, id: &SessionId) -> Result<(), SessionError> {
        let path = find_path(&self.root, id)?
            .ok_or_else(|| SessionError::new(SessionErrorCode::NotFound))?;
        let owner_path = lock_path(&self.root, id, "owner");
        let data_path = lock_path(&self.root, id, "data");
        self.runtime
            .checkpoint(SessionCheckpoint::Lock)
            .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
        let owner = open_lock(&owner_path)?;
        owner
            .try_lock_exclusive()
            .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
        let data = open_lock(&data_path)?;
        data.lock_exclusive()
            .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
        let mut session_file = open_private_session(&path, false)?;
        let mut bytes = Vec::new();
        session_file
            .read_to_end(&mut bytes)
            .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?;
        let document = wire::parse(&bytes, Some(id.uuid()))?;
        if path.file_name().and_then(|name| name.to_str())
            != Some(timestamp_filename(&document.header.timestamp, id).as_str())
        {
            return Err(SessionError::new(SessionErrorCode::Damaged));
        }
        let staging = self
            .root
            .join(format!(".trash-{}-{}.staged", id, self.runtime.uuid()));
        atomic_rename_noreplace(&path, &staging)
            .map_err(|_| SessionError::new(SessionErrorCode::TrashFailed))?;
        if !path_matches_file(&staging, &session_file).unwrap_or(false) {
            let _ = atomic_rename_noreplace(&staging, &path);
            return Err(SessionError::new(SessionErrorCode::TrashFailed));
        }
        if sync_dir(&self.root).is_err()
            || self.runtime.checkpoint(SessionCheckpoint::Trash).is_err()
            || self.runtime.trash(&staging).is_err()
        {
            // Never delete on failure. Restore only by a no-replace atomic move so an
            // attacker-created destination cannot be overwritten.
            let restored = atomic_rename_noreplace(&staging, &path).is_ok()
                && path_matches_file(&path, &session_file).unwrap_or(false)
                && sync_dir(&self.root).is_ok();
            if !restored {
                return Err(SessionError::new(SessionErrorCode::TrashFailed));
            }
            return Err(SessionError::new(SessionErrorCode::TrashFailed));
        }
        sync_dir(&self.root).map_err(|_| SessionError::new(SessionErrorCode::TrashFailed))?;
        let _ = FileExt::unlock(&data);
        let _ = FileExt::unlock(&owner);
        drop(data);
        drop(owner);
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(owner_path);
        Ok(())
    }
}
fn validate_root(root: &Path) -> Result<(), SessionError> {
    if !root.is_absolute()
        || root.file_name().and_then(|x| x.to_str()) != Some("agents")
        || crate::agent::auth::path_is_inside_repository(root)
        || crate::agent::auth::canonical_existing_prefix_is_inside_repository(root)
    {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    let existing = root
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .ok_or_else(|| SessionError::new(SessionErrorCode::InvalidRoot))?;
    let canonical =
        fs::canonicalize(existing).map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    if canonical != existing {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    for ancestor in root
        .ancestors()
        .take_while(|ancestor| *ancestor != Path::new("/"))
    {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            if unsafe_path_metadata(&metadata) {
                return Err(SessionError::new(SessionErrorCode::InvalidRoot));
            }
        }
    }
    if let Ok(m) = fs::symlink_metadata(root) {
        if unsafe_path_metadata(&m) || !m.is_dir() {
            return Err(SessionError::new(SessionErrorCode::InvalidRoot));
        }
    }
    private_dir(root)?;
    private_dir(&root.join("sessions"))
}
pub(crate) fn unsafe_path_metadata(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
pub(crate) fn harden_windows_path(path: &Path, directory: bool) -> Result<(), SessionError> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, GENERIC_ALL, HANDLE};
    use windows_sys::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, SET_ACCESS, SE_FILE_OBJECT,
        TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
        PROTECTED_DACL_SECURITY_INFORMATION, SUB_CONTAINERS_AND_OBJECTS_INHERIT, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = null_mut();
    // SAFETY: output handle is valid and closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    let result = (|| {
        let mut needed = 0_u32;
        // First call intentionally obtains the required buffer size.
        unsafe {
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        }
        if needed == 0 {
            return Err(SessionError::new(SessionErrorCode::InvalidRoot));
        }
        let words = (needed as usize + size_of::<usize>() - 1) / size_of::<usize>();
        let mut buffer = vec![0_usize; words];
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast::<c_void>(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err(SessionError::new(SessionErrorCode::InvalidRoot));
        }
        let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
        let mut explicit = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL,
            grfAccessMode: SET_ACCESS,
            grfInheritance: if directory {
                SUB_CONTAINERS_AND_OBJECTS_INHERIT
            } else {
                NO_INHERITANCE
            },
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: user.User.Sid.cast(),
            },
        };
        let mut acl = null_mut();
        if unsafe { SetEntriesInAclW(1, &mut explicit, null(), &mut acl) } != 0 {
            return Err(SessionError::new(SessionErrorCode::InvalidRoot));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let status = unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null(),
            )
        };
        unsafe {
            LocalFree(acl.cast());
        }
        if status == 0 {
            Ok(())
        } else {
            Err(SessionError::new(SessionErrorCode::InvalidRoot))
        }
    })();
    unsafe {
        CloseHandle(token);
    }
    result
}

fn private_dir(path: &Path) -> Result<(), SessionError> {
    fs::create_dir_all(path).map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    let m =
        fs::symlink_metadata(path).map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    if unsafe_path_metadata(&m) || !m.is_dir() {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    #[cfg(windows)]
    harden_windows_path(path, true)?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    }
    Ok(())
}
fn validate_timestamp(s: &str) -> Result<(), SessionError> {
    let t = OffsetDateTime::parse(s, &Rfc3339)
        .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
    if t.offset() != time::UtcOffset::UTC {
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    Ok(())
}
fn generate_session_id(runtime: &dyn Runtime) -> Result<SessionId, SessionError> {
    for _ in 0..100 {
        let id = runtime.uuid();
        if id.get_version_num() == 7 {
            return SessionId::from_str(&id.to_string());
        }
    }
    Err(SessionError::new(SessionErrorCode::NotSaved))
}
fn entry_id(runtime: &dyn Runtime, used: &[String]) -> String {
    for _ in 0..100 {
        let raw = runtime.uuid().simple().to_string();
        let short = raw[..8].to_owned();
        if !used.contains(&short) {
            return short;
        }
    }
    runtime.uuid().to_string()
}
fn timestamp_filename(ts: &str, id: &SessionId) -> String {
    format!("{}_{}.jsonl", ts.replace([':', '.'], "-"), id)
}
fn find_path(root: &Path, id: &SessionId) -> Result<Option<PathBuf>, SessionError> {
    let suffix = format!("_{}.jsonl", id);
    let mut found = None;
    for e in fs::read_dir(root).map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))? {
        let e = e.map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
        let n = e.file_name();
        if n.to_str().is_some_and(|s| s.ends_with(&suffix)) {
            if found.is_some() {
                return Err(SessionError::new(SessionErrorCode::Damaged));
            }
            found = Some(e.path())
        }
    }
    Ok(found)
}
fn lock_path(root: &Path, id: &SessionId, role: &str) -> PathBuf {
    root.join(format!(".{}.{}.lock", id, role))
}
fn open_lock(path: &Path) -> Result<File, SessionError> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        o.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let f = o
        .open(path)
        .map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    if unsafe_path_metadata(&path_metadata) || !path_metadata.is_file() {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    #[cfg(windows)]
    harden_windows_path(path, false)?;
    let metadata = f
        .metadata()
        .map_err(|_| SessionError::new(SessionErrorCode::InvalidRoot))?;
    if !metadata.is_file() {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(SessionError::new(SessionErrorCode::InvalidRoot));
    }
    Ok(f)
}
fn open_path(
    manager: SessionManager,
    id: &SessionId,
    path: PathBuf,
) -> Result<SessionHandle, SessionError> {
    manager
        .runtime
        .checkpoint(SessionCheckpoint::Lock)
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let owner = open_lock(&lock_path(&manager.root, id, "owner"))?;
    let writable = owner.try_lock_exclusive().is_ok();
    let owner = if writable { Some(owner) } else { None };
    let loaded = load(&manager, id, &path, writable);
    let (snapshot, continuation, leaf, len) = match loaded {
        Ok((snapshot, continuation, leaf, len, _)) => (snapshot, continuation, leaf, len),
        Err(error)
            if matches!(
                error.code(),
                SessionErrorCode::Damaged
                    | SessionErrorCode::IncompleteFinalSuffix
                    | SessionErrorCode::SizeLimit
            ) =>
        {
            let created = "1970-01-01T00:00:00Z".to_owned();
            (
                SessionSnapshot::empty(id.clone(), created, SessionAccess::Damaged),
                Vec::new(),
                None,
                fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            )
        }
        Err(error) => return Err(error),
    };
    let owner = if snapshot.access() == SessionAccess::Writable {
        owner
    } else {
        None
    };
    let expected_hash = hash_path(&path).unwrap_or([0; 32]);
    Ok(SessionHandle {
        manager,
        snapshot,
        continuation,
        state: HandleState::Existing {
            path,
            owner,
            expected_len: len,
            expected_hash,
            leaf,
        },
        poisoned: false,
    })
}
fn open_private_session(path: &Path, write: bool) -> Result<File, SessionError> {
    let mut o = OpenOptions::new();
    o.read(true).write(write).append(write);
    #[cfg(unix)]
    {
        o.custom_flags(libc::O_NOFOLLOW);
    }
    let f = o
        .open(path)
        .map_err(|_| SessionError::new(SessionErrorCode::NotFound))?;
    let m = f
        .metadata()
        .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?;
    if !m.is_file()
        || unsafe_path_metadata(
            &fs::symlink_metadata(path)
                .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?,
        )
    {
        return Err(SessionError::new(SessionErrorCode::Damaged));
    }
    #[cfg(unix)]
    if m.permissions().mode() & 0o777 != 0o600 {
        return Err(SessionError::new(SessionErrorCode::Damaged));
    }
    Ok(f)
}
fn read_private(path: &Path) -> Result<Vec<u8>, SessionError> {
    let mut f = open_private_session(path, false)?;
    let mut b = Vec::new();
    f.read_to_end(&mut b)
        .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?;
    Ok(b)
}
fn load(
    manager: &SessionManager,
    id: &SessionId,
    path: &Path,
    writable: bool,
) -> Result<
    (
        SessionSnapshot,
        Vec<ContinuationBlock>,
        Option<String>,
        u64,
        Option<RecoveryNotice>,
    ),
    SessionError,
> {
    manager
        .runtime
        .checkpoint(SessionCheckpoint::Lock)
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let data = open_lock(&lock_path(&manager.root, id, "data"))?;
    if writable {
        data.lock_exclusive()
    } else {
        data.lock_shared()
    }
    .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let mut session_file = open_private_session(path, writable)?;
    let mut bytes = Vec::new();
    session_file
        .read_to_end(&mut bytes)
        .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?;
    let mut notice = None;
    let doc = match wire::parse(&bytes, Some(id.uuid())) {
        Ok(d) => d,
        Err(original) if writable => {
            let Some(cut) = recovery_cut(&bytes, id.uuid(), original.code()) else {
                let _ = FileExt::unlock(&data);
                return Err(original);
            };
            if !path_matches_file(path, &session_file)? {
                return Err(SessionError::new(SessionErrorCode::ExternalChange));
            }
            manager
                .runtime
                .checkpoint(SessionCheckpoint::Truncate)
                .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
            session_file
                .set_len(cut as u64)
                .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
            manager
                .runtime
                .checkpoint(SessionCheckpoint::TruncateSync)
                .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
            session_file
                .sync_all()
                .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
            if !path_matches_file(path, &session_file)? {
                return Err(SessionError::new(SessionErrorCode::ExternalChange));
            }
            bytes.truncate(cut);
            notice = Some(RecoveryNotice::IncompleteFinalTurnDiscarded);
            wire::parse(&bytes, Some(id.uuid()))?
        }
        Err(e) => {
            let _ = FileExt::unlock(&data);
            return Err(e);
        }
    };
    let expected_name = timestamp_filename(&doc.header.timestamp, id);
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        let _ = FileExt::unlock(&data);
        return Err(SessionError::new(SessionErrorCode::Damaged));
    }
    let (snapshot, ctx, leaf) = reconstruct(&doc, !writable, notice)?;
    let _ = FileExt::unlock(&data);
    Ok((snapshot, ctx, leaf, bytes.len() as u64, notice))
}
/// Return the only safe truncation boundary for a provable interrupted final
/// append. Inspection is limited to the final two entries and 32 MiB.
fn recovery_cut(bytes: &[u8], id: uuid::Uuid, original_code: SessionErrorCode) -> Option<usize> {
    let (last_start, last_end, terminated) = final_frame(bytes)?;
    if bytes.len().checked_sub(last_start)? > wire::MAX_BATCH
        || last_end.checked_sub(last_start)? > wire::MAX_LINE
    {
        return None;
    }
    let last = &bytes[last_start..last_end];
    let parsed_last = serde_json::from_slice::<Value>(last).ok();

    // A complete final User frame is itself a provable interrupted turn batch.
    if original_code == SessionErrorCode::IncompleteFinalSuffix
        && terminated
        && parsed_last.as_ref().is_some_and(is_user_value)
    {
        return valid_recovery_prefix(bytes, last_start, id).then_some(last_start);
    }

    // A malformed/non-terminated metadata frame is recoverable only when its
    // top-level type can be established by a byte-structural scan.
    let incomplete = !terminated || parsed_last.is_none();
    if incomplete
        && top_level_type(last).is_some_and(|kind| {
            matches!(
                kind,
                b"model_change" | b"thinking_level_change" | b"session_info" | b"compaction"
            )
        })
    {
        return valid_recovery_prefix(bytes, last_start, id).then_some(last_start);
    }

    // A User plus malformed Assistant is one interrupted complete-turn batch.
    if incomplete {
        let (previous_start, previous_end) = previous_frame(bytes, last_start)?;
        if bytes.len().checked_sub(previous_start)? > wire::MAX_BATCH
            || previous_end.checked_sub(previous_start)? > wire::MAX_LINE
        {
            return None;
        }
        let previous =
            serde_json::from_slice::<Value>(&bytes[previous_start..previous_end]).ok()?;
        let user_is_valid_incomplete = wire::parse(&bytes[..last_start], Some(id))
            .is_err_and(|error| error.code() == SessionErrorCode::IncompleteFinalSuffix);
        if is_user_value(&previous)
            && user_is_valid_incomplete
            && valid_recovery_prefix(bytes, previous_start, id)
        {
            return Some(previous_start);
        }
    }
    None
}

fn valid_recovery_prefix(bytes: &[u8], cut: usize, id: uuid::Uuid) -> bool {
    wire::parse(&bytes[..cut], Some(id))
        .and_then(|document| reconstruct(&document, false, None).map(|_| document))
        .is_ok()
}

fn is_user_value(value: &Value) -> bool {
    value
        .get("message")
        .and_then(|message| message.get("role"))
        .and_then(Value::as_str)
        == Some("user")
        && value.get("type").and_then(Value::as_str) == Some("message")
}

fn final_frame(bytes: &[u8]) -> Option<(usize, usize, bool)> {
    if bytes.is_empty() {
        return None;
    }
    let terminated = bytes.last() == Some(&b'\n');
    let end = bytes.len() - usize::from(terminated);
    if end == 0 {
        return None;
    }
    let start = bytes[..end]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    Some((start, end, terminated))
}

fn previous_frame(bytes: &[u8], before: usize) -> Option<(usize, usize)> {
    let end = before.checked_sub(1)?;
    let start = bytes[..end]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    Some((start, end))
}

// Extract a top-level `type` string without decoding unrelated UTF-8. The
// scanner understands JSON strings/nesting and therefore cannot be fooled by
// message text containing a formatting-dependent `\"type\"` substring.
fn top_level_type(frame: &[u8]) -> Option<&[u8]> {
    let mut index = skip_ws(frame, 0);
    if frame.get(index)? != &b'{' {
        return None;
    }
    index += 1;
    loop {
        index = skip_ws(frame, index);
        if frame.get(index)? == &b'}' {
            return None;
        }
        let (key, next) = json_string(frame, index)?;
        index = skip_ws(frame, next);
        if frame.get(index)? != &b':' {
            return None;
        }
        index = skip_ws(frame, index + 1);
        if key == b"type" {
            let (value, _) = json_string(frame, index)?;
            return Some(value);
        }
        index = skip_json_value(frame, index)?;
        index = skip_ws(frame, index);
        match frame.get(index)? {
            b',' => index += 1,
            b'}' => return None,
            _ => return None,
        }
    }
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        index += 1;
    }
    index
}

fn json_string(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    if bytes.get(start)? != &b'"' {
        return None;
    }
    let mut index = start + 1;
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'"' => return Some((&bytes[start + 1..index], index + 1)),
            b'\\' => index = index.checked_add(2)?,
            byte if *byte < 0x20 => return None,
            _ => index += 1,
        }
    }
    None
}

fn skip_json_value(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start)? == &b'"' {
        return json_string(bytes, start).map(|(_, end)| end);
    }
    let mut index = start;
    let mut depth = 0_u32;
    let mut quoted = false;
    while let Some(byte) = bytes.get(index) {
        if quoted {
            match byte {
                b'\\' => index = index.checked_add(2)?,
                b'"' => {
                    quoted = false;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }
        match byte {
            b'"' => {
                quoted = true;
                index += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' if depth > 0 => {
                depth -= 1;
                index += 1;
            }
            b',' | b'}' if depth == 0 => return Some(index),
            _ => index += 1,
        }
    }
    None
}

pub(crate) fn reload(h: &mut SessionHandle) -> Result<(), SessionError> {
    match &h.state {
        HandleState::Draft { .. } => Ok(()),
        HandleState::Existing { path, owner, .. } => {
            let writable = owner.is_some();
            let (s, c, l, len, _) = load(&h.manager, &h.snapshot.id, path, writable)?;
            h.snapshot = s;
            h.continuation = c;
            h.poisoned = false;
            let expected_hash = hash_path(path)?;
            h.state = HandleState::Existing {
                path: path.clone(),
                owner: owner.as_ref().and_then(|f| f.try_clone().ok()),
                expected_len: len,
                expected_hash,
                leaf: l,
            };
            Ok(())
        }
    }
}
fn ensure_mutable(h: &SessionHandle) -> Result<(), SessionError> {
    if h.poisoned {
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    match &h.state {
        HandleState::Draft { .. } => Ok(()),
        HandleState::Existing { owner: Some(_), .. }
            if h.snapshot.access() == SessionAccess::Writable =>
        {
            Ok(())
        }
        _ => Err(SessionError::new(SessionErrorCode::Locked)),
    }
}
fn draft_append(h: &mut SessionHandle, value: Value, id: String) -> Result<(), SessionError> {
    let HandleState::Draft { queued, leaf, .. } = &mut h.state else {
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    };
    queued.push(value);
    *leaf = Some(id);
    Ok(())
}
fn ms(ts: &str) -> Result<i128, SessionError> {
    Ok(OffsetDateTime::parse(ts, &Rfc3339)
        .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?
        .unix_timestamp_nanos()
        / 1_000_000)
}

pub(crate) fn append_turn(h: &mut SessionHandle, turn: CompletedTurn) -> Result<(), SessionError> {
    ensure_mutable(h)?;
    if turn.user_text.len() > wire::MAX_LINE || turn.assistant_blocks.is_empty() {
        return Err(SessionError::new(SessionErrorCode::SizeLimit));
    }
    match &h.state {
        HandleState::Draft {
            timestamp,
            queued,
            leaf,
        } => {
            let ts = timestamp.clone();
            let mut used: Vec<String> = queued
                .iter()
                .filter_map(|v| v.get("id")?.as_str().map(str::to_owned))
                .collect();
            let uid = entry_id(h.manager.runtime.as_ref(), &used);
            used.push(uid.clone());
            let aid = entry_id(h.manager.runtime.as_ref(), &used);
            let mut bytes = wire::line(wire::header(h.snapshot.id.uuid(), &ts))?;
            for v in queued {
                bytes.extend(wire::line(v.clone())?)
            }
            let parent = leaf.as_deref();
            bytes.extend(wire::line(wire::user(
                &uid,
                parent,
                &ts,
                &turn.user_text,
                ms(&ts)?,
            ))?);
            bytes.extend(wire::line(wire::assistant(
                &aid,
                &uid,
                &ts,
                &turn,
                ms(&ts)?,
            ))?);
            if bytes.len() > wire::MAX_BATCH {
                return Err(SessionError::new(SessionErrorCode::SizeLimit));
            }
            publish(h, &ts, &bytes)?;
        }
        HandleState::Existing { leaf, .. } => {
            let ts = h.manager.runtime.now();
            let mut used = current_ids(h)?;
            let uid = entry_id(h.manager.runtime.as_ref(), &used);
            used.push(uid.clone());
            let aid = entry_id(h.manager.runtime.as_ref(), &used);
            let mut bytes = wire::line(wire::user(
                &uid,
                leaf.as_deref(),
                &ts,
                &turn.user_text,
                ms(&ts)?,
            ))?;
            bytes.extend(wire::line(wire::assistant(
                &aid,
                &uid,
                &ts,
                &turn,
                ms(&ts)?,
            ))?);
            if bytes.len() > wire::MAX_BATCH {
                return Err(SessionError::new(SessionErrorCode::SizeLimit));
            }
            append_bytes(h, &bytes)?;
        }
    }
    refresh(h)
}
fn publish(h: &mut SessionHandle, ts: &str, bytes: &[u8]) -> Result<(), SessionError> {
    let id = h.snapshot.id.clone();
    h.manager
        .runtime
        .checkpoint(SessionCheckpoint::Lock)
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let owner = open_lock(&lock_path(&h.manager.root, &id, "owner"))?;
    owner
        .try_lock_exclusive()
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let final_path = h.manager.root.join(timestamp_filename(ts, &id));
    let temp = h
        .manager
        .root
        .join(format!(".session-{}-{}.tmp", id, h.manager.runtime.uuid()));
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        o.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut f = o
        .open(&temp)
        .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
    #[cfg(windows)]
    harden_windows_path(&temp, false).map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?;
    let midpoint = bytes.len() / 2;
    if f.write_all(&bytes[..midpoint]).is_err()
        || h.manager
            .runtime
            .checkpoint(SessionCheckpoint::TempWrite)
            .is_err()
        || f.write_all(&bytes[midpoint..]).is_err()
        || h.manager
            .runtime
            .checkpoint(SessionCheckpoint::TempSync)
            .is_err()
        || f.sync_all().is_err()
    {
        let _ = fs::remove_file(&temp);
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    if h.manager
        .runtime
        .checkpoint(SessionCheckpoint::Publish)
        .is_err()
        || atomic_rename_noreplace(&temp, &final_path).is_err()
    {
        let _ = fs::remove_file(&temp);
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    // Publication already happened. A following directory-sync failure is reported
    // deterministically as NotSaved and poisons this handle, while a clean reopen can
    // validate and use the complete published file.
    h.state = HandleState::Existing {
        path: final_path,
        owner: Some(owner),
        expected_len: bytes.len() as u64,
        expected_hash: Sha256::digest(bytes).into(),
        leaf: None,
    };
    if h.manager
        .runtime
        .checkpoint(SessionCheckpoint::DirectorySync)
        .is_err()
        || sync_dir(&h.manager.root).is_err()
    {
        h.poisoned = true;
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    Ok(())
}
fn append_bytes(h: &mut SessionHandle, bytes: &[u8]) -> Result<(), SessionError> {
    let HandleState::Existing {
        path,
        owner: Some(_),
        expected_len,
        expected_hash,
        leaf,
    } = &h.state
    else {
        return Err(SessionError::new(SessionErrorCode::Locked));
    };
    let path = path.clone();
    let expected = *expected_len;
    let expected_hash = *expected_hash;
    let expected_leaf = leaf.clone();
    h.manager
        .runtime
        .checkpoint(SessionCheckpoint::Lock)
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let data = open_lock(&lock_path(&h.manager.root, &h.snapshot.id, "data"))?;
    data.lock_exclusive()
        .map_err(|_| SessionError::new(SessionErrorCode::Locked))?;
    let mut session_file = open_private_session(&path, true)?;
    let mut current = Vec::new();
    session_file
        .read_to_end(&mut current)
        .map_err(|_| SessionError::new(SessionErrorCode::Damaged))?;
    let doc = wire::parse(&current, Some(h.snapshot.id.uuid()))?;
    let (_, _, actual_leaf) = reconstruct(&doc, false, None)?;
    if current.len() as u64 != expected
        || <[u8; 32]>::from(Sha256::digest(&current)) != expected_hash
        || actual_leaf != expected_leaf
    {
        h.poisoned = true;
        let _ = FileExt::unlock(&data);
        return Err(SessionError::new(SessionErrorCode::ExternalChange));
    }
    if !path_matches_file(&path, &session_file)? {
        h.poisoned = true;
        let _ = FileExt::unlock(&data);
        return Err(SessionError::new(SessionErrorCode::ExternalChange));
    }
    let midpoint = bytes.len() / 2;
    let result = if let Err(error) = session_file.write_all(&bytes[..midpoint]) {
        Err(error)
    } else if h
        .manager
        .runtime
        .checkpoint(SessionCheckpoint::AppendWrite)
        .is_err()
    {
        Err(std::io::Error::other("injected append write failure"))
    } else if let Err(error) = session_file.write_all(&bytes[midpoint..]) {
        Err(error)
    } else if h
        .manager
        .runtime
        .checkpoint(SessionCheckpoint::AppendSync)
        .is_err()
    {
        Err(std::io::Error::other("injected append sync failure"))
    } else {
        session_file.sync_all()
    };
    let identity_matches = path_matches_file(&path, &session_file).unwrap_or(false);
    let _ = FileExt::unlock(&data);
    if result.is_err() || !identity_matches {
        h.poisoned = true;
        return Err(SessionError::new(SessionErrorCode::NotSaved));
    }
    Ok(())
}
fn refresh(h: &mut SessionHandle) -> Result<(), SessionError> {
    let HandleState::Existing { path, owner, .. } = &h.state else {
        return Ok(());
    };
    let path = path.clone();
    let owner_clone = owner.as_ref().and_then(|f| f.try_clone().ok());
    let (s, c, l, len, _) = load(&h.manager, &h.snapshot.id, &path, true)?;
    h.snapshot = s;
    h.continuation = c;
    let expected_hash = hash_path(&path)?;
    h.state = HandleState::Existing {
        path,
        owner: owner_clone,
        expected_len: len,
        expected_hash,
        leaf: l,
    };
    Ok(())
}
fn path_matches_file(path: &Path, file: &File) -> Result<bool, SessionError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| SessionError::new(SessionErrorCode::ExternalChange))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Ok(false);
    }
    let file_metadata = file
        .metadata()
        .map_err(|_| SessionError::new(SessionErrorCode::ExternalChange))?;
    #[cfg(unix)]
    {
        return Ok(path_metadata.dev() == file_metadata.dev()
            && path_metadata.ino() == file_metadata.ino());
    }
    #[cfg(windows)]
    {
        use std::mem::zeroed;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_OPEN_REPARSE_POINT,
        };

        if unsafe_path_metadata(&path_metadata) {
            return Ok(false);
        }
        let path_file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| SessionError::new(SessionErrorCode::ExternalChange))?;
        if unsafe_path_metadata(
            &path_file
                .metadata()
                .map_err(|_| SessionError::new(SessionErrorCode::ExternalChange))?,
        ) {
            return Ok(false);
        }

        fn identity(file: &File) -> Result<(u32, u64), SessionError> {
            let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
                return Err(SessionError::new(SessionErrorCode::ExternalChange));
            }
            Ok((
                information.dwVolumeSerialNumber,
                (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            ))
        }

        Ok(identity(&path_file)? == identity(file)?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(path_metadata.len() == file_metadata.len()
            && path_metadata.modified().ok() == file_metadata.modified().ok())
    }
}

fn hash_path(path: &Path) -> Result<[u8; 32], SessionError> {
    Ok(Sha256::digest(read_private(path)?).into())
}

fn current_ids(h: &SessionHandle) -> Result<Vec<String>, SessionError> {
    let HandleState::Existing { path, .. } = &h.state else {
        return Ok(Vec::new());
    };
    let bytes = read_private(path)?;
    let doc = wire::parse(&bytes, Some(h.snapshot.id.uuid()))?;
    Ok(doc.entries.into_iter().map(|entry| entry.id).collect())
}

fn sync_dir(path: &Path) -> Result<(), SessionError> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn atomic_rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both C strings are live and NUL terminated for the duration of the call.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn atomic_rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // Darwin's RENAME_EXCL gives rename semantics while failing if `to` exists.
    const RENAME_EXCL: u32 = 0x0000_0004;
    // SAFETY: both C strings are live and NUL terminated for the duration of the call.
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn atomic_rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // No MOVEFILE_REPLACE_EXISTING flag: Windows atomically fails on collision.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
compile_error!("Agent session publication needs a native no-replace rename implementation");
fn metadata(
    h: &mut SessionHandle,
    make: impl FnOnce(&str, Option<&str>, &str) -> Value,
) -> Result<(), SessionError> {
    ensure_mutable(h)?;
    let ts = h.manager.runtime.now();
    let parent = match &h.state {
        HandleState::Draft { leaf, .. } | HandleState::Existing { leaf, .. } => leaf.as_deref(),
    };
    let used = match &h.state {
        HandleState::Draft { queued, .. } => queued
            .iter()
            .filter_map(|value| value.get("id")?.as_str().map(str::to_owned))
            .collect(),
        HandleState::Existing { .. } => current_ids(h)?,
    };
    let id = entry_id(h.manager.runtime.as_ref(), &used);
    let bytes = wire::line(make(&id, parent, &ts))?;
    if matches!(h.state, HandleState::Draft { .. }) {
        draft_append(
            h,
            serde_json::from_slice(&bytes[..bytes.len() - 1])
                .map_err(|_| SessionError::new(SessionErrorCode::NotSaved))?,
            id,
        )?;
    } else {
        append_bytes(h, &bytes)?;
        refresh(h)?
    }
    Ok(())
}
pub(crate) fn append_model(
    h: &mut SessionHandle,
    p: ProviderId,
    m: ModelId,
) -> Result<(), SessionError> {
    let draft = matches!(h.state, HandleState::Draft { .. });
    metadata(h, |id, parent, ts| {
        wire::model(id, parent, ts, p.as_str(), m.as_str())
    })?;
    if draft {
        h.snapshot.selected_provider = Some(p);
        h.snapshot.selected_model = Some(m);
    }
    Ok(())
}
pub(crate) fn append_reasoning(
    h: &mut SessionHandle,
    l: ReasoningLevel,
) -> Result<(), SessionError> {
    let value = match l {
        ReasoningLevel::Off => "off",
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::XHigh => "xhigh",
        ReasoningLevel::Max => "max",
    };
    let draft = matches!(h.state, HandleState::Draft { .. });
    metadata(h, |id, parent, ts| wire::reasoning(id, parent, ts, value))?;
    if draft {
        h.snapshot.reasoning_level = l;
    }
    Ok(())
}
pub(crate) fn append_name(h: &mut SessionHandle, name: String) -> Result<(), SessionError> {
    let mut sanitized = String::with_capacity(name.len());
    let mut newline_run = false;
    for character in name.chars() {
        if matches!(character, '\r' | '\n') {
            if !newline_run {
                sanitized.push(' ');
            }
            newline_run = true;
        } else {
            newline_run = false;
            sanitized.push(character);
        }
    }
    let clean = sanitized.trim().to_owned();
    let draft = matches!(h.state, HandleState::Draft { .. });
    metadata(h, |id, parent, ts| wire::name(id, parent, ts, &clean))?;
    if draft {
        h.snapshot.display_name = if clean.is_empty() {
            "Untitled session".into()
        } else {
            clean
        };
    }
    Ok(())
}
pub(crate) fn append_compaction(
    h: &mut SessionHandle,
    r: CompactionRecord,
) -> Result<(), SessionError> {
    if matches!(h.state, HandleState::Draft { .. })
        || !valid_compaction_boundary(h, &r.first_kept_entry_id)?
    {
        return Err(SessionError::new(SessionErrorCode::Damaged));
    }
    metadata(h, |id, parent, ts| wire::compaction(id, parent, ts, &r))
}

fn valid_compaction_boundary(h: &SessionHandle, kept: &str) -> Result<bool, SessionError> {
    let HandleState::Existing { path, .. } = &h.state else {
        return Ok(false);
    };
    let bytes = read_private(path)?;
    let doc = wire::parse(&bytes, Some(h.snapshot.id.uuid()))?;
    let by_id: std::collections::HashMap<&str, &wire::Entry> = doc
        .entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    let mut current = doc.entries.last();
    while let Some(entry) = current {
        if entry.id == kept
            && matches!(
                entry.kind,
                wire::EntryKind::User(_) | wire::EntryKind::Assistant { .. }
            )
        {
            return Ok(true);
        }
        current = entry
            .parent
            .as_deref()
            .and_then(|parent| by_id.get(parent).copied());
    }
    Ok(false)
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::{harden_windows_path, unsafe_path_metadata};
    use std::os::windows::fs::symlink_file;

    #[test]
    fn native_acl_helper_hardens_files_and_reparse_metadata_fails_closed() {
        let temp = tempfile::TempDir::new().unwrap();
        let file = temp.path().join("private.jsonl");
        std::fs::write(&file, b"synthetic").unwrap();
        harden_windows_path(&file, false).unwrap();
        assert!(!unsafe_path_metadata(
            &std::fs::symlink_metadata(&file).unwrap()
        ));

        let link = temp.path().join("unsafe.jsonl");
        if symlink_file(&file, &link).is_ok() {
            assert!(unsafe_path_metadata(
                &std::fs::symlink_metadata(link).unwrap()
            ));
        }
    }
}
