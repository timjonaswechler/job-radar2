use agent::{ChatEvent, ChatEventListener};
use tauri::{AppHandle, Emitter};

pub(crate) const AGENT_CHAT_EVENT: &str = "agent-chat-event";

pub(crate) struct TauriAgentChatEventListener {
    pub(crate) app: AppHandle,
}

impl ChatEventListener for TauriAgentChatEventListener {
    fn emit(&self, event: ChatEvent) {
        let _ = self.app.emit(AGENT_CHAT_EVENT, event);
    }
}
