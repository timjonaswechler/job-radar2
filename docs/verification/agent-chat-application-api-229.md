# Persistent Agent Chat application API (#229) verification

This matrix records deterministic evidence for the redacted application/Tauri boundary. The React Chat/Canvas shell, context ownership and Changelog discovery, context-specific canvases, naming/Trash placement, and native release evidence remain follow-up work.

| Resolved behavior | Test/evidence | Gate | Result |
| --- | --- | --- | --- |
| Draft creation and identified resume share one application service while the system prompt remains opaque | `application_service_streams_visible_content_then_projects_only_durable_chat_state`; `identified_resume_preserves_unavailable_model_without_fallback` | Public-seam integration | Pass |
| Send emits typed, monotonic events and completes with the durable `AgentChat` projection | `application_service_streams_visible_content_then_projects_only_durable_chat_state` | Public-seam integration | Pass |
| Stop wins before the spawned operation attaches its cancellation handle; a second operation is rejected | `immediate_stop_wins_the_start_race_and_only_one_operation_can_run_per_chat` | Cancellation/concurrency | Pass |
| Unavailable recorded models remain readable without silent fallback; explicit model and Reasoning Level changes return effective state | `identified_resume_preserves_unavailable_model_without_fallback` | Resume/settings | Pass |
| Manual compaction streams lifecycle events and exposes only a marker, reason, and token count—not summary text or storage entry IDs | `manual_compaction_is_streamed_and_snapshot_history_exposes_no_summary_or_storage_ids` | Compaction/redaction | Pass |
| System prompts, replay metadata, provider bodies, storage paths, compaction summaries, and internal entry IDs are absent from projections/errors/Debug output | application-service canary assertions and closed error mapping | Security/redaction | Pass |
| Tauri exposes create/open/send/stop/model/Reasoning Level/compact commands through the single `agent-chat-event` stream | registrations in `src-tauri/src/lib.rs`; thin adapters in `src-tauri/src/app/commands.rs` | Tauri contract/compile | Pass |

## Gate notes

- Tests use deterministic session IDs/timestamps, scripted providers, temporary app-data, and explicit cancellation; they use no network, credentials, real Chat data, sleeps, or wall-clock ordering.
- Persistence, crash recovery, locking, JSONL conformance, and compaction internals remain covered by the #226–#228 suites; this boundary delegates those guarantees to `AgentChat` and `SessionManager`.
- #230 owns UI accessibility and human approval. #231 owns live-provider and assembled native macOS evidence.
