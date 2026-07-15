use futures_util::StreamExt;
use job_radar_lib::agent::models::{Model, ModelId, ProviderId, ReasoningLevel};
use job_radar_lib::agent::testing::{
    synthetic_assistant_message, ExpectedConversationRequest, ScriptedProvider, ScriptedTurn,
};
use job_radar_lib::agent::{
    AgentConversation, AgentErrorCategory, AssistantContent, ContentKind, ConversationEvent,
    FinishReason, Message, ProviderEvent, ProviderTurnCompletion, TokenUsage, UserMessage,
};

fn synthetic_model(id: &str, levels: Vec<ReasoningLevel>) -> Model {
    Model::new(
        ModelId::new(id).unwrap(),
        format!("Synthetic {id}"),
        ProviderId::new("synthetic-provider").unwrap(),
        levels,
    )
    .unwrap()
}

fn run_turn(conversation: &mut AgentConversation, text: &str) -> Vec<ConversationEvent> {
    tauri::async_runtime::block_on(async {
        let mut stream = conversation.send(text.to_owned()).unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    })
}

fn completed_text(text: &str) -> Vec<ProviderEvent> {
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

#[test]
fn completed_turn_streams_lifecycle_and_commits_one_complete_pair() {
    let model = synthetic_model(
        "synthetic-model",
        vec![ReasoningLevel::Off, ReasoningLevel::Medium],
    );
    let usage = TokenUsage {
        input: 4,
        output: 2,
        cache_read: 1,
        cache_write: 0,
        reasoning: None,
        total: 6,
    };
    let expected = ExpectedConversationRequest::new(
        "Be concise.",
        vec![Message::User(UserMessage::new("Hello"))],
        model.id().clone(),
        ReasoningLevel::Medium,
    );
    let provider = ScriptedProvider::new(
        vec![model.clone()],
        vec![ScriptedTurn::new(
            expected,
            vec![
                ProviderEvent::Started,
                ProviderEvent::ContentStarted {
                    index: 0,
                    kind: ContentKind::Text,
                },
                ProviderEvent::ContentDelta {
                    index: 0,
                    delta: "Hi".to_owned(),
                },
                ProviderEvent::ContentFinished { index: 0 },
                ProviderEvent::Completed(ProviderTurnCompletion::new(
                    usage.clone(),
                    FinishReason::Completed,
                )),
            ],
        )],
    );
    let provider_handle = provider.clone();
    let mut conversation = AgentConversation::new(
        "Be concise.".to_owned(),
        provider,
        model.id().clone(),
        ReasoningLevel::Medium,
    )
    .unwrap();

    let events = tauri::async_runtime::block_on(async {
        let mut stream = conversation.send("Hello".to_owned()).unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });

    assert_eq!(events.len(), 5);
    assert!(matches!(events[0], ConversationEvent::Started));
    assert!(matches!(
        events[1],
        ConversationEvent::ContentStarted {
            index: 0,
            kind: ContentKind::Text
        }
    ));
    assert!(matches!(
        &events[2],
        ConversationEvent::ContentDelta { index: 0, delta } if delta == "Hi"
    ));
    assert!(matches!(
        events[3],
        ConversationEvent::ContentFinished { index: 0 }
    ));
    assert!(matches!(events[4], ConversationEvent::Completed { .. }));
    assert_eq!(conversation.messages().len(), 2);
    assert_eq!(
        conversation.messages()[0],
        Message::User(UserMessage::new("Hello"))
    );
    let Message::Assistant(assistant) = &conversation.messages()[1] else {
        panic!("second committed message must be the assistant reply");
    };
    assert_eq!(
        assistant.content(),
        &[AssistantContent::Text("Hi".to_owned())]
    );
    assert_eq!(assistant.usage(), &usage);
    assert_eq!(assistant.finish_reason(), FinishReason::Completed);
    assert_eq!(assistant.model().id(), model.id());
    provider_handle.assert_exhausted().unwrap();
}

#[test]
fn multi_turn_requests_replay_only_committed_pairs_and_keep_one_conversation_id() {
    let first_model = synthetic_model(
        "synthetic-first",
        vec![ReasoningLevel::Off, ReasoningLevel::Medium],
    );
    let second_model = synthetic_model(
        "synthetic-second",
        vec![ReasoningLevel::Low, ReasoningLevel::High],
    );
    let first_assistant = synthetic_assistant_message(
        vec![AssistantContent::Text("First reply".to_owned())],
        first_model.clone(),
        TokenUsage::default(),
        FinishReason::Completed,
    );
    let provider = ScriptedProvider::new(
        vec![first_model.clone(), second_model.clone()],
        vec![
            ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "System",
                    vec![Message::User(UserMessage::new("First"))],
                    first_model.id().clone(),
                    ReasoningLevel::Medium,
                ),
                completed_text("First reply"),
            ),
            ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "System",
                    vec![
                        Message::User(UserMessage::new("First")),
                        first_assistant,
                        Message::User(UserMessage::new("Second")),
                    ],
                    second_model.id().clone(),
                    ReasoningLevel::High,
                ),
                completed_text("Second reply"),
            ),
        ],
    );
    let handle = provider.clone();
    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        first_model.id().clone(),
        ReasoningLevel::Medium,
    )
    .unwrap();

    run_turn(&mut conversation, "First");
    conversation
        .select_model(second_model.id().clone())
        .unwrap();
    assert_eq!(conversation.reasoning_level(), ReasoningLevel::High);
    run_turn(&mut conversation, "Second");

    assert_eq!(conversation.messages().len(), 4);
    let requests = handle.recorded_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].conversation_id(), requests[1].conversation_id());
    assert_eq!(requests[1].messages().len(), 3);
    handle.assert_exhausted().unwrap();
}

#[test]
fn reasoning_and_text_blocks_preserve_provider_order() {
    let model = synthetic_model("synthetic-model", vec![ReasoningLevel::Medium]);
    let provider = ScriptedProvider::new(
        vec![model.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Explain"))],
                model.id().clone(),
                ReasoningLevel::Medium,
            ),
            vec![
                ProviderEvent::Started,
                ProviderEvent::ContentStarted {
                    index: 0,
                    kind: ContentKind::Reasoning,
                },
                ProviderEvent::ContentDelta {
                    index: 0,
                    delta: "Summary".to_owned(),
                },
                ProviderEvent::ContentFinished { index: 0 },
                ProviderEvent::ContentStarted {
                    index: 1,
                    kind: ContentKind::Text,
                },
                ProviderEvent::ContentDelta {
                    index: 1,
                    delta: "Answer".to_owned(),
                },
                ProviderEvent::ContentFinished { index: 1 },
                ProviderEvent::Completed(ProviderTurnCompletion::new(
                    TokenUsage::default(),
                    FinishReason::LengthLimit,
                )),
            ],
        )],
    );
    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        model.id().clone(),
        ReasoningLevel::Medium,
    )
    .unwrap();

    run_turn(&mut conversation, "Explain");

    let Message::Assistant(assistant) = &conversation.messages()[1] else {
        panic!("assistant reply was not committed");
    };
    assert_eq!(
        assistant.content(),
        &[
            AssistantContent::Reasoning("Summary".to_owned()),
            AssistantContent::Text("Answer".to_owned()),
        ]
    );
    assert_eq!(assistant.finish_reason(), FinishReason::LengthLimit);
}

#[test]
fn dropping_an_unfinished_turn_rolls_back_pending_messages() {
    let model = synthetic_model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![model.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Dropped"))],
                model.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_text("Not consumed"),
        )],
    );
    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        model.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    let stream = conversation.send("Dropped".to_owned()).unwrap();
    drop(stream);

    assert!(conversation.messages().is_empty());
}

#[test]
fn model_selection_rejects_unknown_ids_and_normalizes_reasoning() {
    let sparse = synthetic_model(
        "synthetic-sparse",
        vec![ReasoningLevel::Off, ReasoningLevel::Medium],
    );
    let provider = ScriptedProvider::new(vec![sparse.clone()], Vec::new());
    let missing = ModelId::new("missing").unwrap();
    let error = match AgentConversation::new(
        "System".to_owned(),
        provider.clone(),
        missing.clone(),
        ReasoningLevel::Low,
    ) {
        Ok(_) => panic!("unknown initial model unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.category, AgentErrorCategory::ModelUnavailable);

    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        sparse.id().clone(),
        ReasoningLevel::Low,
    )
    .unwrap();
    assert_eq!(conversation.reasoning_level(), ReasoningLevel::Medium);
    assert_eq!(
        conversation.set_reasoning_level(ReasoningLevel::Minimal),
        ReasoningLevel::Off
    );
    assert_eq!(
        conversation.select_model(missing).unwrap_err().category,
        AgentErrorCategory::ModelUnavailable
    );
    assert_eq!(conversation.model().id(), sparse.id());
}

#[test]
fn failed_and_aborted_turns_roll_back_user_and_partial_assistant_messages() {
    let model = synthetic_model("synthetic-model", vec![ReasoningLevel::Off]);
    let expected_failure = ExpectedConversationRequest::new(
        "System",
        vec![Message::User(UserMessage::new("Fail"))],
        model.id().clone(),
        ReasoningLevel::Off,
    );
    let expected_abort = ExpectedConversationRequest::new(
        "System",
        vec![Message::User(UserMessage::new("Abort"))],
        model.id().clone(),
        ReasoningLevel::Off,
    );
    let provider = ScriptedProvider::new(
        vec![model.clone()],
        vec![
            ScriptedTurn::new(
                expected_failure,
                vec![
                    ProviderEvent::Started,
                    ProviderEvent::ContentStarted {
                        index: 0,
                        kind: ContentKind::Text,
                    },
                    ProviderEvent::ContentDelta {
                        index: 0,
                        delta: "partial".to_owned(),
                    },
                    ProviderEvent::Failed(job_radar_lib::agent::AgentError {
                        category: AgentErrorCategory::Transport,
                        message: "provider transport failed".to_owned(),
                        retry_after: None,
                    }),
                ],
            ),
            ScriptedTurn::new(
                expected_abort,
                vec![ProviderEvent::Started, ProviderEvent::Aborted],
            ),
        ],
    );
    let handle = provider.clone();
    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        model.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    let failed = run_turn(&mut conversation, "Fail");
    assert!(matches!(
        failed.last(),
        Some(ConversationEvent::Failed { error })
            if error.category == AgentErrorCategory::Transport
    ));
    assert!(conversation.messages().is_empty());
    let aborted = run_turn(&mut conversation, "Abort");
    assert!(matches!(aborted.last(), Some(ConversationEvent::Aborted)));
    assert!(conversation.messages().is_empty());
    handle.assert_exhausted().unwrap();
}

#[test]
fn malformed_provider_lifecycles_fail_once_without_committing() {
    let malformed_scripts = vec![
        Vec::new(),
        vec![ProviderEvent::ContentStarted {
            index: 0,
            kind: ContentKind::Text,
        }],
        vec![
            ProviderEvent::Started,
            ProviderEvent::ContentStarted {
                index: 1,
                kind: ContentKind::Text,
            },
        ],
        vec![ProviderEvent::Started],
        vec![
            ProviderEvent::Started,
            ProviderEvent::Completed(ProviderTurnCompletion::new(
                TokenUsage::default(),
                FinishReason::Completed,
            )),
            ProviderEvent::Started,
        ],
    ];

    for (index, events) in malformed_scripts.into_iter().enumerate() {
        let model = synthetic_model("synthetic-model", vec![ReasoningLevel::Off]);
        let text = format!("Malformed {index}");
        let provider = ScriptedProvider::new(
            vec![model.clone()],
            vec![ScriptedTurn::new(
                ExpectedConversationRequest::new(
                    "System",
                    vec![Message::User(UserMessage::new(&text))],
                    model.id().clone(),
                    ReasoningLevel::Off,
                ),
                events,
            )],
        );
        let mut conversation = AgentConversation::new(
            "System".to_owned(),
            provider,
            model.id().clone(),
            ReasoningLevel::Off,
        )
        .unwrap();

        let public_events = run_turn(&mut conversation, &text);

        assert!(matches!(
            public_events.last(),
            Some(ConversationEvent::Failed { error })
                if error.category == AgentErrorCategory::Provider
        ));
        assert_eq!(
            public_events
                .iter()
                .filter(|event| matches!(event, ConversationEvent::Failed { .. }))
                .count(),
            1
        );
        assert!(conversation.messages().is_empty());
    }
}

#[test]
fn scripted_provider_detects_request_mismatches_and_missing_calls() {
    let model = synthetic_model("synthetic-model", vec![ReasoningLevel::Off]);
    let provider = ScriptedProvider::new(
        vec![model.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "Different system",
                vec![Message::User(UserMessage::new("Hello"))],
                model.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_text("unused"),
        )],
    );
    let handle = provider.clone();
    let mut conversation = AgentConversation::new(
        "System".to_owned(),
        provider,
        model.id().clone(),
        ReasoningLevel::Off,
    )
    .unwrap();

    let events = run_turn(&mut conversation, "Hello");

    assert!(matches!(
        events.last(),
        Some(ConversationEvent::Failed { error })
            if error.category == AgentErrorCategory::InvalidConfiguration
    ));
    assert!(handle.assert_exhausted().is_err());

    let missing_call = ScriptedProvider::new(
        vec![model.clone()],
        vec![ScriptedTurn::new(
            ExpectedConversationRequest::new(
                "System",
                vec![Message::User(UserMessage::new("Never sent"))],
                model.id().clone(),
                ReasoningLevel::Off,
            ),
            completed_text("unused"),
        )],
    );
    assert!(missing_call.assert_exhausted().is_err());
}
