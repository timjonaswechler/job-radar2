# Persistent Agent Chat application API (#229) verification

This document records evidence for the redacted Tauri application boundary delivered by #229. The React Chat/Canvas shell, context ownership and Changelog discovery, context-specific canvases, rename/Trash affordances, and native release evidence remain in their owning follow-up tickets.

| Resolved behavior | Test/evidence | Gate | Platforms | Result |
| --- | --- | --- | --- | --- |
| Draft creation and identified resume use one application-wide service while the caller supplies an opaque, non-projected system prompt | `application_service_streams_visible_content_then_projects_only_durable_chat_state`; `identified_resume_preserves_unavailable_model_without_fallback` | Public-seam deterministic integration | portable Rust | Pass |
| Send streams typed, sequenced events and ends with the durable Agent Chat projection produced by `AgentChat` | `application_service_streams_visible_content_then_projects_only_durable_chat_state` | Public-seam deterministic integration | portable Rust | Pass |
| Stop wins even when requested before the spawned operation attaches its cancellation handle; a second operation for the same Chat is rejected | `immediate_stop_wins_the_start_race_and_only_one_operation_can_run_per_chat` | Cancellation/concurrency regression | portable Rust | Pass |
| Explicit Agent Model and Reasoning Level changes remain unavailable during active operations, use the latest registry snapshot, and return effective projected state | `model_selection_uses_the_latest_provider_registry_snapshot`; application-service locking plus Reasoning Level assertions in `application_service_streams_visible_content_then_projects_only_durable_chat_state`; existing `agent::chats` model-remediation tests | Public seam + regression | portable Rust | Pass |
| Manual compaction streams lifecycle events, retains full durable history, and exposes only a marker/reason/token count—not summary text or storage entry IDs | `manual_compaction_is_streamed_and_snapshot_history_exposes_no_summary_or_storage_ids`; existing `agent::chats` compaction suite | Public seam + regression | portable Rust | Pass |
| A resumed Chat with an unavailable recorded Agent Model stays readable and cannot silently substitute the provider's available model | `identified_resume_preserves_unavailable_model_without_fallback` | Public-seam deterministic integration | portable Rust | Pass |
| Writable, model-unavailable, read-only/unsupported, damaged-error, recovery-notice, and Not-saved semantics are mapped to stable projections/events or stable redacted command errors | projection/error mapping in `agent::chat_application`; existing session and Agent Chat state suites | Contract + regression | portable Rust | Pass |
| Ordinary visible text/reasoning and deltas are the only content payloads; system prompts, replay metadata, credentials, storage paths, provider bodies, generated compaction summaries, and identifiers are absent from Debug/error output | Debug canary assertions in `application_service_streams_visible_content_then_projects_only_durable_chat_state`; compaction serialization assertion; fixed error mapping | Security/redaction | portable Rust | Pass |
| Tauri exposes only create/open/send/stop/model/Reasoning Level/compact commands and the `agent-chat-event` stream; no list, rename, Trash, context, Changelog, or Canvas API is introduced | command registrations in `src-tauri/src/lib.rs` and thin adapters in `src-tauri/src/app/commands.rs` | Tauri contract | portable compile | Pass |

## Gate notes

- No network, real credentials, real Chat data, wall-clock ordering, or provider recordings are used.
- Crash/recovery, JSONL conformance, locking, and persistence faults remain covered by the #226–#228 suites because #229 delegates completed-turn durability to `AgentChat`/`SessionManager` rather than reimplementing it.
- UI accessibility and human checkpoints are not applicable to this backend/Tauri boundary slice; #230 owns them.
- Live-provider and native macOS assembled evidence remains owned by #231.
