use futures_util::stream;
use job_radar_lib::agent::chat_application::{
    AgentChatApplication, AgentChatApplicationEvent, AgentChatApplicationEventKind,
    AgentChatCreateInput, AgentChatEventListener, AgentChatStatus, ApplicationReasoningLevel,
};
use job_radar_lib::agent::models::{Model, ModelId, ProviderId, ReasoningLevel};
use job_radar_lib::agent::testing::{
    ExpectedConversationRequest, ScriptedProvider, ScriptedTurn, SessionTestHarness,
};
use job_radar_lib::agent::{
    ContentKind, ConversationProvider, ConversationRequest, FinishReason, Message, ProviderEvent,
    ProviderEventStream, ProviderTurnCompletion, TokenUsage, UserMessage,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use uuid::Uuid;

const CHAT_ID: &str = "01890f47-e8b0-7cc3-98c4-dc0c0c07398f";
const VALID_SUMMARY: &str = "# Goal\nKeep helping\n# Constraints & Preferences\nNone\n# Progress\n## Done\nPrior turns\n## In Progress\nCurrent task\n## Blocked\nNone\n# Key Decisions\nBe concise\n# Next Steps\nContinue\n# Critical Context\nCOMPACTION-SUMMARY-CANARY";

fn provider_id() -> ProviderId {
    ProviderId::new("synthetic-provider").unwrap()
}

fn model(id: &str, levels: Vec<ReasoningLevel>) -> Model {
    Model::new(
        ModelId::new(id).unwrap(),
        format!("Synthetic {id}"),
        provider_id(),
        levels,
    )
    .unwrap()
}

fn harness() -> SessionTestHarness {
    let timestamps = (0..30)
        .map(|second| format!("2023-07-01T00:00:{second:02}Z"))
        .collect();
    let mut uuids = vec![Uuid::parse_str(CHAT_ID).unwrap()];
    uuids.extend((1_u128..80).map(|value| Uuid::from_u128(value << 96)));
    SessionTestHarness::new(timestamps, uuids, true)
}

fn root(temp: &TempDir) -> PathBuf {
    temp.path().canonicalize().unwrap().join("agents")
}

fn completed(text: &str) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::Started,
        ProviderEvent::ContentStarted {
            index: 0,
            kind: ContentKind::Text,
        },
        ProviderEvent::ContentDelta {
            index: 0,
            delta: text.to_owned(),
        },
        ProviderEvent::ContentFinished { index: 0 },
        ProviderEvent::Completed(ProviderTurnCompletion::new(
            TokenUsage::default(),
            FinishReason::Completed,
        )),
    ]
}

struct ChannelListener(mpsc::UnboundedSender<AgentChatApplicationEvent>);

impl AgentChatEventListener for ChannelListener {
    fn emit(&self, event: AgentChatApplicationEvent) {
        let _ = self.0.send(event);
    }
}

fn input(model: &Model) -> AgentChatCreateInput {
    AgentChatCreateInput {
        system_prompt: "SYSTEM-PROMPT-CANARY".into(),
        provider_id: provider_id().as_str().into(),
        model_id: model.id().as_str().into(),
        reasoning_level: ApplicationReasoningLevel::High,
    }
}

#[test]
fn application_service_streams_visible_content_then_projects_only_durable_chat_state() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new().unwrap();
        let selected = model(
            "synthetic-model",
            vec![ReasoningLevel::Off, ReasoningLevel::Medium],
        );
        let provider = ScriptedProvider::new(
            vec![selected.clone()],
            vec![ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "SYSTEM-PROMPT-CANARY",
                    vec![Message::User(UserMessage::new("Visible user"))],
                    selected.id().clone(),
                    ReasoningLevel::Medium,
                ),
                completed("Visible reply"),
            )],
        );
        let manager = harness().manager(&root(&temp)).unwrap();
        let application = Arc::new(AgentChatApplication::new(manager, provider));
        let draft = application.create(input(&selected)).unwrap();
        assert_eq!(draft.status, AgentChatStatus::Ready);
        assert_eq!(draft.selected_model_id.as_deref(), Some("synthetic-model"));
        assert_eq!(draft.reasoning_level, ApplicationReasoningLevel::Medium);

        let (sender, mut receiver) = mpsc::unbounded_channel();
        application
            .send(
                draft.id.clone(),
                "Visible user".into(),
                Arc::new(ChannelListener(sender)),
            )
            .unwrap();

        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            let terminal = matches!(event.event, AgentChatApplicationEventKind::Completed { .. });
            events.push(event);
            if terminal {
                break;
            }
        }
        assert!(matches!(
            events.iter().map(|event| &event.event).collect::<Vec<_>>().as_slice(),
            [
                AgentChatApplicationEventKind::Started,
                AgentChatApplicationEventKind::ContentStarted { .. },
                AgentChatApplicationEventKind::ContentDelta { delta, .. },
                AgentChatApplicationEventKind::ContentFinished { .. },
                AgentChatApplicationEventKind::Completed { .. },
            ] if delta == "Visible reply"
        ));
        assert!(events
            .windows(2)
            .all(|pair| pair[1].sequence > pair[0].sequence));

        let completed = match &events.last().unwrap().event {
            AgentChatApplicationEventKind::Completed { chat } => chat,
            _ => unreachable!(),
        };
        assert_eq!(completed.history.len(), 1);
        assert_eq!(completed.status, AgentChatStatus::Ready);
        application
            .set_reasoning_level(&draft.id, ApplicationReasoningLevel::Off)
            .await
            .expect("terminal event must make settings immediately available");

        let serialized = serde_json::to_string(&events).unwrap();
        let debugged = format!("{events:?}");
        for forbidden in ["SYSTEM-PROMPT-CANARY", "response_id", "first_kept_entry_id"] {
            assert!(
                !serialized.contains(forbidden),
                "serialized leaked {forbidden}"
            );
            assert!(!debugged.contains(forbidden), "Debug leaked {forbidden}");
        }
    });
}

#[test]
fn identified_resume_preserves_unavailable_model_without_fallback() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new().unwrap();
        let deterministic = harness();
        let old = model("old-model", vec![ReasoningLevel::Off]);
        let first_provider = ScriptedProvider::new(
            vec![old.clone()],
            vec![ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "SYSTEM-PROMPT-CANARY",
                    old.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("Durable reply"),
            )],
        );
        let first = Arc::new(AgentChatApplication::new(
            deterministic.manager(&root(&temp)).unwrap(),
            first_provider,
        ));
        let draft = first.create(input(&old)).unwrap();
        let (sender, mut receiver) = mpsc::unbounded_channel();
        first
            .send(
                draft.id.clone(),
                "Durable user".into(),
                Arc::new(ChannelListener(sender)),
            )
            .unwrap();
        while !matches!(
            receiver.recv().await.unwrap().event,
            AgentChatApplicationEventKind::Completed { .. }
        ) {}
        drop(first);

        let replacement = model(
            "replacement",
            vec![ReasoningLevel::Off, ReasoningLevel::Low],
        );
        let resumed = AgentChatApplication::new(
            deterministic.manager(&root(&temp)).unwrap(),
            ScriptedProvider::new(vec![replacement.clone()], vec![]),
        );
        let opened = resumed
            .open(job_radar_lib::agent::chat_application::AgentChatOpenInput {
                id: draft.id.clone(),
                system_prompt: "NEW-SYSTEM-PROMPT-CANARY".into(),
            })
            .await
            .unwrap();
        assert_eq!(opened.status, AgentChatStatus::ModelUnavailable);
        assert_eq!(opened.history.len(), 1);

        let remediated = resumed
            .select_model(
                &draft.id,
                provider_id().as_str().into(),
                replacement.id().as_str().into(),
            )
            .await
            .unwrap();
        assert_eq!(remediated.status, AgentChatStatus::Ready);
        let adjusted = resumed
            .set_reasoning_level(&draft.id, ApplicationReasoningLevel::High)
            .await
            .unwrap();
        assert_eq!(adjusted.reasoning_level, ApplicationReasoningLevel::Low);
    });
}

#[test]
fn manual_compaction_is_streamed_and_snapshot_history_exposes_no_summary_or_storage_ids() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new().unwrap();
        let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
        let provider = ScriptedProvider::new(
            vec![selected.clone()],
            vec![
                ScriptedTurn::new(
                    ExpectedConversationRequest::any_messages(
                        "SYSTEM-PROMPT-CANARY",
                        selected.id().clone(),
                        ReasoningLevel::Off,
                    ),
                    completed("One reply"),
                ),
                ScriptedTurn::new(
                    ExpectedConversationRequest::any_messages(
                        "SYSTEM-PROMPT-CANARY",
                        selected.id().clone(),
                        ReasoningLevel::Off,
                    ),
                    completed("Two reply"),
                ),
                ScriptedTurn::new(
                    ExpectedConversationRequest::any_messages(
                        "You summarize prior conversation context. Do not answer or continue the conversation.",
                        selected.id().clone(),
                        ReasoningLevel::Off,
                    )
                    .with_max_tokens(13_107),
                    completed(VALID_SUMMARY),
                ),
            ],
        );
        let application = Arc::new(AgentChatApplication::new(
            harness().manager(&root(&temp)).unwrap(),
            provider,
        ));
        let draft = application.create(input(&selected)).unwrap();

        for text in ["One", "Two"] {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            application
                .send(
                    draft.id.clone(),
                    text.into(),
                    Arc::new(ChannelListener(sender)),
                )
                .unwrap();
            while !matches!(
                receiver.recv().await.unwrap().event,
                AgentChatApplicationEventKind::Completed { .. }
            ) {}
        }

        let (sender, mut receiver) = mpsc::unbounded_channel();
        application
            .compact(
                draft.id.clone(),
                Some("focus".into()),
                Arc::new(ChannelListener(sender)),
            )
            .unwrap();
        let mut events = Vec::new();
        loop {
            let event = receiver.recv().await.unwrap();
            let done = matches!(
                event.event,
                AgentChatApplicationEventKind::CompactionCompleted { .. }
                    | AgentChatApplicationEventKind::CompactionFailed { .. }
            );
            events.push(event);
            if done {
                break;
            }
        }
        assert!(matches!(
            events.last().map(|event| &event.event),
            Some(AgentChatApplicationEventKind::CompactionCompleted { chat, .. })
                if chat.history.len() == 3
        ));
        let serialized = serde_json::to_string(&events).unwrap();
        assert!(!serialized.contains("COMPACTION-SUMMARY-CANARY"));
        assert!(!serialized.contains("firstKeptEntryId"));
    });
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
        Box::pin(stream::pending())
    }
}

#[test]
fn immediate_stop_wins_the_start_race_and_only_one_operation_can_run_per_chat() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new().unwrap();
        let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
        let manager = harness().manager(&root(&temp)).unwrap();
        let application = Arc::new(AgentChatApplication::new(
            manager,
            PendingProvider {
                models: vec![selected.clone()],
            },
        ));
        let draft = application.create(input(&selected)).unwrap();
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let listener = Arc::new(ChannelListener(sender));

        application
            .send(draft.id.clone(), "cancel".into(), listener.clone())
            .unwrap();
        let busy = application
            .send(draft.id.clone(), "second".into(), listener)
            .unwrap_err();
        assert_eq!(busy.code, "chat_busy");
        assert!(application.stop(&draft.id));

        let terminal = receiver.recv().await.unwrap();
        assert!(matches!(
            terminal.event,
            AgentChatApplicationEventKind::Aborted
        ));
        let snapshot = application.snapshot(&draft.id).await.unwrap();
        assert!(snapshot.history.is_empty());
        assert_eq!(snapshot.status, AgentChatStatus::Ready);
    });
}
