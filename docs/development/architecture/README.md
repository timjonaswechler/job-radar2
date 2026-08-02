# Architecture

Architecture documentation explains durable system structure, boundaries, and why important technical choices were made.

Repository-wide module and naming conventions are defined in [Module design and naming](module-design.md). Apply them when introducing or materially restructuring a module.

Rust-specific engineering rules for types, ownership, errors, effects, performance, and testing are defined in [Rust engineering](rust.md). Apply them to new and materially changed Rust code.

TypeScript- and React-specific engineering rules for runtime contracts, module roles, state, adapters, performance, and testing are defined in [TypeScript and React engineering](typescript-react.md). Apply them to new and materially changed frontend code.

Accepted decisions live in [`../adr/`](../adr/). Start with:

- [Source Config as JSON Schema](../adr/0001-source-config-as-json-schema.md)
- [Declarative Source Profile DSL (historical ADR title)](../adr/0009-declarative-source-profile-dsl.md)
- [Source Live Checks as operational confidence](../adr/0010-source-live-checks-as-operational-confidence.md)
- [Minimal Agent Conversation contract](../adr/0011-minimal-agent-conversation-contract.md)
- [Separate Source behavior from installed Source ownership](../adr/0013-source-engine-and-sources.md)
- [Keep Search Request catalog ownership separate from execution](../adr/0014-search-request-catalog.md)

Use an ADR for a durable architectural decision, its context, alternatives, and consequences. Intended product behavior belongs in [Product](../product/README.md); repeatable implementation workflows belong in [Development](../README.md).

[Back to the documentation portal](../../index.md)
