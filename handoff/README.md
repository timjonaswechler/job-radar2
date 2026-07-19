# Handoff inventory

`handoff/` now contains only the active implementation-transfer contract and the compact publication record for Issue #166. Live GitHub remains authoritative for issue state, native parent links, dependencies, labels and readiness.

## Canonical sources

1. [GitHub Issue #166](https://github.com/timjonaswechler/job-radar2/issues/166) — Wayfinder Map and canonical architecture specification.
2. GitHub Issues [#235–#276](https://github.com/timjonaswechler/job-radar2/issues/235) — 42 final `wayfinder:task` implementation tickets.
3. `CONTEXT.md` — domain vocabulary.
4. `docs/prd/` and `docs/adr/` — accepted product and architecture decisions.
5. `AGENTS.md` and `docs/agents/` — repository and tracker workflow.

## Active implementation-transfer files

| File | Purpose | Retention |
|---|---|---|
| `issue-166-delivery.md` | Shared readiness, testing, migration, deletion and PR-evidence contract referenced by all 42 final tickets | Keep while any #166 implementation ticket remains active |
| `issue-166-contract-decisions.md` | Definitions and rationale for accepted D-001–D-013 contracts referenced by the final tickets | Keep while any #166 implementation ticket remains active |

Neither file replaces live GitHub dependencies or readiness metadata.

## Publication record

`issue-166-phase-5-working/` retains only the compact verified publication evidence:

- `phase-5-final-report.md` — complete 42-issue map, 86 dependency edges and 27 predecessor dispositions;
- `wayfinder-migration-report.md` — verified Wayfinder labels and map structure;
- `final-live-snapshot.json` — redacted deterministic final tracker snapshot;
- `publication-journal.json` and `wayfinder-migration-journal.json` — resumable mutation histories retained until this repository state is safely committed.

The Phase-5 publication and Wayfinder migration are complete. No implementation readiness was granted by publication.

## Removed historical material

Lean drafts, Phase-1–4 orchestration/review artifacts, frozen publication-source copies, raw GitHub baselines, helper scripts, caches, inventories and superseded planning documents were removed after successful live validation. They must not be restored as implementation authority.

## Next gate

Before assigning any final ticket, apply `issue-166-delivery.md`: verify direct blockers live, re-inspect current code and tests, re-baseline provisional paths and names, confirm that no in-scope decision remains unresolved, and only then review `ready-for-agent`.
