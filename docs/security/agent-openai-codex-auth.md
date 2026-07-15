# OpenAI Codex authentication and model selection

Issue [#185](https://github.com/timjonaswechler/job-radar2/issues/185) implements the OpenAI ChatGPT/Codex subscription authentication and pinned model-selection slice behind the internal Rust agent module. The behavior is derived from the MIT-licensed Pi baseline pinned in [`docs/research/pi-rust-agent-baseline.md`](../research/pi-rust-agent-baseline.md) at commit `dcfe36c79702ec240b146c45f167ab75ecddd205` (copyright © 2025 Mario Zechner).

## Implemented contract

- `AgentAuthentication` provides value-free status, browser Authorization Code + PKCE login, the observed bounded device-code flow, logout, and per-request exact-expiry refresh.
- Login and refresh persist complete rotated OAuth credentials through the protected `AuthStorage` before credentials can be used.
- Browser authorization validates returned state when present. Device polling uses a monotonic 15-minute deadline, a minimum one-second interval, and bounded `slow_down` handling.
- Provider, transport, token, and storage failures are translated to stable redacted `AgentError` values. Credential-bearing request/response values never enter diagnostics.
- The static catalog exposes the seven models and Reasoning Levels from the pinned Pi snapshot. Reasoning selection follows the accepted Job Radar contract: nearest supported level, preferring the higher level on a tie. Provider-specific effort mapping remains internal.
- There is no API-key or environment-variable fallback, live model discovery, conversation streaming, REPL, or live account probe in this slice.

## Verification

Tests use the caller-facing authentication and model interfaces with injected synthetic HTTP, interaction, clock, randomness, and filesystem adapters. All JWT-shaped values and credential fields are fabricated. Coverage includes PKCE request construction, login/logout, device polling and deadline behavior, exact-expiry refresh after lock acquisition, refresh rotation and persistence, model lookup, reasoning normalization, and redacted error categories.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent:: --no-fail-fast
```
