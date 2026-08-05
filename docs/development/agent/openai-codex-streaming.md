# OpenAI Codex conversation streaming

This document describes the explicit-SSE OpenAI Codex `ConversationProvider` behind the provider-neutral Agent Conversation interface. Request and event behavior is derived from the MIT-licensed Pi baseline pinned in [`docs/development/research/pi-rust-agent-baseline.md`](../research/pi-rust-agent-baseline.md) at commit `dcfe36c79702ec240b146c45f167ab75ecddd205`.

## Contract

- `OpenAiCodexProvider` captures one immutable `ModelRegistry` generation per turn, then resolves protected OAuth immediately before transport. Reloads affect later turns while an in-flight turn keeps its captured generation.
- Requests derive the `/codex/responses` path, model, configured non-reserved headers, and reasoning effort from that generation while preserving `store:false`, streaming input, low text verbosity, encrypted-reasoning inclusion, and bounded session/request identifiers.
- Credential-bearing requests are restricted to the pinned `https://chatgpt.com` origin. Registry overrides may change only the path below that origin.
- Completed history is converted to Responses input. Private opaque terminal metadata is replayed later so encrypted reasoning and provider item identifiers remain available without exposing them to callers.
- The bounded SSE decoder supports arbitrary chunk boundaries, LF/CRLF framing, multiline `data` fields, and `[DONE]`. Bounds apply to each current line/event rather than the aggregate transport chunk.
- Output text, refusal text in provider wire order, and provider-approved reasoning become app-owned indexed lifecycle events. Completed and incomplete responses map to the accepted finish reasons and token-usage shape.
- HTTP, stream, authentication, model, rate-limit, configuration, and transport failures become fixed redacted `AgentError` values.
- The adapter never retries, including after output starts. WebSockets, tools, live discovery, persistence, and live-account probes remain out of scope.

## Credential containment

Credential-bearing fields and account-routing data stay inside private transport structures with no public accessor or `Debug` implementation. Synthetic tests inspect boolean credential/header matches and safe request data only. Raw provider bodies and external errors are used only for category selection and never included in Diagnostics.

## Verification

The synthetic byte-stream adapter drives production provider behavior through `Conversation` where practical. Coverage includes request semantics, authentication, opaque replay, model/reasoning changes, completion, usage, SSE framing and bounds, provider/transport failures, malformed terminals, bounded rate-limit delays, no retry after output, and redaction.

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::openai_codex --no-fail-fast
```
