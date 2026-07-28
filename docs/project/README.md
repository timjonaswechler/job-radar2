# Project-internal work

Project-internal work surfaces support planning, investigation, coordination, and engineering agents. They are not user documentation and should not become a second home for durable product or architecture decisions.

- **GitHub Issues** are the authoritative request and planning surface. Repository workflow details are in the tool-owned [issue tracker configuration](../agents/issue-tracker.md).
- [`research/`](../../research/README.md) contains time-bounded investigations with their evidence and date-specific conclusions.
- [`handoff/`](../../handoff/README.md) contains only active temporary implementation transfers.
- [`docs/agents/`](../agents/) contains generated/tool-owned engineering-skill configuration and is an intentional documentation-layout exception. Do not move it without changing the consuming skill integration.

When work becomes durable, move the conclusion—not the whole work log—to its owning area: requirements to [Product](../product/README.md), decisions to [Architecture](../architecture/README.md), workflows to [Development](../development/README.md), or lookup material to [Reference](../reference/README.md). Completed issue reports and generated verification transcripts remain in Git and GitHub history.

[Back to the documentation portal](../index.md)
