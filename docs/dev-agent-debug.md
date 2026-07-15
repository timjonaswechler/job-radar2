# Agent debug harness

Issue [#188](https://github.com/timjonaswechler/job-radar2/issues/188) adds a debug-only, line-based caller of the public Rust Agent Conversation and authentication interfaces. It is a development harness, not a Tauri command or product UI.

Start a debug build on macOS:

```bash
npm run agent:debug
```

The harness uses the pinned `gpt-5.4` Agent Model with `Medium` reasoning initially. Ordinary input starts a streamed, ephemeral turn. Text and provider-approved reasoning are printed incrementally; reasoning is prefixed with `[reasoning]`.

Commands:

- `/login` — choose browser PKCE or device-code login from a numbered menu.
- `/logout` — remove the stored OpenAI Codex credential.
- `/model` — select a pinned Agent Model from a numbered menu.
- `/settings` — select a Reasoning Level supported by the current Agent Model. This changes only the in-memory Agent Conversation.
- `/quit` — exit. `Ctrl+C` retains the process default and exits the entire harness.

## Safety

The harness depends only on public `job_radar_lib::agent` interfaces. It does not inspect provider requests, headers, credential storage, replay metadata, or account data. Authentication status is only `configured` or `not configured`. Pasted browser authorization input is read directly into the non-displayable secret input type and is never written back, logged, or persisted by the harness. Caller-visible failures are rendered from stable error categories rather than external error text.

The Cargo binary requires the opt-in `agent-debug` feature and contains a release-mode compile guard. Even when that feature is explicitly supplied, a release build cannot produce a runnable harness.

## Verification

Tests use only the public deterministic `ScriptedProvider` and synthetic values:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --features agent-debug --bin agent-debug
```
