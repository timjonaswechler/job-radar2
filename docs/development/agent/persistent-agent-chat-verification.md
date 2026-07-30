# Persistent Agent Chat verification

This is the aggregate requirement-to-evidence record for [issue #231](https://github.com/timjonaswechler/job-radar2/issues/231) and the persistent Agent Chat map. It records only synthetic or value-free outcomes. It must never contain Chat text, Thinking, replay fields, credentials, account identifiers, headers, provider payloads, session identifiers, or local paths.

## Stable gates

Run the portable proof locally with:

```bash
just agent-chat-proof
```

CI runs the same Agent crate, Tauri application, and React shell gates on macOS, Windows, and Linux before building each native desktop bundle. The full local handoff gate remains `just verify`.

The opt-in macOS storage smoke creates one synthetic session under a temporary application-data root and moves it through the production macOS Trash adapter:

```bash
just agent-chat-macos-smoke
```

It records no session content or path. The ordinary test suite skips this test so a normal test run never modifies Trash.

## Aggregate requirement-to-evidence matrix

| Requirement | Evidence | Gate | Platforms | Result |
| --- | --- | --- | --- | --- |
| Strict pinned Pi JSONL v3 subset, supported/read-only/damaged classification, active-path reconstruction, model and Reasoning Level history, naming, typed replay, and compaction context | Reviewed synthetic corpus in `src-tauri/crates/agent/tests/fixtures/agent_sessions/`; `pinned_fixtures_classify_through_public_seam`, `conformance_fixtures_cover_reconstruction_and_unsupported_context`, `malformed_graph_fixtures_are_damaged_even_with_unsupported_entries` | Synthetic conformance through public session seam | Portable Rust | Pass |
| Bounded malformed data, graph, UTF-8, frame, entry, and append handling without panic or unsafe mutation | Fixed-seed property tests and `structural_recovery_handles_whitespace_and_invalid_utf8_but_not_ambiguity` | Property/security | Portable Rust | Pass |
| Delayed atomic publication, synchronized complete turns and metadata, bounded final-suffix repair, and unchanged ambiguous damage | Session publication/recovery tests and `checkpoint_faults_cover_publication_append_recovery_lock_and_trash` | Deterministic integration and fault injection | Portable Rust | Pass |
| Crash/relaunch state, process-death lock release, one writer, read-only snapshots, explicit reload, and external-change poisoning | `subprocess_crash_and_snapshot_contracts_use_explicit_ipc`, locking and external-change tests | Subprocess/restart and concurrency | Portable Rust | Pass |
| Durable completed turns, exact restart continuation, typed replay, unavailable-model remediation, cancellation, and Not-saved blocking | Agent Chat public-seam tests including `completed_turn_is_durable_before_success_and_restart_resumes_exact_context`, cancellation, remediation, and persistence-failure cases | Deterministic provider-neutral integration | Portable Rust | Pass |
| Manual/threshold/overflow compaction, cut points, iterative and split summaries, cancellation/failure, one retry, context limits, and full-history retention | Agent Chat compaction contract tests introduced in `62aac64` | Deterministic compaction integration | Portable Rust | Pass |
| Redacted application commands/events for create, open, send, stop, model/Reasoning changes, and compaction | Six `agent::chat_application` desktop integration tests, including content/replay canaries and stable provider failures | Tauri application contract | macOS host; all desktop targets in CI | Pass locally; cross-platform result comes from the CI run for this revision |
| Reusable Chat/Canvas shell: messages, Reasoning, input, model and Reasoning controls, context indication, streaming/Stop, compaction, exceptional states, accessibility, and resizing | 14 focused Vitest tests under `src/features/agent-chat/tests/`; approved checkpoints recorded on #230 | React behavior/accessibility plus human checkpoints | Browser-independent tests; native macOS review | Pass |
| Credential, authorization, account, header, replay, content, and path redaction | Synthetic canaries in session, Agent Chat, provider, application, and UI tests; repository credential safeguard | Security/redaction | Portable plus repository scan | Pass locally |
| Private storage, two-process locking/release, and safe Trash with no permanent-delete fallback | Portable fault/subprocess tests; `native_macos_storage_locking_and_trash_smoke` uses production storage/lock/Trash adapters | Native macOS smoke | macOS | Pass on 2026-07-29 |
| Native Tauri startup and reusable shell | #230 live shell checkpoints; release bundle built and launched with output discarded | Human/native acceptance | macOS | Pass on 2026-07-29; no private output retained |
| Portable tests and desktop packaging | `desktop-build` matrix in `.github/workflows/ci.yml` runs Agent, Tauri application, frontend, and bundle gates | CI | macOS, Windows, Linux | Required for this revision; do not mark the ticket resolved until the run passes |
| Configured-provider first durable turn, restart continuation, cancellation, explicit model change, manual compaction, and continuation | Sanitized opt-in assembled smoke described below | Live provider | macOS | Required before resolution; existing #230 evidence proves live persistent streaming and Stop only |

## Slice aggregation

| Slice | Delivery | Evidence retained here |
| --- | --- | --- |
| #226 session manager | `b9fb9a6` | v3 corpus, persistence/recovery faults, property tests, subprocess locking/restart, storage and Trash |
| #227 durable Agent Chats | `6c7a692` | completed-turn durability, resume, cancellation, model remediation, Not-saved behavior |
| #228 compaction | `62aac64` | manual/automatic/overflow compaction and context reconstruction contracts |
| #229 application/Tauri API | `f67345d` | public application seam, streaming events, redacted projections/errors |
| #230 reusable shell | `a6413b0` | React behavior/accessibility tests and two approved human checkpoints |

Earlier per-slice verification files were removed during the repository documentation reorganization. This page is the maintained aggregate; executable test names, commits, and issue resolutions remain the source of truth.

## Sanitized opt-in provider smoke

Run this only with a disposable synthetic conversation and an explicitly configured supported provider. Record only `Pass`, `Fail: <stable category>`, or `Not run`, plus a public model identifier only when needed to diagnose model availability.

| Check | Outcome |
| --- | --- |
| First complete turn becomes durable | Not run for #231 |
| Relaunch opens the same Chat and continues it | Not run for #231 |
| Stop/cancellation creates no partial durable turn | Live Stop approved on #230; assembled restart check not run |
| Explicit Agent Model change is retained | Not run for #231 |
| Manual compaction succeeds and remains visible after relaunch | Not run for #231 |
| Conversation continues after compaction | Not run for #231 |

Do not copy terminal output into this document. A provider or environmental failure never triggers an automatic resend.

## Native evidence policy

macOS is the blocking native smoke platform for the initial feature. Windows and Linux run portable contracts and native builds in CI; their native Trash, ACL/permission, startup, and shell smokes become blocking only when Agent Chats are released there as supported features.

A closure comment for #231 may link the passing CI run and summarize the provider table using only value-free outcomes. Until both are present, this document deliberately reports the remaining external gates instead of overstating completion.
