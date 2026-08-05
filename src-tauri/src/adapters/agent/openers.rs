use agent::{
    DataFolderOpener, InteractionError, InteractionFuture, LoginAttemptId, LoginInteraction,
    LoginMethod, LoginProgress, OpenError, ProviderId,
};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

pub(crate) const LOGIN_PROGRESS_EVENT: &str = "agent-subscription-login-progress";

pub(crate) struct TauriAgentOpener {
    app: AppHandle,
}

impl TauriAgentOpener {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl LoginInteraction for TauriAgentOpener {
    fn select_method(
        &mut self,
        _attempt: &LoginAttemptId,
        _provider: &ProviderId,
    ) -> InteractionFuture<'_, LoginMethod> {
        Box::pin(async { Ok(LoginMethod::Browser) })
    }

    fn open_url(&mut self, _attempt: &LoginAttemptId, url: &str) -> Result<(), InteractionError> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|_| InteractionError)
    }

    fn display_device_code(
        &mut self,
        _attempt: &LoginAttemptId,
        _verification_uri: &str,
        _user_code: &str,
    ) -> Result<(), InteractionError> {
        Err(InteractionError)
    }

    fn report(&mut self, progress: LoginProgress) {
        let _ = self.app.emit(LOGIN_PROGRESS_EVENT, progress);
    }
}

impl DataFolderOpener for TauriAgentOpener {
    fn open(&self, path: &Path) -> Result<(), OpenError> {
        self.app
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|_| OpenError)
    }
}
