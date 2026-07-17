# OpenAI Codex conversation streaming

Issue [#187](https://github.com/timjonaswechler/job-radar2/issues/187) implements the explicit-SSE OpenAI Codex `ConversationProvider` behind the provider-neutral Agent Conversation interface. Request and event behavior is derived from the MIT-licensed Pi baseline pinned in [`docs/research/pi-rust-agent-baseline.md`](../research/pi-rust-agent-baseline.md) at commit `dcfe36c79702ec240b146c45f167ab75ecddd205` (copyright © 2025 Mario Zechner).

## Implemented slice

- `OpenAiCodexProvider` captures one immutable `ModelRegistry` request generation per turn, then resolves the protected OAuth credential through generic authentication immediately before transport; credentials never cross `ConversationProvider`. A registry reload affects later turns while an in-flight turn keeps its captured generation.
- Requests derive the `/codex/responses` path, model, configured non-reserved headers, and reasoning effort from that generation while preserving `store:false`, streaming input, low text verbosity, encrypted-reasoning inclusion, bounded session/request identifiers, and Pi-compatible headers. The Codex adapter restricts credential-bearing requests to the pinned `https://chatgpt.com` origin; registry overrides may change only the path beneath that origin.
- Completed User and Assistant history is converted to Responses input. Opaque terminal output metadata is kept private and replayed on later turns so encrypted reasoning and provider item identifiers remain available without exposing them to callers.
- The bounded incremental SSE decoder supports arbitrary transport chunk sizes, LF and CRLF framing, multiline `data` fields, and the final `[DONE]` marker. Bounds apply to each current SSE line/event rather than the aggregate transport chunk. It translates output text, refusal text in wire order, and provider-approved reasoning into app-owned indexed lifecycle events.
- Completed and incomplete responses map to the accepted finish reasons and token-usage shape. HTTP, stream, authentication, model, rate-limit, configuration, and transport failures map to fixed redacted `AgentError` values.
- The adapter never retries, including after output starts. WebSockets, tools, live discovery, persistence, and live-account probes remain out of scope.

## Credential containment

Credential-bearing request fields and account routing data exist only inside private transport structures that implement neither `Debug` nor a public accessor. Synthetic tests inspect only boolean credential/header matches and safe request-body data; they do not snapshot credential-bearing headers. Raw provider bodies and external error text are parsed only for category selection and are never included in diagnostics.

## Verification

The internal synthetic byte-stream adapter is the accepted transport test seam. Tests drive the production provider through `AgentConversation` where practical and cover request semantics, per-turn authentication, multi-turn opaque replay including mixed output-text/refusal content, model/reasoning changes, normal and incomplete completion, usage, split/multiline/CRLF SSE, large transport chunks with individually bounded events, oversized-event rejection, provider and transport failures, missing/malformed terminals, bounded rate-limit delays, no retry after output, and redaction.

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::openai_codex --no-fail-fast
cargo test --manifest-path src-tauri/Cargo.toml
```
