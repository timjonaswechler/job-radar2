# Job Radar documentation portal

This is the central entry point for maintained Job Radar documentation. Choose the route that matches why you are here:

- **Using Job Radar:** follow the [User Guide](user-guide/README.md) from local startup through Source setup, Search Request creation, the first complete Search Run, and troubleshooting.
- **Building or changing Job Radar:** start with [Development](development/README.md), then consult [Architecture](architecture/README.md), [Product](product/README.md), and [Reference](reference/README.md) as needed.

The repository [README](../README.md) remains the product overview and local setup entry point.

## Documentation areas

| Area | Purpose | Put here |
|---|---|---|
| [User Guide](user-guide/README.md) | Help people use the application successfully, starting with the first complete Search Run. | User-facing concepts, task-oriented guides, and troubleshooting. |
| [Development](development/README.md) | Help contributors build, test, debug, and operate the development environment. | Maintained engineering workflows and implemented technical contracts. |
| [Architecture](architecture/README.md) | Explain why the system is shaped as it is. | Architecture navigation and accepted Architecture Decision Records. |
| [Product](product/README.md) | Define intended product behavior and scope. | Product navigation and accepted Product Requirement Documents. |
| [Reference](reference/README.md) | Provide stable facts that readers look up rather than read sequentially. | Domain vocabulary, schemas, catalogues, command inventories, and shipped-format references. |
| [Project-internal work](project/README.md) | Locate temporary or tool-owned planning and coordination artifacts. | Issue-tracker work, time-bounded research, active handoffs, and agent configuration. |

## Where a new document belongs

Choose by the document's primary reader and purpose, not by the code it mentions:

1. A user completing a product task or diagnosing a product problem → **User Guide**.
2. A contributor performing a repeatable engineering workflow → **Development**.
3. A durable explanation of an architectural choice and its consequences → **Architecture**, normally as an ADR under [`adr/`](adr/).
4. Accepted behavior, scope, or product requirements → **Product**, normally as a PRD under [`prd/`](prd/).
5. Stable lookup material with little narrative sequence → **Reference**.
6. Investigation, planning, coordination, or generated/tool-owned material that is not durable product documentation → **Project-internal work** and its owning repository surface.

If a document serves several audiences, keep one canonical document in the area matching its primary purpose and link to it from the other area indexes. Do not duplicate it.

## Other repository material

Not every maintained artifact is documentation:

| Content | Owning location |
|---|---|
| Product overview and first local setup | [`README.md`](../README.md) |
| Tool-owned engineering-skill configuration | [`docs/agents/`](agents/) |
| Time-bounded investigation | [`research/`](../research/) |
| Rust integration tests | `src-tauri/tests/` |
| Test fixtures and snapshots | `src-tauri/tests/fixtures/` |
| Shipped built-in Source Profiles | `src-tauri/resources/profiles/` |
| Executable maintenance tools | [`scripts/`](../scripts/) |
| Temporary implementation transfers | [`handoff/`](../handoff/) |

Completed issue reports, generated verification transcripts, and generated catalogues are not maintained documentation. Keep durable decisions in PRDs or ADRs, executable evidence in tests, and history in Git/GitHub.

## Existing layout during migration

This portal introduces the target navigation without moving every existing document. Existing paths under [`development/`](development/), [`adr/`](adr/), [`prd/`](prd/), and [`agents/`](agents/) remain canonical and reachable. The `agents/` files are generated configuration consumed by repository engineering skills; do not move them without changing that integration. Future moves must update inbound links atomically or leave an intentional compatibility document at the old path.

Run `npm run check:markdown-links` (or `just docs-check`) after documentation changes. The dependency-free checker validates internal destinations and Markdown heading anchors across every tracked Markdown file; its self-test is `npm run test:markdown-links`.
