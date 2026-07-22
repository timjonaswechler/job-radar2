//! App-owned managed Chromium process and CDP session lifecycle.
//!
//! This module deliberately does not use `Browser::launch`: Job Radar must own
//! the process tree before any cancellable endpoint or CDP work begins.

use chromiumoxide::{browser::Browser, handler::Handler};
use futures_util::{FutureExt, StreamExt};
use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, task::JoinHandle};
use uuid::Uuid;

use super::{
    begin_active_browser_session, current_runtime_spec, status_for_runtime_dir,
    ActiveBrowserSession, BrowserRuntimeState,
};

const DROP_REAP_TIMEOUT: Duration = Duration::from_secs(1);
const ENDPOINT_FILE: &str = "DevToolsActivePort";
const ENDPOINT_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_ENDPOINT_FILE_BYTES: usize = 4_096;
const STABLE_MALFORMED_OBSERVATIONS: usize = 3;

#[derive(Debug)]
struct ProcessTreeSpawnError {
    source: io::Error,
    cleanup_unconfirmed: bool,
}

impl ProcessTreeSpawnError {
    fn launch(source: io::Error) -> Self {
        Self {
            source,
            cleanup_unconfirmed: false,
        }
    }

    fn cleanup(source: io::Error) -> Self {
        Self {
            source,
            cleanup_unconfirmed: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OwnedChromiumDeadlines {
    pub work: tokio::time::Instant,
    pub graceful: tokio::time::Instant,
    pub force: tokio::time::Instant,
    pub handler: tokio::time::Instant,
    pub finalize: tokio::time::Instant,
}

impl OwnedChromiumDeadlines {
    fn validate(self) -> Result<Self, OwnedChromiumError> {
        if self.work <= self.graceful
            && self.graceful <= self.force
            && self.force <= self.handler
            && self.handler <= self.finalize
        {
            Ok(self)
        } else {
            Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::InvalidControl,
                "owned Chromium deadlines must be monotonic",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedChromiumErrorKind {
    RuntimeUnavailable,
    InvalidControl,
    Launch,
    EndpointDiscovery,
    Connect,
    Cancelled,
    Deadline,
    Cleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedChromiumError {
    pub kind: OwnedChromiumErrorKind,
    pub message: String,
}

impl OwnedChromiumError {
    fn new(kind: OwnedChromiumErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub(crate) struct OwnedChromiumLauncher {
    executable_path: PathBuf,
    runtime_dir: PathBuf,
    #[cfg(test)]
    arguments_override: Option<Vec<String>>,
}

impl OwnedChromiumLauncher {
    /// Resolves only the current pinned, installed Job Radar runtime.
    pub(crate) fn from_installed_runtime(runtime_dir: &Path) -> Result<Self, OwnedChromiumError> {
        let spec = current_runtime_spec().ok_or_else(|| {
            OwnedChromiumError::new(
                OwnedChromiumErrorKind::RuntimeUnavailable,
                "managed Chromium is unsupported on this platform",
            )
        })?;
        let status = status_for_runtime_dir(runtime_dir, Some(&spec), false);
        if status.status != BrowserRuntimeState::Installed {
            return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::RuntimeUnavailable,
                format!(
                    "pinned managed Chromium is not installed: {:?}",
                    status.status
                ),
            ));
        }
        let executable_path = status.executable_path.map(PathBuf::from).ok_or_else(|| {
            OwnedChromiumError::new(
                OwnedChromiumErrorKind::RuntimeUnavailable,
                "installed managed Chromium has no executable path",
            )
        })?;
        Ok(Self {
            executable_path,
            runtime_dir: runtime_dir.to_path_buf(),
            #[cfg(test)]
            arguments_override: None,
        })
    }

    pub(crate) async fn launch<C>(
        &self,
        deadlines: OwnedChromiumDeadlines,
        cancellation: C,
    ) -> Result<OwnedChromiumSession, OwnedChromiumError>
    where
        C: Future<Output = ()> + Send,
    {
        self.launch_executable(deadlines, cancellation).await
    }

    async fn launch_executable<C>(
        &self,
        deadlines: OwnedChromiumDeadlines,
        cancellation: C,
    ) -> Result<OwnedChromiumSession, OwnedChromiumError>
    where
        C: Future<Output = ()> + Send,
    {
        let deadlines = deadlines.validate()?;
        let mut cancellation = Box::pin(cancellation);
        if tokio::time::Instant::now() >= deadlines.work {
            return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium launch work deadline already expired",
            ));
        }

        let session_dir = self
            .runtime_dir
            .join(".tmp")
            .join(format!("session-{}", Uuid::new_v4()));
        let mut active_session = begin_active_browser_session(&session_dir);
        let create_result = tokio::select! {
            biased;
            _ = &mut cancellation => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled during session setup",
            )),
            result = create_private_session_dir(&session_dir) => result.map_err(|error| OwnedChromiumError::new(
                OwnedChromiumErrorKind::Launch,
                format!("failed to create private owned Chromium session: {error}"),
            )),
            _ = tokio::time::sleep_until(deadlines.work) => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium session setup exceeded its work deadline",
            )),
        };
        if let Err(primary) = create_result {
            return match cleanup_session_dir(&session_dir, deadlines.force).await {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(cleanup),
            };
        }

        let clear_result = tokio::select! {
            biased;
            _ = &mut cancellation => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled while clearing stale endpoint state",
            )),
            result = clear_stale_endpoint_file(&session_dir) => result,
            _ = tokio::time::sleep_until(deadlines.work) => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium stale endpoint cleanup exceeded its work deadline",
            )),
        };
        if let Err(primary) = clear_result {
            return match cleanup_session_dir(&session_dir, deadlines.force).await {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(cleanup),
            };
        }

        #[cfg(not(test))]
        let args = chromium_arguments(&session_dir);
        #[cfg(test)]
        let args = self
            .arguments_override
            .clone()
            .unwrap_or_else(|| chromium_arguments(&session_dir));
        // Native spawn plus process-tree ownership is one synchronous atomic
        // transaction. It is deliberately not advertised as interruptible;
        // control is observed immediately before and after this call.
        let mut process = match ProcessTree::spawn(
            &self.executable_path,
            &args,
            &[],
            deadlines.force,
            &session_dir,
        ) {
            Ok(process) => process,
            Err(error) => {
                let kind = if error.cleanup_unconfirmed {
                    active_session.quarantine();
                    OwnedChromiumErrorKind::Cleanup
                } else {
                    OwnedChromiumErrorKind::Launch
                };
                let primary = OwnedChromiumError::new(
                    kind,
                    format!("failed to create owned Chromium process: {}", error.source),
                );
                if error.cleanup_unconfirmed {
                    return Err(primary);
                }
                return match cleanup_session_dir(&session_dir, deadlines.force).await {
                    Ok(()) => Err(primary),
                    Err(cleanup) => Err(cleanup),
                };
            }
        };

        let post_spawn_primary = if cancellation.as_mut().now_or_never().is_some() {
            Some(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled immediately after process creation",
            ))
        } else if tokio::time::Instant::now() >= deadlines.work {
            Some(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium process creation exceeded its work deadline",
            ))
        } else {
            None
        };
        if let Some(primary) = post_spawn_primary {
            let error =
                finalize_failed_launch(primary, process, &session_dir, deadlines.force).await;
            if error.kind == OwnedChromiumErrorKind::Cleanup {
                active_session.quarantine();
            }
            return Err(error);
        }

        let endpoint_result = discover_endpoint(
            &session_dir,
            &mut process,
            deadlines.work,
            &mut cancellation,
        )
        .await;
        let endpoint = match endpoint_result {
            Ok(endpoint) => endpoint,
            Err(primary) => {
                let error =
                    finalize_failed_launch(primary, process, &session_dir, deadlines.force).await;
                if error.kind == OwnedChromiumErrorKind::Cleanup {
                    active_session.quarantine();
                }
                return Err(error);
            }
        };

        let connect = Browser::connect(endpoint.websocket_url());
        tokio::pin!(connect);
        let connection_result = tokio::select! {
            biased;
            _ = &mut cancellation => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled during CDP connection",
            )),
            result = &mut connect => result.map_err(|error| OwnedChromiumError::new(
                OwnedChromiumErrorKind::Connect,
                format!("failed to connect to owned Chromium: {error}"),
            )),
            _ = tokio::time::sleep_until(deadlines.work) => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium CDP connection exceeded its work deadline",
            )),
        };
        let (browser, handler) = match connection_result {
            Ok(connection) => connection,
            Err(primary) => {
                let error =
                    finalize_failed_launch(primary, process, &session_dir, deadlines.force).await;
                if error.kind == OwnedChromiumErrorKind::Cleanup {
                    active_session.quarantine();
                }
                return Err(error);
            }
        };

        let handler_task = spawn_handler(handler);
        Ok(OwnedChromiumSession {
            browser: Some(browser),
            handler_task: Some(handler_task),
            process: Some(process),
            session_dir,
            active_session: Some(active_session),
            deadlines,
        })
    }

    #[cfg(test)]
    fn for_executable(
        executable_path: PathBuf,
        runtime_dir: PathBuf,
        arguments: Vec<String>,
    ) -> Self {
        Self {
            executable_path,
            runtime_dir,
            arguments_override: Some(arguments),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnedChromiumShutdown {
    pub forced: bool,
}

#[must_use = "owned Chromium sessions must be shut down and awaited"]
pub(crate) struct OwnedChromiumSession {
    browser: Option<Browser>,
    handler_task: Option<JoinHandle<()>>,
    process: Option<ProcessTree>,
    session_dir: PathBuf,
    active_session: Option<ActiveBrowserSession>,
    deadlines: OwnedChromiumDeadlines,
}

impl OwnedChromiumSession {
    pub(crate) async fn shutdown(mut self) -> Result<OwnedChromiumShutdown, OwnedChromiumError> {
        let deadlines = self.deadlines;
        let mut forced = false;

        if let Some(browser) = self.browser.as_mut() {
            let _ = tokio::time::timeout_at(deadlines.graceful, browser.close()).await;
        }
        self.browser.take();

        // Keep process ownership in `self` while awaiting. If this future is
        // dropped, `OwnedChromiumSession::drop` still reaches ProcessTree's
        // synchronous terminate-and-observe fallback.
        let process_error = match finalize_owned_process(
            self.process
                .as_mut()
                .expect("owned process is present until shutdown"),
            deadlines.graceful,
            deadlines.force,
        )
        .await
        {
            Ok(was_forced) => {
                forced = was_forced;
                None
            }
            Err(error) => Some(error),
        };

        if let Some(mut handler_task) = self.handler_task.take() {
            if tokio::time::timeout_at(deadlines.handler, &mut handler_task)
                .await
                .is_err()
            {
                handler_task.abort();
                let _ = handler_task.await;
            }
        }

        if process_error.is_some() {
            if let Some(active_session) = self.active_session.as_mut() {
                active_session.quarantine();
            }
            self.active_session.take();
            return Err(process_error.expect("checked above"));
        }

        // Drop the confirmed process owner while active protection is retained.
        // Its bounded synchronous Drop re-check cannot race status cleanup.
        self.process.take();
        let cleanup_result = cleanup_session_dir(&self.session_dir, deadlines.finalize).await;
        // Keep status cleanup excluded until our own filesystem finalization has
        // reached a terminal result. The process tree is already confirmed gone,
        // so safe residue after a typed filesystem failure need not be quarantined.
        self.active_session.take();
        cleanup_result?;
        Ok(OwnedChromiumShutdown { forced })
    }

    pub(super) fn browser_mut(&mut self) -> &mut Browser {
        self.browser.as_mut().expect("browser remains connected")
    }
}

impl Drop for OwnedChromiumSession {
    fn drop(&mut self) {
        // Abort application work first. The following field drop then performs
        // synchronous bounded whole-tree termination and observation.
        if let Some(handler) = self.handler_task.as_ref() {
            handler.abort();
        }
    }
}

fn spawn_handler(mut handler: Handler) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = handler.next().await {
            if message.is_err() {
                break;
            }
        }
    })
}

async fn finalize_failed_launch(
    primary: OwnedChromiumError,
    mut process: ProcessTree,
    session_dir: &Path,
    force_deadline: tokio::time::Instant,
) -> OwnedChromiumError {
    let cleanup = async {
        force_terminate_and_reap(&mut process, force_deadline).await?;
        cleanup_session_dir(session_dir, force_deadline).await
    }
    .await;
    cleanup.err().unwrap_or(primary)
}

async fn cleanup_session_dir(
    session_dir: &Path,
    deadline: tokio::time::Instant,
) -> Result<(), OwnedChromiumError> {
    match tokio::time::timeout_at(deadline, tokio::fs::remove_dir_all(session_dir)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(Err(error)) => Err(OwnedChromiumError::new(
            OwnedChromiumErrorKind::Cleanup,
            format!("failed to finalize owned Chromium session directory: {error}"),
        )),
        Err(_) => Err(OwnedChromiumError::new(
            OwnedChromiumErrorKind::Cleanup,
            "owned Chromium session finalization exceeded its deadline",
        )),
    }
}

fn chromium_arguments(session_dir: &Path) -> Vec<String> {
    vec![
        "--remote-debugging-address=127.0.0.1".to_string(),
        "--remote-debugging-port=0".to_string(),
        format!("--user-data-dir={}", session_dir.display()),
        "--headless=new".to_string(),
        "--no-first-run".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--disable-component-update".to_string(),
        "about:blank".to_string(),
    ]
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DevToolsEndpoint {
    port: u16,
    browser_path: String,
}

impl DevToolsEndpoint {
    fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}{}", self.port, self.browser_path)
    }
}

fn parse_devtools_active_port(contents: &str) -> Result<DevToolsEndpoint, String> {
    let normalized = contents.strip_suffix('\n').unwrap_or(contents);
    if normalized.ends_with('\n') || normalized.contains('\r') {
        return Err("DevToolsActivePort must contain exactly two LF-delimited lines".to_string());
    }
    let mut lines = normalized.split('\n');
    let port_text = lines.next().unwrap_or_default();
    let browser_path = lines.next().unwrap_or_default();
    if lines.next().is_some() || port_text.is_empty() || browser_path.is_empty() {
        return Err("DevToolsActivePort must contain exactly two non-empty lines".to_string());
    }
    let port = port_text
        .chars()
        .all(|character| character.is_ascii_digit())
        .then(|| port_text.parse::<u16>().ok())
        .flatten()
        .filter(|port| *port != 0)
        .ok_or_else(|| "DevToolsActivePort contains an invalid TCP port".to_string())?;
    let browser_id = browser_path
        .strip_prefix("/devtools/browser/")
        .unwrap_or_default();
    if browser_id.is_empty()
        || !browser_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("DevToolsActivePort contains an invalid browser websocket path".to_string());
    }
    Ok(DevToolsEndpoint {
        port,
        browser_path: browser_path.to_string(),
    })
}

async fn create_private_session_dir(session_dir: &Path) -> io::Result<()> {
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    // Windows inherits the protected per-user ACL of Job Radar's app-data runtime root;
    // Unix must opt out of DirBuilder's world-readable 0o777 default explicitly.
    builder.create(session_dir).await
}

async fn clear_stale_endpoint_file(session_dir: &Path) -> Result<(), OwnedChromiumError> {
    match tokio::fs::remove_file(session_dir.join(ENDPOINT_FILE)).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(OwnedChromiumError::new(
            OwnedChromiumErrorKind::EndpointDiscovery,
            format!("failed to clear stale DevToolsActivePort: {error}"),
        )),
    }
}

async fn read_bounded_endpoint_file(path: &Path) -> io::Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::with_capacity(128);
    file.take((MAX_ENDPOINT_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    Ok(bytes)
}

#[derive(Default)]
struct EndpointCandidateTracker {
    previous_malformed: Option<Vec<u8>>,
    repeated_observations: usize,
}

impl EndpointCandidateTracker {
    fn observe(&mut self, bytes: Vec<u8>) -> Result<Option<DevToolsEndpoint>, OwnedChromiumError> {
        if bytes.len() > MAX_ENDPOINT_FILE_BYTES {
            return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::EndpointDiscovery,
                format!("DevToolsActivePort exceeds the {MAX_ENDPOINT_FILE_BYTES}-byte limit"),
            ));
        }
        let parse_result = std::str::from_utf8(&bytes)
            .map_err(|_| "DevToolsActivePort is not valid UTF-8".to_string())
            .and_then(parse_devtools_active_port);
        match parse_result {
            Ok(endpoint) => Ok(Some(endpoint)),
            Err(message) => {
                if self.previous_malformed.as_deref() == Some(bytes.as_slice()) {
                    self.repeated_observations += 1;
                } else {
                    self.previous_malformed = Some(bytes);
                    self.repeated_observations = 1;
                }
                if self.repeated_observations >= STABLE_MALFORMED_OBSERVATIONS {
                    Err(OwnedChromiumError::new(
                        OwnedChromiumErrorKind::EndpointDiscovery,
                        format!("stable malformed DevToolsActivePort: {message}"),
                    ))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

async fn discover_endpoint<C>(
    session_dir: &Path,
    process: &mut ProcessTree,
    work_deadline: tokio::time::Instant,
    cancellation: &mut Pin<Box<C>>,
) -> Result<DevToolsEndpoint, OwnedChromiumError>
where
    C: Future<Output = ()> + Send,
{
    let endpoint_path = session_dir.join(ENDPOINT_FILE);
    let mut tracker = EndpointCandidateTracker::default();
    loop {
        if process.try_wait()?.is_some() {
            return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::EndpointDiscovery,
                "owned Chromium exited before publishing its DevTools endpoint",
            ));
        }
        let read_result = tokio::select! {
            biased;
            _ = &mut *cancellation => return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled during endpoint discovery",
            )),
            result = read_bounded_endpoint_file(&endpoint_path) => result,
            _ = tokio::time::sleep_until(work_deadline) => return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium endpoint discovery exceeded its work deadline",
            )),
        };
        match read_result {
            Ok(bytes) => {
                if let Some(endpoint) = tracker.observe(bytes)? {
                    return Ok(endpoint);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                tracker = EndpointCandidateTracker::default();
            }
            Err(error) => {
                return Err(OwnedChromiumError::new(
                    OwnedChromiumErrorKind::EndpointDiscovery,
                    format!("failed to read DevToolsActivePort: {error}"),
                ))
            }
        }
        tokio::select! {
            biased;
            _ = &mut *cancellation => return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cancelled,
                "owned Chromium launch cancelled during endpoint discovery",
            )),
            _ = tokio::time::sleep_until(work_deadline) => return Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Deadline,
                "owned Chromium endpoint discovery exceeded its work deadline",
            )),
            _ = tokio::time::sleep(ENDPOINT_POLL_INTERVAL) => {}
        }
    }
}

trait ForceReapProcess {
    fn initiate_force_termination(&mut self) -> io::Result<()>;

    fn wait_until(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Option<()>, OwnedChromiumError>> + Send + '_>>;

    fn wait_for_reap(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), OwnedChromiumError>> + Send + '_>>;
}

async fn finalize_owned_process(
    process: &mut impl ForceReapProcess,
    graceful_deadline: tokio::time::Instant,
    force_deadline: tokio::time::Instant,
) -> Result<bool, OwnedChromiumError> {
    match process.wait_until(graceful_deadline).await {
        Ok(Some(())) => Ok(false),
        // An observation error is not permission to return and rely on Drop.
        // Establish the cleanup invariant through the independent forced path.
        Ok(None) | Err(_) => {
            force_terminate_and_reap(process, force_deadline).await?;
            Ok(true)
        }
    }
}

async fn force_terminate_and_reap(
    process: &mut impl ForceReapProcess,
    deadline: tokio::time::Instant,
) -> Result<(), OwnedChromiumError> {
    process.initiate_force_termination().map_err(|error| {
        OwnedChromiumError::new(
            OwnedChromiumErrorKind::Cleanup,
            format!("failed to initiate owned Chromium tree termination: {error}"),
        )
    })?;
    match tokio::time::timeout_at(deadline, process.wait_for_reap()).await {
        Ok(result) => result,
        Err(_) => Err(OwnedChromiumError::new(
            OwnedChromiumErrorKind::Cleanup,
            "owned Chromium forced termination was initiated but reap exceeded its deadline",
        )),
    }
}

#[cfg(unix)]
struct ProcessTree {
    child: tokio::process::Child,
    process_group: i32,
    kill_initiated: bool,
    cleanup_confirmed: bool,
    protection: ActiveBrowserSession,
}

#[cfg(unix)]
impl ProcessTree {
    fn spawn(
        executable: &Path,
        args: &[String],
        env: &[(String, String)],
        _force_deadline: tokio::time::Instant,
        session_dir: &Path,
    ) -> Result<Self, ProcessTreeSpawnError> {
        let mut command = tokio::process::Command::new(executable);
        command
            .args(args)
            .envs(env.iter().cloned())
            // Managed Chromium never receives application-owned pipes: a browser
            // cannot block teardown by filling an unread stdout/stderr pipe.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.process_group(0);
        let child = command.spawn().map_err(ProcessTreeSpawnError::launch)?;
        // Tokio guarantees an id for a successfully spawned, not-yet-waited
        // child; supported Unix kernels allocate process ids within i32.
        let process_group = child
            .id()
            .expect("newly spawned process must expose its pid")
            .try_into()
            .expect("supported Unix process ids fit i32");
        Ok(Self {
            child,
            process_group,
            kill_initiated: false,
            cleanup_confirmed: false,
            protection: begin_active_browser_session(session_dir),
        })
    }

    fn try_wait(&mut self) -> Result<Option<()>, OwnedChromiumError> {
        self.child
            .try_wait()
            .map(|status| status.map(|_| ()))
            .map_err(|error| {
                OwnedChromiumError::new(OwnedChromiumErrorKind::Cleanup, error.to_string())
            })
    }

    fn initiate_force_termination(&mut self) -> io::Result<()> {
        if self.kill_initiated {
            return Ok(());
        }
        let result = unsafe { libc::kill(-self.process_group, libc::SIGKILL) };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        self.kill_initiated = true;
        Ok(())
    }

    fn process_group_is_empty(&self) -> Result<bool, OwnedChromiumError> {
        let result = unsafe { libc::kill(-self.process_group, 0) };
        if result == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(true),
            Some(libc::EPERM) => Ok(false),
            Some(libc::EINTR) => Ok(false),
            _ => Err(OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cleanup,
                format!("failed to inspect owned Chromium process group: {error}"),
            )),
        }
    }

    fn lifecycle_is_complete(&mut self) -> Result<bool, OwnedChromiumError> {
        if self.cleanup_confirmed {
            return Ok(true);
        }
        let complete = self.try_wait()?.is_some() && self.process_group_is_empty()?;
        if complete {
            // Persist the observation. Re-probing an absent numeric PGID later
            // would race operating-system reuse of that identifier.
            self.cleanup_confirmed = true;
        }
        Ok(complete)
    }

    async fn wait_until(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Result<Option<()>, OwnedChromiumError> {
        loop {
            if self.lifecycle_is_complete()? {
                return Ok(Some(()));
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(None);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn terminate_and_reap_on_drop(&mut self) -> Result<(), OwnedChromiumError> {
        self.initiate_force_termination().map_err(|error| {
            OwnedChromiumError::new(
                OwnedChromiumErrorKind::Cleanup,
                format!("failed to initiate dropped Chromium tree termination: {error}"),
            )
        })?;
        let deadline = std::time::Instant::now() + DROP_REAP_TIMEOUT;
        loop {
            if self.lifecycle_is_complete()? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(OwnedChromiumError::new(
                    OwnedChromiumErrorKind::Cleanup,
                    "dropped Chromium tree reap exceeded its bounded deadline",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(unix)]
impl ForceReapProcess for ProcessTree {
    fn initiate_force_termination(&mut self) -> io::Result<()> {
        ProcessTree::initiate_force_termination(self)
    }

    fn wait_until(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Option<()>, OwnedChromiumError>> + Send + '_>> {
        Box::pin(ProcessTree::wait_until(self, deadline))
    }

    fn wait_for_reap(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), OwnedChromiumError>> + Send + '_>> {
        Box::pin(async move {
            loop {
                if self.lifecycle_is_complete()? {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }
}

#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        // Do not signal a PGID after successful observation: once the group is
        // absent, the numeric ID may be reused by an unrelated process group.
        if self.cleanup_confirmed || self.lifecycle_is_complete() == Ok(true) {
            return;
        }
        if self.terminate_and_reap_on_drop().is_err() {
            self.protection.quarantine();
        }
    }
}

#[cfg(windows)]
mod windows_process {
    use super::*;
    use std::{mem, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
                SetInformationJobObject, TerminateJobObject,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                CreateProcessW, ResumeThread, CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    };

    pub(super) struct ProcessTree {
        process: HANDLE,
        job: HANDLE,
        kill_initiated: bool,
        cleanup_confirmed: bool,
        protection: ActiveBrowserSession,
    }

    unsafe impl Send for ProcessTree {}

    impl ProcessTree {
        pub(super) fn spawn(
            executable: &Path,
            args: &[String],
            env: &[(String, String)],
            force_deadline: tokio::time::Instant,
            session_dir: &Path,
        ) -> Result<Self, ProcessTreeSpawnError> {
            if !env.is_empty() {
                return Err(ProcessTreeSpawnError::launch(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "raw Windows launcher test environment is unsupported",
                )));
            }
            let executable_wide: Vec<u16> = executable
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            let mut command_line: Vec<u16> =
                std::iter::once(quote_windows_argument(&executable.to_string_lossy()))
                    .chain(args.iter().map(|arg| quote_windows_argument(arg)))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
            let mut startup: STARTUPINFOW = unsafe { mem::zeroed() };
            startup.cb = mem::size_of::<STARTUPINFOW>() as u32;
            let mut info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
            // bInheritHandles is false and STARTF_USESTDHANDLES is absent: the
            // browser receives no application-owned stdio pipes that could fill.
            let created = unsafe {
                CreateProcessW(
                    executable_wide.as_ptr(),
                    command_line.as_mut_ptr(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    CREATE_SUSPENDED,
                    ptr::null(),
                    ptr::null(),
                    &startup,
                    &mut info,
                )
            };
            if created == 0 {
                return Err(ProcessTreeSpawnError::launch(io::Error::last_os_error()));
            }
            let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if job.is_null() {
                let primary = io::Error::last_os_error();
                let cleanup = unsafe { terminate_and_wait_partial(&info, None, force_deadline) };
                unsafe { close_process_information(&info) };
                return Err(spawn_failure(primary, cleanup));
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    mem::size_of_val(&limits) as u32,
                )
            };
            if configured == 0 {
                let primary = io::Error::last_os_error();
                let cleanup =
                    unsafe { terminate_and_wait_partial(&info, Some(job), force_deadline) };
                unsafe {
                    close_process_information(&info);
                    CloseHandle(job);
                }
                return Err(spawn_failure(primary, cleanup));
            }
            if unsafe { AssignProcessToJobObject(job, info.hProcess) } == 0 {
                let primary = io::Error::last_os_error();
                let cleanup =
                    unsafe { terminate_and_wait_partial(&info, Some(job), force_deadline) };
                unsafe {
                    close_process_information(&info);
                    CloseHandle(job);
                }
                return Err(spawn_failure(primary, cleanup));
            }
            if unsafe { ResumeThread(info.hThread) } == u32::MAX {
                let primary = io::Error::last_os_error();
                let cleanup =
                    unsafe { terminate_and_wait_partial(&info, Some(job), force_deadline) };
                unsafe {
                    close_process_information(&info);
                    CloseHandle(job);
                }
                return Err(spawn_failure(primary, cleanup));
            }
            unsafe { CloseHandle(info.hThread) };
            Ok(Self {
                process: info.hProcess,
                job,
                kill_initiated: false,
                cleanup_confirmed: false,
                protection: begin_active_browser_session(session_dir),
            })
        }

        pub(super) fn try_wait(&mut self) -> Result<Option<()>, OwnedChromiumError> {
            match unsafe {
                windows_sys::Win32::System::Threading::WaitForSingleObject(self.process, 0)
            } {
                WAIT_OBJECT_0 => Ok(Some(())),
                WAIT_TIMEOUT => Ok(None),
                WAIT_FAILED => Err(OwnedChromiumError::new(
                    OwnedChromiumErrorKind::Cleanup,
                    io::Error::last_os_error().to_string(),
                )),
                other => Err(OwnedChromiumError::new(
                    OwnedChromiumErrorKind::Cleanup,
                    format!("unexpected process wait result {other}"),
                )),
            }
        }

        pub(super) fn initiate_force_termination(&mut self) -> io::Result<()> {
            if self.kill_initiated {
                return Ok(());
            }
            if unsafe { TerminateJobObject(self.job, 1) } == 0 {
                return Err(io::Error::last_os_error());
            }
            self.kill_initiated = true;
            Ok(())
        }

        fn job_is_empty(&self) -> Result<bool, OwnedChromiumError> {
            job_active_processes(self.job)
                .map(|active| active == 0)
                .map_err(|error| {
                    OwnedChromiumError::new(
                        OwnedChromiumErrorKind::Cleanup,
                        format!("failed to inspect owned Chromium Job Object: {error}"),
                    )
                })
        }

        fn lifecycle_is_complete(&mut self) -> Result<bool, OwnedChromiumError> {
            if self.cleanup_confirmed {
                return Ok(true);
            }
            let complete = self.try_wait()?.is_some() && self.job_is_empty()?;
            if complete {
                self.cleanup_confirmed = true;
            }
            Ok(complete)
        }

        pub(super) async fn wait_until(
            &mut self,
            deadline: tokio::time::Instant,
        ) -> Result<Option<()>, OwnedChromiumError> {
            loop {
                if self.lifecycle_is_complete()? {
                    return Ok(Some(()));
                }
                if tokio::time::Instant::now() >= deadline {
                    return Ok(None);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        fn terminate_and_reap_on_drop(&mut self) -> Result<(), OwnedChromiumError> {
            self.initiate_force_termination().map_err(|error| {
                OwnedChromiumError::new(
                    OwnedChromiumErrorKind::Cleanup,
                    format!("failed to initiate dropped Chromium tree termination: {error}"),
                )
            })?;
            let deadline = std::time::Instant::now() + DROP_REAP_TIMEOUT;
            loop {
                if self.lifecycle_is_complete()? {
                    return Ok(());
                }
                if std::time::Instant::now() >= deadline {
                    return Err(OwnedChromiumError::new(
                        OwnedChromiumErrorKind::Cleanup,
                        "dropped Chromium tree reap exceeded its bounded deadline",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl ForceReapProcess for ProcessTree {
        fn initiate_force_termination(&mut self) -> io::Result<()> {
            ProcessTree::initiate_force_termination(self)
        }

        fn wait_until(
            &mut self,
            deadline: tokio::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<Option<()>, OwnedChromiumError>> + Send + '_>>
        {
            Box::pin(ProcessTree::wait_until(self, deadline))
        }

        fn wait_for_reap(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), OwnedChromiumError>> + Send + '_>> {
            Box::pin(async move {
                loop {
                    if self.lifecycle_is_complete()? {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            if !self.cleanup_confirmed && self.terminate_and_reap_on_drop().is_err() {
                self.protection.quarantine();
            }
            unsafe {
                CloseHandle(self.job);
                CloseHandle(self.process);
            }
        }
    }

    pub(super) fn quote_windows_argument(argument: &str) -> String {
        if !argument.is_empty()
            && !argument
                .chars()
                .any(|character| character.is_whitespace() || character == '"')
        {
            return argument.to_string();
        }
        let mut quoted = String::from("\"");
        let mut backslashes = 0;
        for character in argument.chars() {
            if character == '\\' {
                backslashes += 1;
            } else if character == '"' {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            } else {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
        quoted.push_str(&"\\".repeat(backslashes * 2));
        quoted.push('"');
        quoted
    }

    fn job_active_processes(job: HANDLE) -> io::Result<u32> {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { mem::zeroed() };
        let queried = unsafe {
            QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                &mut accounting as *mut _ as *mut _,
                mem::size_of_val(&accounting) as u32,
                ptr::null_mut(),
            )
        };
        if queried == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(accounting.ActiveProcesses)
        }
    }

    fn spawn_failure(primary: io::Error, cleanup: io::Result<()>) -> ProcessTreeSpawnError {
        match cleanup {
            Ok(()) => ProcessTreeSpawnError::launch(primary),
            Err(error) => ProcessTreeSpawnError::cleanup(error),
        }
    }

    unsafe fn terminate_and_wait_partial(
        info: &PROCESS_INFORMATION,
        job: Option<HANDLE>,
        force_deadline: tokio::time::Instant,
    ) -> io::Result<()> {
        use windows_sys::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};

        if let Some(job) = job {
            let _ = TerminateJobObject(job, 1);
        }
        if WaitForSingleObject(info.hProcess, 0) != WAIT_OBJECT_0 {
            let _ = TerminateProcess(info.hProcess, 1);
        }

        loop {
            let root_finished = match WaitForSingleObject(info.hProcess, 0) {
                WAIT_OBJECT_0 => true,
                WAIT_TIMEOUT => false,
                WAIT_FAILED => return Err(io::Error::last_os_error()),
                other => {
                    return Err(io::Error::other(format!(
                        "unexpected process wait result {other}"
                    )))
                }
            };
            let job_empty = match job {
                Some(job) => job_active_processes(job)? == 0,
                None => true,
            };
            if root_finished && job_empty {
                return Ok(());
            }
            if tokio::time::Instant::now() >= force_deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "partial Windows process creation cleanup did not observe an empty process tree",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    unsafe fn close_process_information(info: &PROCESS_INFORMATION) {
        CloseHandle(info.hThread);
        CloseHandle(info.hProcess);
    }
}

#[cfg(windows)]
use windows_process::ProcessTree;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_command_line_quoting_preserves_spaces_quotes_and_trailing_backslashes() {
        use super::windows_process::quote_windows_argument;

        assert_eq!(quote_windows_argument("simple"), "simple");
        assert_eq!(quote_windows_argument(""), "\"\"");
        assert_eq!(quote_windows_argument("two words"), "\"two words\"");
        assert_eq!(quote_windows_argument(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(
            quote_windows_argument("C:\\Program Files\\Chrome\\"),
            "\"C:\\Program Files\\Chrome\\\\\""
        );
    }

    #[test]
    fn devtools_active_port_accepts_one_strict_endpoint_record() {
        assert_eq!(
            parse_devtools_active_port("9222\n/devtools/browser/abc-123\n").unwrap(),
            DevToolsEndpoint {
                port: 9222,
                browser_path: "/devtools/browser/abc-123".to_string(),
            }
        );
    }

    #[test]
    fn devtools_active_port_rejects_ambiguous_or_remote_records() {
        for invalid in [
            "0\n/devtools/browser/id\n",
            "70000\n/devtools/browser/id\n",
            "9222\nhttp://remote/devtools/browser/id\n",
            "9222\n/devtools/browser/\n",
            "9222\n/devtools/browser/id\nextra\n",
            "9222\r\n/devtools/browser/id\r\n",
            "9222\n/devtools/browser/id?token=x\n",
            "+9222\n/devtools/browser/id\n",
            "9222\n/devtools/browser/id/child\n",
        ] {
            assert!(parse_devtools_active_port(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn partial_endpoint_contents_retry_and_can_become_valid() {
        let mut tracker = EndpointCandidateTracker::default();
        assert_eq!(tracker.observe(b"9222\n".to_vec()).unwrap(), None);
        assert_eq!(tracker.observe(b"9222\n".to_vec()).unwrap(), None);

        assert_eq!(
            tracker
                .observe(b"9222\n/devtools/browser/current-id\n".to_vec())
                .unwrap(),
            Some(DevToolsEndpoint {
                port: 9222,
                browser_path: "/devtools/browser/current-id".to_string(),
            })
        );
    }

    #[test]
    fn stable_malformed_endpoint_fails_after_bounded_retries() {
        let mut tracker = EndpointCandidateTracker::default();
        let malformed = b"not-a-port\n/devtools/browser/id\n".to_vec();
        assert_eq!(tracker.observe(malformed.clone()).unwrap(), None);
        assert_eq!(tracker.observe(malformed.clone()).unwrap(), None);
        let error = tracker.observe(malformed).unwrap_err();

        assert_eq!(error.kind, OwnedChromiumErrorKind::EndpointDiscovery);
        assert!(error
            .message
            .starts_with("stable malformed DevToolsActivePort:"));
    }

    #[test]
    fn oversized_endpoint_is_rejected_without_retrying_or_allocating_the_whole_file() {
        let mut tracker = EndpointCandidateTracker::default();
        let error = tracker
            .observe(vec![b'x'; MAX_ENDPOINT_FILE_BYTES + 1])
            .unwrap_err();

        assert_eq!(error.kind, OwnedChromiumErrorKind::EndpointDiscovery);
        assert!(error.message.contains("exceeds the 4096-byte limit"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn session_state_is_created_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let session_dir = temp.path().join("nested/session-private");

        create_private_session_dir(&session_dir).await.unwrap();

        assert_eq!(
            std::fs::metadata(session_dir).unwrap().permissions().mode() & 0o077,
            0
        );
    }

    #[tokio::test]
    async fn endpoint_file_read_is_bounded_and_stale_state_is_removed_before_launch() {
        let temp = tempfile::tempdir().unwrap();
        let endpoint_path = temp.path().join(ENDPOINT_FILE);
        tokio::fs::write(&endpoint_path, vec![b'x'; MAX_ENDPOINT_FILE_BYTES * 2])
            .await
            .unwrap();

        let bytes = read_bounded_endpoint_file(&endpoint_path).await.unwrap();
        assert_eq!(bytes.len(), MAX_ENDPOINT_FILE_BYTES + 1);

        clear_stale_endpoint_file(temp.path()).await.unwrap();
        assert!(!endpoint_path.exists());
    }

    struct PendingReapProcess {
        termination_initiated: bool,
    }

    impl ForceReapProcess for PendingReapProcess {
        fn initiate_force_termination(&mut self) -> io::Result<()> {
            self.termination_initiated = true;
            Ok(())
        }

        fn wait_until(
            &mut self,
            _deadline: tokio::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<Option<()>, OwnedChromiumError>> + Send + '_>>
        {
            Box::pin(std::future::pending())
        }

        fn wait_for_reap(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), OwnedChromiumError>> + Send + '_>> {
            Box::pin(std::future::pending())
        }
    }

    struct ObservationErrorProcess {
        termination_initiated: bool,
        reaped: bool,
    }

    impl ForceReapProcess for ObservationErrorProcess {
        fn initiate_force_termination(&mut self) -> io::Result<()> {
            self.termination_initiated = true;
            Ok(())
        }

        fn wait_until(
            &mut self,
            _deadline: tokio::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<Option<()>, OwnedChromiumError>> + Send + '_>>
        {
            Box::pin(async {
                Err(OwnedChromiumError::new(
                    OwnedChromiumErrorKind::Cleanup,
                    "graceful observation failed",
                ))
            })
        }

        fn wait_for_reap(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), OwnedChromiumError>> + Send + '_>> {
            Box::pin(async move {
                self.reaped = true;
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn graceful_observation_error_forces_and_reaps_before_returning() {
        let mut process = ObservationErrorProcess {
            termination_initiated: false,
            reaped: false,
        };
        let now = tokio::time::Instant::now();

        let forced = finalize_owned_process(
            &mut process,
            now + Duration::from_secs(1),
            now + Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert!(forced);
        assert!(process.termination_initiated);
        assert!(process.reaped);
    }

    #[tokio::test(start_paused = true)]
    async fn forced_reap_timeout_is_typed_only_after_termination_is_initiated() {
        let mut process = PendingReapProcess {
            termination_initiated: false,
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);

        let error = force_terminate_and_reap(&mut process, deadline)
            .await
            .unwrap_err();

        assert!(process.termination_initiated);
        assert_eq!(error.kind, OwnedChromiumErrorKind::Cleanup);
        assert_eq!(
            error.message,
            "owned Chromium forced termination was initiated but reap exceeded its deadline"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_helper_uses_absolute_application_name() {
        let executable = test_shell_executable();
        assert!(executable.is_absolute());
        assert!(executable.ends_with("powershell.exe"));
    }

    #[tokio::test]
    async fn normal_process_tree_exit_observes_complete_descendant_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("descendant.pid");
        let mut process = ProcessTree::spawn(
            &test_shell_executable(),
            &test_cooperative_descendant_arguments(&pid_file),
            &[],
            tokio::time::Instant::now() + Duration::from_secs(5),
            temp.path(),
        )
        .unwrap();
        let descendant = wait_for_descendant_pid(&pid_file).await;

        assert_eq!(
            process
                .wait_until(tokio::time::Instant::now() + Duration::from_secs(5))
                .await
                .unwrap(),
            Some(())
        );
        assert!(
            !descendant_is_alive(descendant),
            "normal finalization returned while descendant {descendant} was still alive"
        );
    }

    #[tokio::test]
    async fn forced_process_tree_finalization_returns_only_after_descendants_are_gone() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("descendant.pid");
        let args = test_descendant_arguments(&pid_file);
        let mut process = ProcessTree::spawn(
            &test_shell_executable(),
            &args,
            &[],
            tokio::time::Instant::now() + Duration::from_secs(5),
            temp.path(),
        )
        .unwrap();
        let descendant = wait_for_descendant_pid(&pid_file).await;
        assert!(descendant_is_alive(descendant));

        force_terminate_and_reap(
            &mut process,
            tokio::time::Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();

        assert!(
            !descendant_is_alive(descendant),
            "process finalization returned while descendant {descendant} was still alive"
        );
    }

    #[tokio::test]
    async fn dropping_process_ownership_terminates_and_observes_descendants() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("dropped-descendant.pid");
        let args = test_descendant_arguments(&pid_file);
        let process = ProcessTree::spawn(
            &test_shell_executable(),
            &args,
            &[],
            tokio::time::Instant::now() + Duration::from_secs(5),
            temp.path(),
        )
        .unwrap();
        let descendant = wait_for_descendant_pid(&pid_file).await;
        assert!(descendant_is_alive(descendant));

        drop(process);

        assert!(
            !descendant_is_alive(descendant),
            "ProcessTree::drop returned while descendant {descendant} was alive"
        );
    }

    #[tokio::test]
    async fn dropping_launch_future_terminates_and_observes_owned_descendants() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("dropped-launch-descendant.pid");
        let launcher = OwnedChromiumLauncher::for_executable(
            test_shell_executable(),
            temp.path().to_path_buf(),
            test_descendant_arguments(&pid_file),
        );
        let now = tokio::time::Instant::now();
        let deadlines = OwnedChromiumDeadlines {
            work: now + Duration::from_secs(5),
            graceful: now + Duration::from_secs(6),
            force: now + Duration::from_secs(7),
            handler: now + Duration::from_secs(8),
            finalize: now + Duration::from_secs(9),
        };
        let launch_task =
            tokio::spawn(async move { launcher.launch(deadlines, std::future::pending()).await });
        let descendant = wait_for_descendant_pid(&pid_file).await;

        launch_task.abort();
        let _ = launch_task.await;

        assert!(
            !descendant_is_alive(descendant),
            "dropping launch future left descendant {descendant} alive"
        );
    }

    #[tokio::test]
    async fn already_cancelled_launch_returns_before_process_creation() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("spawned.marker");
        let launcher = OwnedChromiumLauncher::for_executable(
            test_shell_executable(),
            temp.path().to_path_buf(),
            test_marker_arguments(&marker),
        );
        let now = tokio::time::Instant::now();
        let deadlines = OwnedChromiumDeadlines {
            work: now + Duration::from_secs(1),
            graceful: now + Duration::from_secs(2),
            force: now + Duration::from_secs(3),
            handler: now + Duration::from_secs(4),
            finalize: now + Duration::from_secs(5),
        };

        let error = match launcher.launch(deadlines, std::future::ready(())).await {
            Err(error) => error,
            Ok(_) => panic!("already-cancelled launch must not create a session"),
        };

        assert_eq!(error.kind, OwnedChromiumErrorKind::Cancelled);
        assert!(!marker.exists(), "cancelled launch spawned the helper");
        assert_no_session_residue(temp.path());
    }

    #[tokio::test]
    async fn endpoint_discovery_deadline_forces_and_reaps_before_error_returns() {
        let temp = tempfile::tempdir().unwrap();
        let executable = test_sleep_executable();
        let pid_file = temp.path().join("deadline-descendant.pid");
        let launcher = OwnedChromiumLauncher::for_executable(
            executable,
            temp.path().to_path_buf(),
            test_descendant_arguments(&pid_file),
        );
        let now = tokio::time::Instant::now();
        let deadlines = OwnedChromiumDeadlines {
            work: now + Duration::from_secs(2),
            graceful: now + Duration::from_secs(3),
            force: now + Duration::from_secs(5),
            handler: now + Duration::from_secs(6),
            finalize: now + Duration::from_secs(7),
        };

        let error = match launcher.launch(deadlines, std::future::pending()).await {
            Err(error) => error,
            Ok(_) => panic!("helper must not publish a DevTools endpoint"),
        };

        // The helper never publishes DevToolsActivePort. A deadline is preserved
        // only after forced finalization observes the complete owned tree gone.
        assert_eq!(error.kind, OwnedChromiumErrorKind::Deadline);
        let descendant = wait_for_descendant_pid(&pid_file).await;
        assert!(
            !descendant_is_alive(descendant),
            "deadline returned while descendant {descendant} was still alive"
        );
        assert_no_session_residue(temp.path());
    }

    #[tokio::test]
    async fn cancellation_during_endpoint_discovery_forces_and_reaps_before_error_returns() {
        let temp = tempfile::tempdir().unwrap();
        let executable = test_sleep_executable();
        let pid_file = temp.path().join("cancel-descendant.pid");
        let launcher = OwnedChromiumLauncher::for_executable(
            executable,
            temp.path().to_path_buf(),
            test_descendant_arguments(&pid_file),
        );
        let now = tokio::time::Instant::now();
        let deadlines = OwnedChromiumDeadlines {
            work: now + Duration::from_secs(5),
            graceful: now + Duration::from_secs(6),
            force: now + Duration::from_secs(7),
            handler: now + Duration::from_secs(8),
            finalize: now + Duration::from_secs(9),
        };

        let error = match launcher
            .launch(deadlines, cancel_when_descendant_is_published(&pid_file))
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("helper must not publish a DevTools endpoint"),
        };

        assert_eq!(error.kind, OwnedChromiumErrorKind::Cancelled);
        let descendant = wait_for_descendant_pid(&pid_file).await;
        assert!(
            !descendant_is_alive(descendant),
            "cancellation returned while descendant {descendant} was still alive"
        );
        assert_no_session_residue(temp.path());
    }

    #[tokio::test]
    async fn real_managed_chromium_probe_is_environment_gated() -> Result<(), String> {
        let Ok(runtime_dir) = std::env::var("JOB_RADAR_BROWSER_RUNTIME_DIR") else {
            return Ok(());
        };
        let launcher = OwnedChromiumLauncher::from_installed_runtime(Path::new(&runtime_dir))
            .map_err(|error| error.message)?;
        let now = tokio::time::Instant::now();
        let deadlines = OwnedChromiumDeadlines {
            work: now + Duration::from_secs(15),
            graceful: now + Duration::from_secs(18),
            force: now + Duration::from_secs(20),
            handler: now + Duration::from_secs(22),
            finalize: now + Duration::from_secs(24),
        };
        let mut session = launcher
            .launch(deadlines, std::future::pending())
            .await
            .map_err(|error| error.message)?;
        let primary_result = match tokio::time::timeout_at(deadlines.work, async {
            let page = session
                .browser_mut()
                .new_page("data:text/html,%3Ch1%3Eowned%3C%2Fh1%3E")
                .await
                .map_err(|error| format!("real managed Chromium page failed: {error}"))?;
            let content = page
                .content()
                .await
                .map_err(|error| format!("real managed Chromium content failed: {error}"))?;
            if !content.contains("owned") {
                return Err("real managed Chromium content omitted probe marker".to_string());
            }
            Ok(())
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err(
                "real managed Chromium page/content probe exceeded its work deadline".to_string(),
            ),
        };
        let cleanup_result = session
            .shutdown()
            .await
            .map_err(|error| format!("real managed Chromium cleanup failed: {}", error.message));

        match cleanup_result {
            Err(cleanup) => Err(cleanup),
            Ok(_) => primary_result,
        }
    }

    async fn cancel_when_descendant_is_published(path: &Path) {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn wait_for_descendant_pid(path: &Path) -> u32 {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(contents) = tokio::fs::read_to_string(path).await {
                return contents.trim().parse().unwrap();
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "helper did not publish descendant pid"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn assert_no_session_residue(runtime_dir: &Path) {
        let temp = runtime_dir.join(".tmp");
        if temp.exists() {
            assert_eq!(std::fs::read_dir(temp).unwrap().count(), 0);
        }
    }

    #[cfg(unix)]
    fn test_shell_executable() -> PathBuf {
        PathBuf::from("/bin/sh")
    }

    #[cfg(unix)]
    fn test_sleep_executable() -> PathBuf {
        test_shell_executable()
    }

    #[cfg(unix)]
    fn test_marker_arguments(marker: &Path) -> Vec<String> {
        vec![
            "-c".to_string(),
            "touch \"$1\"; sleep 30".to_string(),
            "job-radar-helper".to_string(),
            marker.to_string_lossy().to_string(),
        ]
    }

    #[cfg(unix)]
    fn test_cooperative_descendant_arguments(pid_file: &Path) -> Vec<String> {
        vec![
            "-c".to_string(),
            "sleep 0.1 & echo $! > \"$1\"; wait".to_string(),
            "job-radar-helper".to_string(),
            pid_file.to_string_lossy().to_string(),
        ]
    }

    #[cfg(unix)]
    fn test_descendant_arguments(pid_file: &Path) -> Vec<String> {
        vec![
            "-c".to_string(),
            "sleep 30 & echo $! > \"$1\"; wait".to_string(),
            "job-radar-helper".to_string(),
            pid_file.to_string_lossy().to_string(),
        ]
    }

    #[cfg(unix)]
    fn descendant_is_alive(pid: u32) -> bool {
        let result = unsafe { libc::kill(pid as i32, 0) };
        result == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    #[cfg(windows)]
    fn test_shell_executable() -> PathBuf {
        let system_root = std::env::var_os("SystemRoot").expect("Windows tests require SystemRoot");
        PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    }

    #[cfg(windows)]
    fn test_sleep_executable() -> PathBuf {
        test_shell_executable()
    }

    #[cfg(windows)]
    fn test_marker_arguments(marker: &Path) -> Vec<String> {
        let escaped = marker.to_string_lossy().replace('\'', "''");
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!(
                "Set-Content -NoNewline -Path '{escaped}' -Value spawned; Start-Sleep -Seconds 30"
            ),
        ]
    }

    #[cfg(windows)]
    fn test_cooperative_descendant_arguments(pid_file: &Path) -> Vec<String> {
        let escaped = pid_file.to_string_lossy().replace('\'', "''");
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!("$p=Start-Process powershell.exe -ArgumentList '-NoProfile','-Command','Start-Sleep -Milliseconds 100' -PassThru; Set-Content -NoNewline -Path '{escaped}' -Value $p.Id; Wait-Process -Id $p.Id"),
        ]
    }

    #[cfg(windows)]
    fn test_descendant_arguments(pid_file: &Path) -> Vec<String> {
        let escaped = pid_file.to_string_lossy().replace('\'', "''");
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!("$p=Start-Process powershell.exe -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 30' -PassThru; Set-Content -NoNewline -Path '{escaped}' -Value $p.Id; Wait-Process -Id $p.Id"),
        ]
    }

    #[cfg(windows)]
    fn descendant_is_alive(pid: u32) -> bool {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, WAIT_TIMEOUT},
            System::Threading::{OpenProcess, WaitForSingleObject},
        };
        const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let alive = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
        unsafe { CloseHandle(handle) };
        alive
    }
}
