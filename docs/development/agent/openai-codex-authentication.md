# OpenAI Codex authentication and model selection

This document describes the OpenAI ChatGPT/Codex subscription authentication and pinned model-selection adapter behind the provider-neutral Rust Agent module. The behavior is derived from the MIT-licensed Pi baseline pinned in [`research/pi-rust-agent-baseline.md`](../../../research/pi-rust-agent-baseline.md) at commit `dcfe36c79702ec240b146c45f167ab75ecddd205`.

## Contract

- `AgentAuthentication` provides value-free status, browser Authorization Code with PKCE login, the observed bounded device-code flow, logout, and per-request exact-expiry refresh.
- Login and refresh persist complete rotated OAuth credentials through the protected `AuthStorage` before credentials can be used.
- Browser authorization validates returned state when present. Device polling uses a monotonic 15-minute deadline, a minimum one-second interval, and bounded `slow_down` handling.
- Provider, transport, token, and storage failures become stable redacted `AgentError` values. Credential-bearing request and response values never enter Diagnostics.
- The provider-neutral `ModelRegistry` publishes the pinned built-in models and applies validated `models.json` overrides. Reasoning selection chooses the nearest supported level and prefers the higher level on a tie.
- The adapter resolves credentials through the generic `AuthStorage` precedence contract. The Codex subscription transport accepts only OAuth credentials with valid `accountId` metadata and rejects API-key variants without fallback.
- Model capabilities come from the pinned catalogue. There is no live model discovery or live account probe.

## Verification

Tests use injected synthetic HTTP, interaction, clock, randomness, and filesystem adapters. Coverage includes PKCE construction, login/logout, device polling and deadlines, exact-expiry refresh after lock acquisition, rotation and persistence, model lookup, reasoning normalization, and redacted error categories.

The OAuth filesystem fixtures currently run on macOS and Linux. Native Windows persistence support and equivalent coverage are tracked by [#294](https://github.com/timjonaswechler/job-radar2/issues/294).

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent:: --no-fail-fast
```
