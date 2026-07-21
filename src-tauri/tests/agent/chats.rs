use futures_util::{stream, StreamExt};
use job_radar_lib::agent::models::{Model, ModelId, ProviderId, ReasoningLevel};
use job_radar_lib::agent::sessions::{CompactionReason, SessionCheckpoint, SessionId};
use job_radar_lib::agent::testing::{
    synthetic_assistant_message_with_replay, synthetic_model_with_limits,
    synthetic_turn_completion_with_replay, ExpectedConversationRequest, ScriptedProvider,
    ScriptedTurn, SessionTestHarness, SyntheticReplaySignature,
};
use job_radar_lib::agent::{
    AgentChat, AgentChatEvent, AgentChatState, AssistantContent, ContentKind, ConversationProvider,
    ConversationRequest, FinishReason, Message, ProviderEvent, ProviderEventStream,
    ProviderTurnCompletion, TokenUsage, UserMessage,
};
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

const SESSION: &str = "01890f47-e8b0-7cc3-98c4-dc0c0c07398f";

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

fn harness(fail: Option<SessionCheckpoint>) -> SessionTestHarness {
    let timestamps = (0..30)
        .map(|second| format!("2023-07-01T00:00:{second:02}Z"))
        .collect();
    let mut uuids = vec![Uuid::parse_str(SESSION).unwrap()];
    uuids.extend((1_u128..80).map(|value| Uuid::from_u128(value << 96)));
    let harness = SessionTestHarness::new(timestamps, uuids, true);
    if let Some(checkpoint) = fail {
        harness.fail_at([checkpoint])
    } else {
        harness
    }
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

const VALID_SUMMARY: &str = "# Goal\nKeep helping\n# Constraints & Preferences\nNone\n# Progress\n## Done\nPrior turns\n## In Progress\nCurrent task\n## Blocked\nNone\n# Key Decisions\nBe concise\n# Next Steps\nContinue\n# Critical Context\nSynthetic context";
const VALID_SPLIT_SUMMARY: &str =
    "## Original Request\nLarge request\n## Early Progress\nStarted\n## Context for Suffix\nContinue";

fn completed_with_reason(text: &str, finish_reason: FinishReason) -> Vec<ProviderEvent> {
    let mut events = completed(text);
    *events.last_mut().unwrap() = ProviderEvent::Completed(ProviderTurnCompletion::new(
        TokenUsage::default(),
        finish_reason,
    ));
    events
}

fn completed_with_usage(text: &str, total: u64) -> Vec<ProviderEvent> {
    let mut events = completed(text);
    *events.last_mut().unwrap() = ProviderEvent::Completed(ProviderTurnCompletion::new(
        TokenUsage {
            total,
            ..TokenUsage::default()
        },
        FinishReason::Completed,
    ));
    events
}

fn completed_with_typed_replay(text: &str) -> Vec<ProviderEvent> {
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
        ProviderEvent::Completed(synthetic_turn_completion_with_replay(
            TokenUsage::default(),
            FinishReason::Completed,
            Some("synthetic-response-id".into()),
            vec![Some(SyntheticReplaySignature::Text(
                "synthetic-text-signature".into(),
            ))],
        )),
    ]
}

fn run(chat: &mut AgentChat, text: &str) -> Vec<AgentChatEvent> {
    tauri::async_runtime::block_on(async {
        let mut stream = chat.send(text.to_owned()).unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    })
}

#[test]
fn completed_turn_is_durable_before_success_and_restart_resumes_exact_context() {
    let temp = TempDir::new().unwrap();
    let harness = harness(None);
    let manager = harness.manager(&root(&temp)).unwrap();
    let selected = model(
        "synthetic-model",
        vec![ReasoningLevel::Off, ReasoningLevel::Medium],
    );
    let first_provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("First"))],
                selected.id().clone(),
                ReasoningLevel::Medium,
            ),
            completed_with_typed_replay("First reply"),
        )],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        first_provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Medium,
    )
    .unwrap();

    let events = run(&mut chat, "First");
    assert!(matches!(
        events.last(),
        Some(AgentChatEvent::Completed { .. })
    ));
    assert_eq!(chat.snapshot().turns().len(), 1);
    let id = chat.snapshot().id();
    drop(chat);

    let session_file = std::fs::read_dir(root(&temp).join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .unwrap();
    let persisted = std::fs::read_to_string(session_file).unwrap();
    assert!(persisted.contains("synthetic-response-id"));
    assert!(persisted.contains("synthetic-text-signature"));
    assert!(!persisted.contains("authorization"));

    let first_assistant = synthetic_assistant_message_with_replay(
        vec![AssistantContent::Text("First reply".into())],
        selected.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
        Some("synthetic-response-id".into()),
        vec![Some(SyntheticReplaySignature::Text(
            "synthetic-text-signature".into(),
        ))],
    );
    let second_provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![
                    Message::User(UserMessage::new("First")),
                    first_assistant,
                    Message::User(UserMessage::new("Second")),
                ],
                selected.id().clone(),
                ReasoningLevel::Medium,
            ),
            completed("Second reply"),
        )],
    );
    let provider_handle = second_provider.clone();
    let mut resumed = AgentChat::open(&manager, &id, "System".into(), second_provider).unwrap();

    assert_eq!(resumed.state(), AgentChatState::Ready);
    let resumed_events = run(&mut resumed, "Second");
    assert!(
        matches!(
            resumed_events.last(),
            Some(AgentChatEvent::Completed { .. })
        ),
        "{resumed_events:?}"
    );
    assert_eq!(resumed.snapshot().turns().len(), 2);
    let requests = provider_handle.recorded_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].conversation_id(), id.as_str());
}

#[test]
fn restart_preserves_historical_attribution_across_a_model_change() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let old = model("old-model", vec![ReasoningLevel::Off]);
    let new = model("new-model", vec![ReasoningLevel::Off]);
    let first_provider = ScriptedProvider::new(
        vec![old.clone(), new.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Old turn"))],
                old.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_with_typed_replay("Old reply"),
        )],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        first_provider,
        provider_id(),
        old.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "Old turn");
    chat.select_model(provider_id(), new.id().clone()).unwrap();
    let id = chat.snapshot().id();
    drop(chat);

    let historical = synthetic_assistant_message_with_replay(
        vec![AssistantContent::Text("Old reply".into())],
        old.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
        Some("synthetic-response-id".into()),
        vec![Some(SyntheticReplaySignature::Text(
            "synthetic-text-signature".into(),
        ))],
    );
    let resumed_provider = ScriptedProvider::new(
        vec![old.clone(), new.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![
                    Message::User(UserMessage::new("Old turn")),
                    historical,
                    Message::User(UserMessage::new("New turn")),
                ],
                new.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("New reply"),
        )],
    );
    let mut resumed = AgentChat::open(&manager, &id, "System".into(), resumed_provider).unwrap();

    assert!(matches!(
        run(&mut resumed, "New turn").last(),
        Some(AgentChatEvent::Completed { .. })
    ));
}

#[test]
fn unsuccessful_and_dropped_turns_never_publish_or_enter_resume_context() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let expected = |text: &str| {
        ExpectedConversationRequest::new(
            "System",
            vec![Message::User(UserMessage::new(text))],
            selected.id().clone(),
            ReasoningLevel::Off,
        )
    };
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![
            ScriptedTurn::new(
                expected("Failed"),
                vec![
                    ProviderEvent::Started,
                    ProviderEvent::Failed(job_radar_lib::agent::AgentError {
                        category: job_radar_lib::agent::AgentErrorCategory::Transport,
                        message: "synthetic transport failure".into(),
                        retry_after: None,
                    }),
                ],
            ),
            ScriptedTurn::new(
                expected("Provider aborted"),
                vec![ProviderEvent::Started, ProviderEvent::Aborted],
            ),
            ScriptedTurn::new(expected("Malformed"), vec![ProviderEvent::Started]),
            ScriptedTurn::new(expected("Dropped"), completed("Must not commit")),
            ScriptedTurn::new(expected("Successful"), completed("Committed")),
        ],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    assert!(matches!(
        run(&mut chat, "Failed").last(),
        Some(AgentChatEvent::Failed { .. })
    ));
    assert!(matches!(
        run(&mut chat, "Provider aborted").last(),
        Some(AgentChatEvent::Aborted)
    ));
    assert!(matches!(
        run(&mut chat, "Malformed").last(),
        Some(AgentChatEvent::Failed { .. })
    ));
    drop(chat.send("Dropped".into()).unwrap());
    assert!(chat.snapshot().turns().is_empty());
    assert!(manager.list().unwrap().is_empty());

    assert!(matches!(
        run(&mut chat, "Successful").last(),
        Some(AgentChatEvent::Completed { .. })
    ));
    assert_eq!(chat.snapshot().turns().len(), 1);
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
fn caller_cancellation_wakes_a_pending_turn_and_never_publishes_a_partial_chat() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = PendingProvider {
        models: vec![selected.clone()],
    };
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    let terminal = tauri::async_runtime::block_on(async {
        let mut events = chat.send("Cancel me".into()).unwrap();
        let cancellation = events.cancellation();
        cancellation.cancel();
        events.next().await
    });

    assert!(matches!(terminal, Some(AgentChatEvent::Aborted)));
    assert!(chat.snapshot().turns().is_empty());
    assert_eq!(manager.list().unwrap().len(), 0);
}

#[test]
fn caller_cancellation_after_partial_output_wins_without_committing_the_ready_completion() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Cancel partial"))],
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("Partial then complete"),
        )],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    tauri::async_runtime::block_on(async {
        let mut events = chat.send("Cancel partial".into()).unwrap();
        assert!(matches!(events.next().await, Some(AgentChatEvent::Started)));
        assert!(matches!(
            events.next().await,
            Some(AgentChatEvent::ContentStarted { .. })
        ));
        assert!(matches!(
            events.next().await,
            Some(AgentChatEvent::ContentDelta { .. })
        ));
        events.cancellation().cancel();
        assert!(matches!(events.next().await, Some(AgentChatEvent::Aborted)));
    });
    assert!(chat.snapshot().turns().is_empty());
    assert!(manager.list().unwrap().is_empty());
}

#[test]
fn persistence_failure_is_not_success_and_reload_restores_the_last_durable_state() {
    let temp = TempDir::new().unwrap();
    let manager = harness(Some(SessionCheckpoint::TempSync))
        .manager(&root(&temp))
        .unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![
            ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "System",
                    vec![Message::User(UserMessage::new("Unsaved"))],
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("Copyable reply"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "System",
                    vec![Message::User(UserMessage::new("Retry explicitly"))],
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("Saved reply"),
            ),
        ],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    assert!(matches!(
        run(&mut chat, "Unsaved").last(),
        Some(AgentChatEvent::NotSaved { .. })
    ));
    assert_eq!(chat.state(), AgentChatState::NotSaved);
    assert!(chat.send("Blocked".into()).is_err());
    assert!(manager.list().unwrap().is_empty());

    chat.reload().unwrap();
    assert_eq!(chat.state(), AgentChatState::Ready);
    assert!(matches!(
        run(&mut chat, "Retry explicitly").last(),
        Some(AgentChatEvent::Completed { .. })
    ));
    assert_eq!(chat.snapshot().turns().len(), 1);
}

#[test]
fn failed_reasoning_change_blocks_sends_until_reload_restores_durable_settings() {
    let temp = TempDir::new().unwrap();
    let manager = harness(Some(SessionCheckpoint::AppendSync))
        .manager(&root(&temp))
        .unwrap();
    let selected = model(
        "synthetic-model",
        vec![ReasoningLevel::Off, ReasoningLevel::Medium],
    );
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Persist"))],
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("Durable reply"),
        )],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "Persist");

    assert!(chat.set_reasoning_level(ReasoningLevel::Medium).is_err());
    assert_eq!(chat.state(), AgentChatState::NotSaved);
    assert!(chat.send("Blocked".into()).is_err());

    chat.reload().unwrap();
    assert_eq!(chat.state(), AgentChatState::Ready);
    assert_eq!(chat.reasoning_level(), ReasoningLevel::Medium);
    assert_eq!(chat.snapshot().turns().len(), 1);
}

#[test]
fn manual_compaction_is_durable_reconstructs_context_and_retains_full_history() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let first_reply = job_radar_lib::agent::testing::synthetic_assistant_message(
        vec![AssistantContent::Text("One reply".into())],
        selected.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
    );
    let second_reply = job_radar_lib::agent::testing::synthetic_assistant_message(
        vec![AssistantContent::Text("Two reply".into())],
        selected.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
    );
    let wrapped = format!("The conversation history before this point was compacted into the following summary:\n\n<summary>\n{VALID_SUMMARY}\n</summary>");
    let provider = ScriptedProvider::new(vec![selected.clone()], vec![
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("One"))], selected.id().clone(), ReasoningLevel::Off), completed("One reply")),
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("One")), first_reply, Message::User(UserMessage::new("Two"))], selected.id().clone(), ReasoningLevel::Off), completed("Two reply")),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off).with_max_tokens(13_107), completed(VALID_SUMMARY)),
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new(wrapped)), Message::User(UserMessage::new("Two")), second_reply, Message::User(UserMessage::new("Three"))], selected.id().clone(), ReasoningLevel::Off), completed("Three reply")),
    ]);
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "One");
    run(&mut chat, "Two");
    let events = tauri::async_runtime::block_on(async {
        chat.compact(Some("focus".into()))
            .unwrap()
            .collect::<Vec<_>>()
            .await
    });
    assert!(
        matches!(
            events.last(),
            Some(AgentChatEvent::CompactionCompleted { .. })
        ),
        "{events:?}"
    );
    assert_eq!(chat.snapshot().turns().len(), 2);
    assert_eq!(chat.snapshot().compactions().len(), 1);
    assert_eq!(chat.snapshot().compactions()[0].summary(), VALID_SUMMARY);
    assert!(matches!(
        run(&mut chat, "Three").last(),
        Some(AgentChatEvent::Completed { .. })
    ));
    assert_eq!(chat.snapshot().turns().len(), 3);
    handle.assert_exhausted().unwrap();
}

#[test]
fn manual_compaction_rejects_a_single_complete_turn_below_keep_budget() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::any_messages(
                "System",
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("short reply"),
        )],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    run(&mut chat, "short request");

    assert!(chat.compact(None).is_err());
    assert!(chat.snapshot().compactions().is_empty());
    assert_eq!(chat.snapshot().turns().len(), 1);
}

#[test]
fn threshold_compaction_finishes_durably_before_completed_is_observable() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let first_reply = job_radar_lib::agent::testing::synthetic_assistant_message(
        vec![AssistantContent::Text("A1".into())],
        selected.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
    );
    let provider = ScriptedProvider::new(vec![selected.clone()], vec![
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("U1"))], selected.id().clone(), ReasoningLevel::Off), completed("A1")),
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("U1")), first_reply, Message::User(UserMessage::new("U2"))], selected.id().clone(), ReasoningLevel::Off), completed_with_usage("A2", 120_000)),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off), completed(VALID_SUMMARY)),
    ]);
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");
    tauri::async_runtime::block_on(async {
        let mut stream = chat.send("U2".into()).unwrap();
        while let Some(event) = stream.next().await {
            if matches!(event, AgentChatEvent::Completed { .. }) {
                break;
            }
        }
    });
    assert_eq!(chat.snapshot().turns().len(), 2);
    assert_eq!(chat.snapshot().compactions().len(), 1);
    assert_eq!(chat.snapshot().compactions()[0].reason(), Some("threshold"));
}

#[test]
fn threshold_compaction_cancellation_preserves_durable_turn_then_completes() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(vec![selected.clone()], vec![
        ScriptedTurn::new(
            ExpectedConversationRequest::any_messages(
                "System",
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("A1"),
        ),
        ScriptedTurn::new(
            ExpectedConversationRequest::any_messages(
                "System",
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_with_usage("A2", 120_000),
        ),
        ScriptedTurn::new(
            ExpectedConversationRequest::any_messages(
                "You summarize prior conversation context. Do not answer or continue the conversation.",
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed(VALID_SUMMARY),
        ),
    ]);
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");

    let events = tauri::async_runtime::block_on(async {
        let mut stream = chat.send("U2".into()).unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            let compaction_started = matches!(
                event,
                AgentChatEvent::CompactionStarted {
                    reason: CompactionReason::Threshold
                }
            );
            events.push(event);
            if compaction_started {
                stream.cancellation().cancel();
                events.extend(stream.by_ref().collect::<Vec<_>>().await);
                break;
            }
        }
        events
    });

    let cancellation = events
        .iter()
        .position(|event| {
            matches!(
                event,
                AgentChatEvent::CompactionCancelled {
                    reason: CompactionReason::Threshold
                }
            )
        })
        .expect("threshold cancellation feedback");
    let completed = events
        .iter()
        .position(|event| matches!(event, AgentChatEvent::Completed { .. }))
        .expect("durable turn completion");
    assert!(cancellation < completed, "{events:?}");
    assert!(!events
        .iter()
        .any(|event| matches!(event, AgentChatEvent::Aborted)));
    assert_eq!(chat.snapshot().turns().len(), 2);
    assert!(chat.snapshot().compactions().is_empty());
}

#[test]
fn overflow_compacts_and_retries_exactly_once_without_persisting_failed_attempt() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let first_reply = job_radar_lib::agent::testing::synthetic_assistant_message(
        vec![AssistantContent::Text("A1".into())],
        selected.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
    );
    let provider = ScriptedProvider::new(vec![selected.clone()], vec![
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("U1"))], selected.id().clone(), ReasoningLevel::Off), completed("A1")),
        ScriptedTurn::new(ExpectedConversationRequest::new("System", vec![Message::User(UserMessage::new("U1")), first_reply, Message::User(UserMessage::new("U2"))], selected.id().clone(), ReasoningLevel::Off), completed("A2")),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), vec![ProviderEvent::Started, ProviderEvent::Failed(job_radar_lib::agent::AgentError { category: job_radar_lib::agent::AgentErrorCategory::ContextOverflow, message: "overflow".into(), retry_after: None })]),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off), completed(VALID_SUMMARY)),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), completed("recovered")),
    ]);
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");
    run(&mut chat, "U2");
    let events = run(&mut chat, "U3");
    assert!(
        matches!(events.last(), Some(AgentChatEvent::Completed { .. })),
        "{events:?}"
    );
    assert_eq!(chat.snapshot().turns().len(), 3);
    assert_eq!(chat.snapshot().compactions().len(), 1);
    assert_eq!(chat.snapshot().compactions()[0].reason(), Some("overflow"));
    handle.assert_exhausted().unwrap();
}

#[test]
fn iterative_compaction_uses_previous_summary_and_only_the_new_raw_span() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "System",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("old assistant"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "System",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("new assistant"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "You summarize prior conversation context. Do not answer or continue the conversation.",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed(VALID_SUMMARY),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "System",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("latest assistant"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "You summarize prior conversation context. Do not answer or continue the conversation.",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed(VALID_SUMMARY),
            ),
        ],
    );
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "old user");
    run(&mut chat, "new user");
    tauri::async_runtime::block_on(chat.compact(None).unwrap().collect::<Vec<_>>());
    run(&mut chat, "latest user");
    tauri::async_runtime::block_on(chat.compact(None).unwrap().collect::<Vec<_>>());

    let requests = handle.recorded_requests();
    let prompt = match &requests[4].messages()[0] {
        Message::User(message) => message.text(),
        Message::Assistant(_) => panic!("summary request must be a user prompt"),
    };
    assert!(prompt.contains(&format!(
        "<previous-summary>\n{VALID_SUMMARY}\n</previous-summary>"
    )));
    assert!(prompt.contains("User:\nnew user"));
    assert!(prompt.contains("Assistant:\nnew assistant"));
    assert!(!prompt.contains("old user"));
    assert!(!prompt.contains("old assistant"));
    assert!(!prompt.contains("conversation history before this point was compacted"));
    assert_eq!(chat.snapshot().turns().len(), 3);
    assert_eq!(chat.snapshot().compactions().len(), 2);
}

#[test]
fn split_turn_uses_8192_cap_and_merges_valid_prefix_summary() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let large = "x".repeat(80_004);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "System",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                ),
                completed("suffix"),
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
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages(
                    "You summarize prior conversation context. Do not answer or continue the conversation.",
                    selected.id().clone(),
                    ReasoningLevel::Off,
                )
                .with_max_tokens(8_192),
                completed(VALID_SPLIT_SUMMARY),
            ),
        ],
    );
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, &large);
    let events = tauri::async_runtime::block_on(chat.compact(None).unwrap().collect::<Vec<_>>());
    assert!(matches!(
        events.last(),
        Some(AgentChatEvent::CompactionCompleted { .. })
    ));
    let summary = chat.snapshot().compactions()[0].summary();
    assert!(summary.starts_with(VALID_SUMMARY));
    assert!(summary.contains("**Turn Context (split turn):**"));
    assert!(summary.ends_with(VALID_SPLIT_SUMMARY));
    let requests = handle.recorded_requests();
    let split_prompt = match &requests[2].messages()[0] {
        Message::User(message) => message.text(),
        _ => unreachable!(),
    };
    assert!(split_prompt.contains(&large));
}

#[test]
fn cancelled_malformed_and_truncated_compactions_create_no_entry() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off),
                completed("A1"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off),
                completed("A2"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off),
                completed(VALID_SUMMARY),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off),
                completed("# Goal\nmissing sections"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off),
                completed_with_reason(VALID_SUMMARY, FinishReason::LengthLimit),
            ),
        ],
    );
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");
    run(&mut chat, "U2");
    tauri::async_runtime::block_on(async {
        let mut stream = chat.compact(None).unwrap();
        assert!(matches!(
            stream.next().await,
            Some(AgentChatEvent::CompactionStarted { .. })
        ));
        stream.cancellation().cancel();
        assert!(matches!(
            stream.next().await,
            Some(AgentChatEvent::CompactionCancelled {
                reason: CompactionReason::Manual
            })
        ));
    });
    assert!(chat.snapshot().compactions().is_empty());
    let malformed = tauri::async_runtime::block_on(chat.compact(None).unwrap().collect::<Vec<_>>());
    assert!(matches!(
        malformed.last(),
        Some(AgentChatEvent::CompactionFailed { .. })
    ));
    assert!(chat.snapshot().compactions().is_empty());
    let truncated = tauri::async_runtime::block_on(chat.compact(None).unwrap().collect::<Vec<_>>());
    assert!(matches!(
        truncated.last(),
        Some(AgentChatEvent::CompactionFailed { .. })
    ));
    assert!(chat.snapshot().compactions().is_empty());
    assert_eq!(chat.snapshot().turns().len(), 2);
}

#[test]
fn small_model_blocks_at_safe_threshold_without_starting_compaction() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = synthetic_model_with_limits(
        model("small-model", vec![ReasoningLevel::Off]),
        36_384,
        8_192,
    );
    let provider = ScriptedProvider::new(
        vec![selected.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::any_messages(
                "System",
                selected.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_with_usage("A1", 20_001),
        )],
    );
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");
    assert!(chat.send("draft".into()).is_err());
    assert_eq!(handle.recorded_requests().len(), 1);
    assert_eq!(chat.snapshot().turns().len(), 1);
    assert!(chat.snapshot().compactions().is_empty());
}

#[test]
fn second_overflow_stops_without_loop_or_partial_turn() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let selected = model("synthetic-model", vec![ReasoningLevel::Off]);
    let overflow = || {
        vec![
            ProviderEvent::Started,
            ProviderEvent::Failed(job_radar_lib::agent::AgentError {
                category: job_radar_lib::agent::AgentErrorCategory::ContextOverflow,
                message: "overflow".into(),
                retry_after: None,
            }),
        ]
    };
    let provider = ScriptedProvider::new(vec![selected.clone()], vec![
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), completed("A1")),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), completed("A2")),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), overflow()),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("You summarize prior conversation context. Do not answer or continue the conversation.", selected.id().clone(), ReasoningLevel::Off), completed(VALID_SUMMARY)),
        ScriptedTurn::new(ExpectedConversationRequest::any_messages("System", selected.id().clone(), ReasoningLevel::Off), overflow()),
    ]);
    let handle = provider.clone();
    let mut chat = AgentChat::create(
        &manager,
        "System".into(),
        provider,
        provider_id(),
        selected.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut chat, "U1");
    run(&mut chat, "U2");
    let events = run(&mut chat, "U3");
    assert!(matches!(events.last(), Some(AgentChatEvent::Failed { .. })));
    assert_eq!(chat.snapshot().turns().len(), 2);
    assert_eq!(chat.snapshot().compactions().len(), 1);
    assert_eq!(handle.recorded_requests().len(), 5);
    handle.assert_exhausted().unwrap();
}

#[test]
fn unavailable_recorded_model_is_readable_until_explicit_model_remediation() {
    let temp = TempDir::new().unwrap();
    let manager = harness(None).manager(&root(&temp)).unwrap();
    let old = model("old-model", vec![ReasoningLevel::Off]);
    let first_provider = ScriptedProvider::new(
        vec![old.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Persist"))],
                old.id().clone(),
                ReasoningLevel::Off,
            ),
            completed("History"),
        )],
    );
    let mut first = AgentChat::create(
        &manager,
        "System".into(),
        first_provider,
        provider_id(),
        old.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();
    run(&mut first, "Persist");
    let id: SessionId = first.snapshot().id();
    drop(first);

    let replacement = model(
        "replacement-model",
        vec![ReasoningLevel::Low, ReasoningLevel::High],
    );
    let replacement_provider = ScriptedProvider::new(vec![replacement.clone()], Vec::new());
    let mut reopened =
        AgentChat::open(&manager, &id, "System".into(), replacement_provider).unwrap();

    assert_eq!(reopened.state(), AgentChatState::ModelUnavailable);
    assert_eq!(reopened.snapshot().turns().len(), 1);
    assert!(reopened.send("Must be blocked".into()).is_err());
    let effective = reopened
        .select_model(provider_id(), replacement.id().clone())
        .unwrap();
    assert_eq!(effective, ReasoningLevel::Low);
    assert_eq!(reopened.state(), AgentChatState::Ready);
    assert_eq!(reopened.snapshot().selected_model(), Some(replacement.id()));
    assert_eq!(reopened.reasoning_level(), ReasoningLevel::Low);
    assert_eq!(reopened.snapshot().reasoning_level(), ReasoningLevel::Off);
}
