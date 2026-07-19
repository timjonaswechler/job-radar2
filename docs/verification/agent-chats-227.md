# Durable Agent Chats (#227) verification

This document covers the provider-neutral Rust Agent Chat integration delivered by #227. Tauri commands, the React shell, compaction generation, live-provider evidence, and native release smokes belong to later map tickets.

## Deterministic requirement-to-evidence matrix

| Resolved behavior | Test/evidence | Gate | Platforms | Result |
| --- | --- | --- | --- | --- |
| A completed provider turn is reported successful only after the complete User/Assistant pair is synchronized | `completed_turn_is_durable_before_success_and_restart_resumes_exact_context` | Public-seam integration | portable Rust | Pass |
| Restart reopens the validated active path, preserves the session-derived conversation identity, per-turn attribution, selected model and effective Reasoning Level, and sends exact prior context | `completed_turn_is_durable_before_success_and_restart_resumes_exact_context`, `restart_preserves_historical_attribution_across_a_model_change` | Public-seam restart integration | portable Rust | Pass |
| Only typed `responseId`, `textSignature`, and `thinkingSignature` replay data crosses the persistence seam | typed replay canaries in `completed_turn_is_durable_before_success_and_restart_resumes_exact_context`; pinned adapter tests `model_change_drops_old_replay_signatures_and_resolves_auth_each_turn` and `refusal_content_preserves_visible_order_and_replays_only_typed_text_metadata` | Persistence/provider conformance and redaction | portable Rust | Pass |
| Failed, provider-aborted, malformed, dropped, and caller-cancelled turns publish and append nothing | `unsuccessful_and_dropped_turns_never_publish_or_enter_resume_context`, `caller_cancellation_wakes_a_pending_turn_and_never_publishes_a_partial_chat`, `caller_cancellation_after_partial_output_wins_without_committing_the_ready_completion` | Public-seam integration | portable Rust | Pass |
| Caller cancellation wakes a pending turn, wins before a ready completion is observed, and cancels the concrete HTTP operation | public-seam cancellation tests plus `caller_cancellation_drops_the_in_flight_http_request` | Deterministic cancellation/transport | portable Rust | Pass |
| Persistence failure after provider completion yields a copyable Not-saved terminal, blocks another send, and requires explicit reload without automatic resend | `persistence_failure_is_not_success_and_reload_restores_the_last_durable_state` with deterministic `TempSync` failure | Public-seam fault injection | portable Rust | Pass |
| An unavailable recorded provider/model leaves history readable and sending disabled until explicit selection; remediation persists the model change and deterministically normalizes the effective Reasoning Level from the latest explicit Reasoning Level entry | `unavailable_recorded_model_is_readable_until_explicit_model_remediation` | Public-seam restart integration | portable Rust | Pass |
| Initial and explicit later Reasoning Level changes are normalized, persisted, and restored; model changes persist as one standard entry and derive their constrained effective level without a second partial metadata transaction | restart test starts at `Medium`; unavailable-model remediation derives effective `Low` from recorded `Off`; `failed_reasoning_change_blocks_sends_until_reload_restores_durable_settings` covers a synchronization failure and explicit reload | Public-seam integration/fault injection | portable Rust | Pass |
| Read-only, unsupported, damaged, locking, external-change, recovery, and compaction-derived continuation semantics remain owned by the verified #226 session manager | `agent_sessions` suite and `docs/verification/agent-sessions-226.md`; Agent Chat gates send/settings on session access | Regression | portable Rust plus #226 native gates | Pass |
| Agent Chat Debug output redacts streamed and completed message content and Not-saved response content | custom `AgentChatEvent` Debug implementation; typed replay fields remain opaque in `AssistantMessage` Debug | Security/redaction | portable Rust | Pass |

## Scope and release gates

The integration remains text-only and linear. It introduces no tools, images, branching, global Chat browser, Tauri commands, UI, or compaction policy/generation.

The opt-in configured-provider restart/cancellation smoke, native macOS assembled smoke, Tauri contract tests, and React behavioral approval remain explicit gates for #229–#231. They are not claimed by this Rust integration slice.
