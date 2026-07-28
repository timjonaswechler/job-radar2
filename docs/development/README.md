# Development documentation

Maintained developer guides live here. General setup and commands are in the repository [`README.md`](../../README.md); repository-specific agent rules are in [`AGENTS.md`](../../AGENTS.md).

## Guides

- [`validation.md`](validation.md) — local `quick → focused → full` validation loop and Justfile command surface.
- [`search-run-smoke.md`](search-run-smoke.md) — manual, network-dependent Search Run smoke.
- [`agent/`](agent/) — Agent Conversation, authentication, credential containment, and debug-harness contracts.
- [`profile-dsl/`](profile-dsl/) — implemented Profile DSL contracts that need explanation beyond code and tests.
- [`source-live-check/`](source-live-check/) — Source Live Check implementation and freshness contracts.

A development document should describe a maintained workflow or stable implemented contract. Closed-issue reports and one-time verification results belong in Git/GitHub history, not this directory.
