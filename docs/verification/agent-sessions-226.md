# Agent sessions (#226) verification

This document covers the Rust session manager delivered by #226. It does not
claim provider orchestration, Tauri command, UI, or live-provider evidence.

## Deterministic requirement-to-evidence matrix

| Resolved behavior | Test/evidence | Gate | Platforms | Result |
| --- | --- | --- | --- | --- |
| #221 pinned v3, delayed first durability, settings/name, typed replay and redacted projections | `draft_is_ephemeral_then_publishes_and_reopens`, `typed_replay_and_errors_have_redacted_debug`, hand-reviewed synthetic fixtures | Public-seam integration | macOS host | Pass |
| Per-entry support; pinned unsupported schemas are validated, remain visible where possible, and never enter continuation context | `conformance_fixtures_cover_reconstruction_and_unsupported_context`, `malformed_graph_fixtures_are_damaged_even_with_unsupported_entries`; image/tool-call/user-image/tool-result/Bash/custom/label/branch-summary fixtures | Public-seam conformance | portable Rust | Pass |
| Active/off-path reconstruction; model/Reasoning history; name clearing; replay/Thinking | `active-off-path-v3.jsonl`, `history-name-replay-v3.jsonl` exercised by `conformance_fixtures_cover_reconstruction_and_unsupported_context` | Public-seam conformance | portable Rust | Pass |
| Supported message pairing remains validated in a read-only document | `damaged-pair-with-unsupported-v3.jsonl` exercised by `malformed_graph_fixtures_are_damaged_even_with_unsupported_entries` | Public-seam conformance | portable Rust | Pass |
| Duplicate/missing/cyclic graph rejection and malformed recognized-entry/message rejection | damaged graph/schema fixtures exercised by `malformed_graph_fixtures_are_damaged_even_with_unsupported_entries` | Public-seam conformance | portable Rust | Pass |
| #222 native atomic no-replace first publication and deterministic post-publish sync behavior | `first_publication_is_atomic_no_overwrite_and_reopens_after_sync_failure`, directory-sync fault case | Integration/fault injection | macOS host; cfg implementations for Linux/Windows | Pass on host |
| Bounded structural recovery (final two entries/32 MiB), including whitespace and invalid UTF-8, with ambiguous damage unchanged | `structural_recovery_handles_whitespace_and_invalid_utf8_but_not_ambiguity`, `large_session_recovery_discards_complete_user_and_truncated_assistant`, recovery fixtures | Integration | portable Rust | Pass |
| Checkpoints for temp write/sync, publication, directory sync, append write/sync, truncate/sync, lock, and Trash | `checkpoint_faults_cover_publication_append_recovery_lock_and_trash` | Deterministic fault injection | portable Rust | Pass |
| Reproducible arbitrary malformed bytes, graph parents, and 16 MiB size boundary | fixed-seed proptests `arbitrary_session_bytes_fail_closed_without_mutation`, `malformed_parent_graphs_are_damaged`, `oversized_final_frames_fail_closed` | Property-based security gate | portable Rust | Pass |
| Process death: delayed publication, partial append recovery, lock release, explicit read-only snapshot reload | `subprocess_crash_and_snapshot_contracts_use_explicit_ipc` (checkpoint/readiness pipes; no sleeps or retries) | Mandatory subprocess integration | macOS host | Pass |
| One writer and external-change poisoning | `second_open_is_read_only_until_writer_drops`, subprocess test, `same_length_external_change_poisoning_blocks_mutation` | Integration/subprocess | macOS host | Pass |
| Trash pathname identity: atomic move to unpredictable owned staging name, identity check, safe restore on failure, no permanent-delete fallback | `trash_failure_preserves_and_success_moves_without_delete_fallback`, Trash checkpoint case | Integration/fault injection | portable adapter contract | Pass |
| #224 compaction context reconstruction, including compatible empty details | `valid-compaction-empty-details-v3.jsonl` is exercised by `conformance_fixtures_cover_reconstruction_and_unsupported_context` | Public-seam conformance | portable Rust | Pass |
| Unix private modes and unsafe ancestor links | `storage_permissions_are_private`; existing root validation tests through public seam | Native integration | macOS host | Pass |
| Windows unsafe reparse rejection and protected current-user-only ACL | cfg(windows) native `FILE_ATTRIBUTE_REPARSE_POINT`, token SID, protected DACL, and stable file identity helpers in `storage.rs` | Windows compile/CI and native ACL smoke | Windows | External platform gate: target unavailable on this host |
| Credential/replay safety | canary assertions plus repository grep described below | Security gate | host | Pass |

## Synthetic fixture policy

All files under `src-tauri/tests/fixtures/agent_sessions/` are synthetic. They
contain fixed fake IDs, timestamps, attribution, replay values, and text. They
contain no captured provider traffic, user sessions, credentials, local paths,
or account data.

The corpus covers minimal v3, version rejection, active/off-path history,
model/Reasoning and naming history, replay/Thinking, every recognized
unsupported family in this slice with its pinned required fields, malformed
recognized schemas, pairing/duplicate/missing/cycle graphs, pinned terminal-reason
classifications, compaction context, and recoverable/non-recoverable final
suffixes. Fixed-seed property tests supplement these reviewed vectors with
bounded arbitrary malformed bytes and graph IDs.

## Platform and release gates

The implementation contains native no-replace rename adapters for Darwin
(`renamex_np(RENAME_EXCL)`), Linux (`renameat2(RENAME_NOREPLACE)`), and Windows
(`MoveFileExW` without replacement), plus Windows reparse and ACL hardening.
Only `aarch64-apple-darwin` and `wasm32-unknown-unknown` Rust targets were
installed on the verification host. Therefore a Windows target `cargo check`
and Windows ACL/reparse conditional execution are honest external platform
gates, not known implementation omissions.

Native desktop Trash success (Finder/Recycle Bin/Freedesktop), Windows ACL
inspection, and assembled Tauri/UI/live-provider smoke remain release-level
environmental gates outside this Rust-only issue slice. Trash failures never
fall back to permanent deletion.
