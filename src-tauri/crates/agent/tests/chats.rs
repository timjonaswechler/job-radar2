use agent::Model;
use agent::{
    ChatCreateInput, ChatEvent, ChatEventKind, ChatEventListener, ChatOpenInput,
    ChatReasoningLevel, ChatStatus, Chats, ContentKind, ConversationProvider, ConversationRequest,
    FinishReason, ModelId, ProviderEvent, ProviderEventStream, ProviderId, ProviderTurnCompletion,
    TokenUsage,
};
use futures_util::{stream, StreamExt};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};

struct ScriptedProvider {
    model: Model,
    response: String,
}

impl ConversationProvider for ScriptedProvider {
    fn models(&self) -> &[Model] {
        std::slice::from_ref(&self.model)
    }

    fn stream(&self, _request: ConversationRequest) -> ProviderEventStream {
        Box::pin(stream::iter([
            ProviderEvent::Started,
            ProviderEvent::ContentStarted {
                index: 0,
                kind: ContentKind::Text,
            },
            ProviderEvent::ContentDelta {
                index: 0,
                delta: self.response.clone(),
            },
            ProviderEvent::ContentFinished { index: 0 },
            ProviderEvent::Completed(ProviderTurnCompletion::new(
                TokenUsage::default(),
                FinishReason::Completed,
            )),
        ]))
    }
}

#[derive(Clone)]
struct PendingProvider {
    models: Vec<Model>,
}

impl ConversationProvider for PendingProvider {
    fn models(&self) -> &[Model] {
        &self.models
    }

    fn stream(&self, _request: ConversationRequest) -> ProviderEventStream {
        Box::pin(stream::iter([ProviderEvent::Started]).chain(stream::pending::<ProviderEvent>()))
    }
}

struct ChannelListener(mpsc::UnboundedSender<ChatEvent>);

impl ChatEventListener for ChannelListener {
    fn emit(&self, event: ChatEvent) {
        let _ = self.0.send(event);
    }
}

struct ReentrantListener {
    chats: Arc<Chats>,
    result: mpsc::UnboundedSender<bool>,
}

impl ChatEventListener for ReentrantListener {
    fn emit(&self, event: ChatEvent) {
        if matches!(event.event, ChatEventKind::Completed { .. }) {
            let _ = self.result.send(self.chats.stop_current(&event.chat_id));
        }
    }
}

struct ReentrantReloadListener {
    chats: Arc<Chats>,
    result: mpsc::UnboundedSender<bool>,
}

impl ChatEventListener for ReentrantReloadListener {
    fn emit(&self, event: ChatEvent) {
        if matches!(event.event, ChatEventKind::Completed { .. }) {
            let chats = Arc::clone(&self.chats);
            let chat_id = event.chat_id;
            let result = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(chats.reload(&chat_id))
                    .is_ok()
            })
            .join()
            .unwrap_or(false);
            let _ = self.result.send(result);
        }
    }
}

struct PanicListener;

impl ChatEventListener for PanicListener {
    fn emit(&self, _event: ChatEvent) {
        panic!("synthetic listener panic");
    }
}

fn model() -> Model {
    Model::new(
        ModelId::new("synthetic-model").unwrap(),
        "Synthetic model",
        ProviderId::new("synthetic-provider").unwrap(),
        vec![agent::ReasoningLevel::Off],
    )
    .unwrap()
}

fn input() -> ChatCreateInput {
    ChatCreateInput {
        system_prompt: "synthetic system prompt".into(),
        provider_id: "synthetic-provider".into(),
        model_id: "synthetic-model".into(),
        reasoning_level: ChatReasoningLevel::Off,
    }
}

#[test]
fn public_model_debug_omits_transport_metadata() {
    assert!(!format!("{:?}", model()).contains("https://api.openai.com"));
}

#[tokio::test]
async fn running_snapshot_does_not_wait_for_the_provider_stream() {
    let temp = TempDir::new().unwrap();
    let chats = Arc::new(
        Chats::new(
            temp.path().join("agents"),
            PendingProvider {
                models: vec![model()],
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();

    let operation_id = chats
        .send(
            created.id.clone(),
            "pending turn".into(),
            Arc::new(ChannelListener(sender.clone())),
        )
        .unwrap();

    let snapshot = timeout(Duration::from_millis(100), chats.snapshot(&created.id))
        .await
        .expect("snapshot must not wait for a blocked provider")
        .unwrap();
    assert_eq!(snapshot.id, created.id);
    assert_eq!(snapshot.status, agent::ChatStatus::Running);
    assert_eq!(snapshot.active_operation_id, Some(operation_id));

    assert!(chats.stop(&created.id, operation_id));
    let terminal = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("stop must finish the pending operation")
        .unwrap();
    assert!(matches!(terminal.event, agent::ChatEventKind::Aborted));

    let second_operation = chats
        .send(
            created.id.clone(),
            "second pending turn".into(),
            Arc::new(ChannelListener(sender)),
        )
        .unwrap();
    assert_ne!(operation_id.as_u64(), second_operation.as_u64());
    assert!(!chats.stop(&created.id, operation_id));
    assert_eq!(
        chats.snapshot(&created.id).await.unwrap().status,
        agent::ChatStatus::Running
    );
    assert!(chats.stop(&created.id, second_operation));
    let terminal = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("the current generation must be stoppable")
        .unwrap();
    assert!(matches!(terminal.event, agent::ChatEventKind::Aborted));
}

#[tokio::test]
async fn terminal_event_is_reentrant_after_operation_authority_is_released() {
    let temp = TempDir::new().unwrap();
    let chats = Arc::new(
        Chats::new(
            temp.path().join("agents"),
            ScriptedProvider {
                model: model(),
                response: "saved response".into(),
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();

    chats
        .send(
            created.id.clone(),
            "saved request".into(),
            Arc::new(ReentrantListener {
                chats: Arc::clone(&chats),
                result: sender,
            }),
        )
        .unwrap();

    assert!(!timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap());
    assert_eq!(
        chats.snapshot(&created.id).await.unwrap().status,
        ChatStatus::Ready
    );
}

#[tokio::test]
async fn terminal_listener_can_reload_chat_without_waiting_for_chat_lock() {
    let temp = TempDir::new().unwrap();
    let chats = Arc::new(
        Chats::new(
            temp.path().join("agents"),
            ScriptedProvider {
                model: model(),
                response: "reloadable response".into(),
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();

    chats
        .send(
            created.id.clone(),
            "reload request".into(),
            Arc::new(ReentrantReloadListener {
                chats: Arc::clone(&chats),
                result: sender,
            }),
        )
        .unwrap();

    assert!(timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("re-entrant reload must complete")
        .expect("re-entrant listener result"));
    assert_eq!(
        chats.snapshot(&created.id).await.unwrap().status,
        ChatStatus::Ready
    );
}

#[tokio::test]
async fn every_event_carries_the_owned_operation_identity() {
    let temp = TempDir::new().unwrap();
    let chats = Arc::new(
        Chats::new(
            temp.path().join("agents"),
            ScriptedProvider {
                model: model(),
                response: "identity response".into(),
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let operation_id = chats
        .send(
            created.id.clone(),
            "identity request".into(),
            Arc::new(ChannelListener(sender)),
        )
        .unwrap();

    let mut events = Vec::new();
    while events.len() < 5 {
        events.push(
            timeout(Duration::from_secs(1), receiver.recv())
                .await
                .expect("operation event")
                .expect("operation event channel"),
        );
    }
    assert!(events
        .iter()
        .all(|event| event.operation_id == operation_id));
    assert_eq!(events.iter().filter(|event| event.terminal).count(), 1);
    assert!(events
        .windows(2)
        .all(|events| events[0].sequence < events[1].sequence));
    let completed = events
        .iter()
        .find_map(|event| match &event.event {
            ChatEventKind::Completed { chat } => Some(chat),
            _ => None,
        })
        .expect("completed event");
    assert_eq!(completed.active_operation_id, None);
}

#[tokio::test]
async fn completed_chat_is_durable_before_terminal_event_and_reopens_through_chats() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("agents");
    let chats = Arc::new(
        Chats::new(
            &root,
            ScriptedProvider {
                model: model(),
                response: "durable response".into(),
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    chats
        .send(
            created.id.clone(),
            "durable request".into(),
            Arc::new(ChannelListener(sender)),
        )
        .unwrap();

    let completed = loop {
        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        if let ChatEventKind::Completed { chat } = event.event {
            break chat;
        }
    };
    assert_eq!(completed.status, ChatStatus::Ready);
    assert_eq!(completed.history.len(), 1);
    drop(chats);

    let reopened = Chats::new(
        &root,
        ScriptedProvider {
            model: model(),
            response: "unused".into(),
        },
    )
    .unwrap();
    let opened = reopened
        .open(ChatOpenInput {
            id: created.id,
            system_prompt: "same caller-owned prompt".into(),
        })
        .await
        .unwrap();
    assert_eq!(opened.status, ChatStatus::Ready);
    assert_eq!(opened.history.len(), 1);
}

#[tokio::test]
async fn listener_panic_cannot_strand_a_chat_busy() {
    let temp = TempDir::new().unwrap();
    let chats = Arc::new(
        Chats::new(
            temp.path().join("agents"),
            PendingProvider {
                models: vec![model()],
            },
        )
        .unwrap(),
    );
    let created = chats.create(input()).unwrap();
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    chats
        .send(
            created.id.clone(),
            "panic in listener".into(),
            Arc::new(PanicListener),
        )
        .unwrap();

    timeout(Duration::from_secs(1), async {
        loop {
            if chats.snapshot(&created.id).await.unwrap().status == ChatStatus::Ready {
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("listener panic must release the operation reservation");
    std::panic::set_hook(panic_hook);

    assert!(!chats.stop_current(&created.id));
}
