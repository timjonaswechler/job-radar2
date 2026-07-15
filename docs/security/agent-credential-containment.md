# Agent credential-containment proof

Issue [#189](https://github.com/timjonaswechler/job-radar2/issues/189) audits the completed Agent Conversation vertical slice against the map's credential-safety invariant. No live login, authenticated request, application-data credential file, environment credential, or account data was inspected during this audit.

## Automated commit safeguard

Run:

```bash
npm run test:agent-credential-safety
npm run check:agent-credentials
```

The dependency-free Node safeguard reads blobs from the Git index with `git ls-files --stage` and `git cat-file`. It therefore scans the exact tracked snapshot that would be committed, including staged content, while deliberately ignoring unrelated unstaged and untracked working-tree files. CI runs the safeguard self-tests and snapshot scan before npm dependency installation, builds, and tests.

The scanner rejects prohibited credential file names and high-confidence private-key, API-token, bearer-token, JWT-token, OAuth-literal, authorization-result, and email-address patterns. It fails closed for unmerged index entries, unreadable Git state, and oversized text. Known binary assets and NUL-containing blobs are not interpreted as text, but prohibited credential paths remain rejected. Diagnostics contain only the indexed path and stable rule; matched content is never printed.

Synthetic fixtures are allowed only when values are conspicuously marked as synthetic/fabricated, use a small fixed test placeholder, or use a reserved invalid/test email domain. Scanner tests assemble prohibited specimens from fragments so no prohibited specimen is itself stored in the repository. The tests also prove positive detections, safe field-name/documentation mentions, synthetic fixture allowance, redacted diagnostics, path rejection, binary and size handling, and the index-versus-working-tree distinction.

## Existing behavior proof

| Invariant | Executable evidence |
| --- | --- |
| Storage remains outside repositories and uses private permissions | `agent::auth::tests::save_load_status_and_remove_use_private_repository_external_storage`, `insecure_storage_locations_are_rejected_without_creating_files`, and `malformed_storage_and_symlinks_fail_closed_without_leaking_paths_or_contents` |
| Parent directory is `0700`; credential and lock files are `0600` | Assertions through the production `AuthStorage` interface in `save_load_status_and_remove_use_private_repository_external_storage`; mode enforcement is shared production code |
| Writes and refresh are serialized without losing other provider updates | `concurrent_provider_mutations_preserve_both_latest_entries`, `concurrent_expired_credential_resolution_refreshes_once_and_persists_before_use`, and `refresh_lock_wait_does_not_block_other_async_work` |
| Expiry is checked after lock acquisition; rotated credentials persist before use | `expiry_clock_is_evaluated_after_waiting_for_the_storage_lock`, `exact_expiry_refresh_rotates_and_persists_before_returning`, and the concurrent refresh test |
| Refresh/storage/provider failures do not expose source values, paths, or bodies | `refresh_failure_keeps_expired_credential_and_returns_only_redacted_diagnostics`, `malformed_token_and_provider_failures_return_fixed_redacted_errors`, streaming redaction tests, and debug-harness category rendering tests |
| Credentials stay behind authentication/provider implementation | `OpenAiCodexProvider` resolves authentication per turn; private credential-bearing transport structures have no public accessor or `Debug`; streaming tests inspect only synthetic boolean matches |
| Provider-neutral callers and deterministic tests share one seam | External `src-tauri/tests/agent_conversation.rs` tests exercise `AgentConversation` with the regularly compiled `ScriptedProvider`; production Codex tests drive the same conversation interface |
| Debug harness does not echo secret manual input or raw errors | `secret_authorization_input_is_not_written_back_or_logged` and `event_renderer_distinguishes_reasoning_and_never_prints_error_payloads` |

Focused verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::auth::tests
cargo test --manifest-path src-tauri/Cargo.toml agent::openai_codex --no-fail-fast
cargo test --manifest-path src-tauri/Cargo.toml --test agent_conversation
cargo test --manifest-path src-tauri/Cargo.toml --features agent-debug --bin agent-debug
```

## Limits

Pattern scanning materially reduces accidental commits but cannot prove that every arbitrary opaque string is harmless. It intentionally favors high-confidence findings to remain usable across the full repository. Known binary assets are path-checked but not content-decoded, and commit metadata or Git history are outside the index snapshot. Reviewers should still inspect staged changes and may run an independent secret scanner. The safeguard never replaces protected external storage, redacted interfaces, or synthetic-only tests.
