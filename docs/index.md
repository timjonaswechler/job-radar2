# Job Radar documentation

This directory contains maintained documentation for Job Radar. Start with the repository [`README.md`](../README.md) for the product overview and local setup.

## Product and architecture

- [`prd/`](prd/) — accepted product requirements and behavioral contracts.
- [`adr/`](adr/) — accepted architecture decisions and their rationale.
- [`development/`](development/) — maintained guides for developing and validating the application.

User-facing help for Sources, Search Requests, Search Runs, and troubleshooting will be added here as those product surfaces stabilize. Documentation intended for the app or website must be understandable without repository or issue history.

## Tool-owned agent configuration

[`agents/`](agents/) is a deliberate exception to the documentation taxonomy. The three Markdown files in that directory are configuration generated for `/setup-matt-pocock-skills` and are consumed by repository engineering skills. Do not move them without changing that integration.

## Placement rules

| Content | Location |
|---|---|
| Product overview and first setup | `README.md` |
| User and developer documentation | `docs/` |
| Product requirements | `docs/prd/` |
| Architecture decisions | `docs/adr/` |
| Engineering-skill configuration | `docs/agents/` |
| Time-bounded research | `research/` |
| Rust integration tests | `src-tauri/tests/` |
| Test fixtures and snapshots | `src-tauri/tests/fixtures/` |
| Shipped built-in Source Profiles | `src-tauri/resources/profiles/` |
| Executable maintenance tools | `scripts/` |
| Temporary implementation transfers | `handoff/` |

Completed issue reports, verification transcripts, generated catalogues, and temporary handoffs are not documentation. Keep durable decisions in PRDs or ADRs and executable evidence in tests; use Git and GitHub for history.
