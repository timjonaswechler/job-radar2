# macOS agent-configuration verification

Issue [#217](https://github.com/timjonaswechler/job-radar2/issues/217) verified the provider-configuration slice on macOS on 2026-07-17. Only sanitized outcomes are recorded. The operator did not inspect or retain credential/configuration contents, environment values, authorization URLs or callbacks, account identifiers, email addresses, prompts, model responses, headers, or provider payloads.

## Live checks

| Check | Outcome |
| --- | --- |
| Tauri development app compiles, starts, and remains running without a startup panic | Passed |
| Existing debug harness starts, reports only its value-free status, and exits cleanly | Passed; captured output was discarded |
| Existing canonical agents directory is a non-symlink directory with mode `0700` | Passed |
| Existing credential/configuration/lock files covered by the check are regular non-symlink files with mode `0600` | Passed; two files checked by metadata only |
| Credential safeguard scans the Git index, tracked working tree, and non-ignored untracked files | Passed |

The earlier authenticated subscription, restart, model-selection, multi-turn conversation, and logout live checks remain recorded in [`agent-openai-subscription-macos.md`](agent-openai-subscription-macos.md). This run did not perform another login, authenticated request, credential mutation, or logout and does not replace that evidence.

## Executable evidence

The following completed successfully on the same macOS checkout:

```bash
npm run test:agent-credential-safety
npm run check:agent-credentials
npm run test:agent-settings-ui
npm run build
cargo test --manifest-path src-tauri/Cargo.toml agent::auth::tests
cargo test --manifest-path src-tauri/Cargo.toml agent::openai_codex --no-fail-fast
cargo test --manifest-path src-tauri/Cargo.toml --test agent_conversation
cargo test --manifest-path src-tauri/Cargo.toml --test agent_configuration_api --test agent_data_root --test agent_model_registry
cargo test --manifest-path src-tauri/Cargo.toml --features agent-debug --bin agent-debug
```

Together these checks cover repository-external protected storage, old-path migration and conflict handling, concurrent mutation and refresh, direct and environment-reference resolution, fail-closed authentication reload, last-known-good model reload, value-free configuration/Tauri projections, settings UI containment, and existing Codex login/conversation behavior. The detailed evidence map is in [`../security/agent-credential-containment.md`](../security/agent-credential-containment.md).

## Acceptance boundary

The formats and core contracts use an injectable agents-data root, immutable snapshots, and explicit opener/progress interfaces. Those boundaries avoid embedding the macOS location in authentication, registry, or UI callers. Current-user root discovery, filesystem-mode checks, Tauri startup, and live behavior in this report are macOS-only. No Windows or Linux permission, installer, or runtime acceptance is claimed.
