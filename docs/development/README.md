# Development documentation

All contributor-facing, technical, and project-internal documentation lives here. General setup and commands are in the repository [`README.md`](../../README.md); repository-specific agent rules are in [`AGENTS.md`](../../AGENTS.md). Publishable end-user documentation belongs in the [User Guide](../user/README.md).

## Areas

- [`validation.md`](validation.md) — local `quick → focused → full` validation loop and Justfile command surface.
- [`search-run-smoke.md`](search-run-smoke.md) — manual, network-dependent Search Run smoke.
- [`architecture/`](architecture/) and [`adr/`](adr/) — architecture navigation and accepted Architecture Decision Records.
- [`product/`](product/) and [`prd/`](prd/) — intended product behavior and accepted Product Requirement Documents.
- [`reference/`](reference/) — stable facts intended for lookup.
- [`project/`](project/), [`agents/`](agents/), and [`research/`](research/) — internal coordination, engineering-skill configuration, and time-bounded investigations.
- [`agent/`](agent/) — Agent Conversation, authentication, credential containment, and debug-harness contracts.
- [`profile-dsl/`](profile-dsl/) — implemented Profile DSL contracts that need explanation beyond code and tests.
- [`source-live-check/`](source-live-check/) — Source Live Check implementation and freshness contracts.

A development document should describe a maintained workflow, decision, requirement, reference, or stable implemented contract. Closed-issue reports and one-time verification results belong in Git/GitHub history, not this directory.

[Back to the documentation portal](../index.md)
