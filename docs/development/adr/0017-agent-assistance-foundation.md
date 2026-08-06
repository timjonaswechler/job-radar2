# Agent Assistance remains an intentionally incomplete Chat foundation

Status: accepted

Agent Assistance currently means an intentionally incomplete foundation for provider-backed Agent Conversations, durable Agent Chats, and provider Configuration. It is not a completed product workflow and does not own Search, Source, Job Posting, application preparation, or any other domain context.

## Ownership

The Tauri-free `agent` crate retains three public Modules:

- `Conversation` owns one provider-neutral streamed turn and its completed transcript;
- `Chats` owns durable Agent Chat creation, opening, projection, operations, recovery, compaction markers, cancellation, event sequencing, and `NotSaved` outcomes;
- `Configuration` owns value-free provider/model capability status and credential/login configuration.

Desktop/Tauri owns only host composition and thin native Adapters: Tauri command/event transport, URL/folder opening, and progress/event sinks. The debug harness uses the same `Conversation` and `Configuration` Interfaces. The Agent Lab (`/labs/agent`) remains a functional development integration and manual-acceptance surface. Its prototype page, route, mock Canvas, harness, localization, and supporting dependencies are preservation targets; they are not a cleanup or LOC-reduction target and do not become boundary authority.

The private implementation hides session journals and compaction, registry loading, authentication and secure-file handling, and the single compiled OpenAI Codex Adapter. Provider transport, OAuth, callback, request, SSE, and credential-storage details do not become public host contracts.

## Capability and configuration

Configured and executable are distinct capabilities. A provider or model may remain visible because it is configured or catalogued while being unavailable for Chat execution. Only the compiled OpenAI Codex Adapter is executable in this foundation. Configuration exposes no credentials, account data, paths, or provider transport internals and publishes one coherent generation of status and model capability information.

Callers own prompts. Chats have no durable Search, Source, Posting, or other context identity, and no context binding is implied by the Agent Lab. The foundation does not attach an Agent Chat to a Job Posting or claim autonomous workflow behavior.

## Chat operation authority

The feature-local Agent Chat lifecycle Module owns request identity, the current Chat identity, generation and sequence filtering, operation identity, stale command/event rejection, listener cleanup, cancellation projection, and `NotSaved` recovery. Runtime-decoded transport values reach this lifecycle only after validation. A Chat operation has one owner and cannot be replaced by a late response from an older request, Chat, generation, selection, or login attempt. `NotSaved` is an explicit recoverable Chat projection; reload is caller-visible and productive.

Settings owns the login attempt identity, progress filtering, cancellation attribution, credential-input clearing, and value-free capability presentation. Transport decoders own validation and redacted error projection; Settings does not parse provider or authentication internals.

## Rejected and deferred alternatives

We rejected:

- splitting providers and sessions into additional crates, because the current implementation has one cohesive owner and no second productive host contract;
- reversing Desktop and `agent` ownership, because reusable conversation, Chat, and Configuration behavior must remain Tauri-free;
- a broad Agent Assistance facade or compatibility export wall, because callers should use the deliberate `Conversation`, `Chats`, and `Configuration` Interfaces;
- treating Agent Lab as a product boundary or deleting it as migration residue;
- a provider/session split that would expose storage, registry, or transport mechanics merely to create hypothetical reuse.

The following remain explicitly deferred: session retention policy and browsing/pagination, context identity and domain attachment, additional providers, tools, retrieval, autonomous workflows, server/Extension hosting, Platform-wide abstractions, and any Chat browser, naming, or Trash product UI.

ADR 0011 remains the decision for the minimal ephemeral `Conversation` contract. This ADR extends its terminology and records the later persistent `Chats` and Configuration ownership; it does not rewrite the Conversation contract as a persistent Chat contract.
