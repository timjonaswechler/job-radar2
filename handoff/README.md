# Handoff inventory

`handoff/` is reserved for temporary implementation-transfer documents. Live GitHub remains authoritative for issue state, parent links, dependencies, labels, and readiness.

## Canonical sources

1. `CONTEXT.md` — domain vocabulary.
2. `docs/development/prd/` and `docs/development/adr/` — accepted product and architecture decisions.
3. `AGENTS.md` and `docs/development/agents/` — repository and tracker workflow.
4. GitHub Issues — current scope, dependencies, readiness, and completion state.

## Lifecycle

Add a handoff document only while work is actively being transferred or resumed. A handoff may summarize current state and next steps, but it does not replace canonical domain, architecture, or tracker documentation.

After the transferred work is completed and its durable decisions are recorded in the canonical sources, remove the obsolete handoff material instead of retaining it as historical authority.
