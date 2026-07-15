# Agent Conversation core

Issue [#186](https://github.com/timjonaswechler/job-radar2/issues/186) implements the provider-neutral, ephemeral conversation contract accepted in [`ADR 0011`](adr/0011-minimal-agent-conversation-contract.md). The lifecycle behavior follows the Pi baseline pinned in [`docs/research/pi-rust-agent-baseline.md`](research/pi-rust-agent-baseline.md) without introducing provider transport, tools, or persistence.

## Behavior

- `AgentConversation` owns an immutable system prompt, one internal conversation identifier, the selected Agent Model and effective Reasoning Level, and only completed User/Assistant pairs.
- `send` includes the current User Message in the provider request but does not commit it yet. A validated `Completed` terminal event commits the User and complete Assistant Messages together. `Failed`, `Aborted`, malformed streams, and dropped streams commit neither.
- `ConversationEventStream` holds a mutable borrow of the conversation, so another turn or model/Reasoning Level mutation cannot overlap it.
- Provider events must begin once with `Started`, use contiguous indexed content blocks, and end once with `Completed`, `Failed`, or `Aborted`. Successful completion requires balanced start/delta/finish events; failures and aborts may terminate a partial block, which is rolled back. The provider stream must then close. Invalid sequences become one redacted provider failure.
- Assistant Messages expose Text and provider-approved Reasoning content, model, token usage, and finish reason. Opaque provider replay data remains private and has a redacted debug representation.
- Model selection is limited to the provider's static catalog. Unsupported Reasoning Levels normalize through the selected model's accepted nearest-level rule.

## Deterministic provider

`agent::testing::ScriptedProvider` is a regularly compiled second adapter at the same `ConversationProvider` seam. It verifies the complete app-owned request, enforces one stable non-empty conversation identifier, records requests for assertions, and fails deterministically for mismatched, unexpected, or unconsumed turns. Its fixtures contain synthetic values only.

## Verification

External integration tests cover lifecycle order, completed commits, failed/aborted rollback, multi-turn replay, stable conversation identity, model and Reasoning Level changes, malformed provider streams, and scripted-provider mismatch handling. A compile-fail doctest verifies that the stream's mutable borrow prevents concurrent mutation.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test agent_conversation
cargo test --manifest-path src-tauri/Cargo.toml --doc
```
