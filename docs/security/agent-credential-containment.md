# Agent credential-containment proof

Issues [#189](https://github.com/timjonaswechler/job-radar2/issues/189) and [#217](https://github.com/timjonaswechler/job-radar2/issues/217) audit the Agent Conversation and provider-configuration slices against the credential-safety invariant. The #217 verification inspected only value-free status and filesystem modes; the operator did not inspect or retain credential/configuration contents, environment credentials, account data, prompts, or provider responses.

## Automated commit safeguard

Run:

```bash
npm run test:agent-credential-safety
npm run check:agent-credentials
```

The dependency-free Node safeguard reads blobs from the Git index with `git ls-files --stage` and `git cat-file`, then scans tracked working-tree files and non-ignored untracked files reported by `git ls-files`. It therefore checks both the exact staged snapshot and current implementation files; a safe working-tree replacement cannot hide a staged finding. CI runs the safeguard self-tests and repository scan before npm dependency installation, builds, and tests.

The scanner rejects prohibited credential file names and high-confidence private-key, API-token, bearer-token, JWT-token, OAuth-literal, authorization-result, and email-address patterns. It fails closed for unmerged index entries, unreadable Git state or working-tree files, symlinks, and oversized text. Known binary assets and NUL-containing blobs are not interpreted as text, but prohibited credential paths remain rejected. Diagnostics contain only the repository-relative path and stable rule; matched content is never printed.

Synthetic fixtures are allowed only when values are conspicuously marked as synthetic/fabricated, use a small fixed test placeholder, or use a reserved invalid/test email domain. Scanner tests assemble prohibited specimens from fragments so no prohibited specimen is itself stored in the repository. The tests also prove positive detections, safe field-name/documentation mentions, synthetic fixture allowance, redacted diagnostics, path rejection, binary and size handling, and the index-versus-working-tree distinction.

## Existing behavior proof

| Invariant | Executable evidence |
| --- | --- |
| Storage remains outside repositories and uses private permissions | `agent::auth::tests::set_resolve_status_and_logout_use_private_repository_external_storage`, `insecure_storage_locations_are_rejected_without_creating_files`, and `malformed_storage_and_symlinks_fail_closed_without_leaking_paths_or_contents` |
| Parent directory is `0700`; credential and lock files are `0600` | Assertions through the production `AuthStorage` interface in `set_resolve_status_and_logout_use_private_repository_external_storage`; mode enforcement is shared production code |
| Writes and refresh are serialized without losing other provider updates | `concurrent_provider_mutations_preserve_both_latest_entries`, `concurrent_expired_credential_resolution_refreshes_once_and_persists_before_use`, and `refresh_lock_wait_does_not_block_other_async_work` |
| Expiry is checked after lock acquisition; rotated credentials persist before use | `expiry_clock_is_evaluated_after_waiting_for_the_storage_lock`, `exact_expiry_refresh_rotates_and_persists_before_returning`, and the concurrent refresh test |
| Refresh/storage/provider failures do not expose source values, paths, or bodies | `refresh_failure_keeps_expired_credential_and_returns_only_redacted_diagnostics`, `malformed_token_and_provider_failures_return_fixed_redacted_errors`, streaming redaction tests, and debug-harness category rendering tests |
| Credentials stay behind authentication/provider implementation | `OpenAiCodexProvider` resolves authentication per turn; private credential-bearing transport structures have no public accessor or `Debug`; streaming tests inspect only synthetic boolean matches |
| Provider-neutral callers and deterministic tests share one seam | External `src-tauri/tests/agent_conversation.rs` tests exercise `AgentConversation` with the regularly compiled `ScriptedProvider`; production Codex tests drive the same conversation interface |
| Debug harness does not echo secret manual input or raw errors | `secret_authorization_input_is_not_written_back_or_logged` and `event_renderer_distinguishes_reasoning_and_never_prints_error_payloads` |
| Legacy migration is no-loss, private, and conflict-safe | `src-tauri/tests/agent_data_root.rs` covers migration, verified `0700`/`0600` modes, redacted conflict handling, canonical-root validation, and symlink rejection |
| Direct keys and exact environment references remain value-free at registry boundaries | `availability_resolves_direct_and_environment_references_without_exposing_values` in `src-tauri/tests/agent_model_registry.rs`; auth resolution is covered by `api_keys_resolve_direct_values_and_exact_environment_references_deterministically` |
| Invalid files have the specified asymmetric behavior | `explicit_reload_publishes_valid_edits_and_fails_closed_as_one_snapshot` makes authentication unavailable; `explicit_reload_is_transactional_and_preserves_old_immutable_snapshots` retains the last-known-good model registry |
| Configuration and Tauri-facing results are redacted | `src-tauri/tests/agent_configuration_api.rs` serializes status, mutations, invalid-file diagnostics, and login progress and proves synthetic secret/account values are absent; `SecretApiKeyInput` is deserialize-only and non-debuggable |
| Settings UI does not retain or render credential/error payloads | `npm run test:agent-settings-ui` checks password input, clearing before submission, code-based errors/diagnostics, and absence of raw backend message rendering |
| In-flight Codex turns pin configuration while later turns observe reloads | `request_generation_is_pinned_in_flight_and_reloaded_for_the_next_turn`; the remaining OpenAI Codex and Agent Conversation suites preserve login, refresh, streaming, and conversation behavior |

Focused verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::auth::tests
cargo test --manifest-path src-tauri/Cargo.toml agent::openai_codex --no-fail-fast
cargo test --manifest-path src-tauri/Cargo.toml --test agent_conversation
cargo test --manifest-path src-tauri/Cargo.toml --test agent_configuration_api --test agent_data_root --test agent_model_registry
cargo test --manifest-path src-tauri/Cargo.toml --features agent-debug --bin agent-debug
npm run test:agent-settings-ui
```

## Limits

Pattern scanning materially reduces accidental commits but cannot prove that every arbitrary opaque string is harmless. It intentionally favors high-confidence findings to remain usable across the full repository. Known binary assets are path-checked but not content-decoded; ignored files, commit metadata, and Git history are outside its snapshot. Reviewers should still inspect staged changes and may run an independent secret scanner. The safeguard never replaces protected external storage, redacted interfaces, or synthetic-only tests.

The document formats, injectable agents-data root, immutable snapshots, and opener/progress traits keep OS-specific integration behind explicit boundaries. Current-user root discovery and the live/permission acceptance recorded for #217 are macOS-only. This evidence makes no Windows or Linux runtime, installer, or permission claim.
