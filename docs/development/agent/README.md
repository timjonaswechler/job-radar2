# Agent development

The internal Rust Agent implementation is intentionally separated into stable contracts:

- [`conversation-core.md`](conversation-core.md) — provider-neutral, ephemeral conversation lifecycle.
- [`auth-storage.md`](auth-storage.md) — provider-neutral credential storage and reload semantics.
- [`credential-containment.md`](credential-containment.md) — repository safeguard and executable security evidence.
- [`openai-codex-authentication.md`](openai-codex-authentication.md) — provider-specific authentication and model selection.
- [`openai-codex-streaming.md`](openai-codex-streaming.md) — provider-specific request, SSE, replay, and redaction contract.
- [`debug-harness.md`](debug-harness.md) — feature-gated development harness.

Upstream behavior studies and proposed future capabilities are kept under [`research/`](../../../research/) because they are non-normative until adopted by an ADR or PRD.
