# Handoff inventory

`handoff/` contains temporary planning and transfer artifacts. It is not the canonical source for GitHub tracker state or accepted architecture.

## Canonical sources

1. Live GitHub issues and native dependencies — current tracker state.
2. `CONTEXT.md` — domain vocabulary.
3. `docs/prd/` and `docs/adr/` — accepted product and architecture decisions.
4. `AGENTS.md` and `docs/agents/` — repository and tracker workflow.

A handoff file is useful only while it supports an active review, planning, or implementation transfer that is not already represented canonically elsewhere.

## Active review artifacts

| File | Purpose | Status |
|---|---|---|
| `issue-166-content-deduplication-matrix.md` | Approved inventory for reducing repetition before restructuring the #166 ticket series | Keep; current work basis |
| `issue-166-delivery.md` | Approved shared delivery, testing, migration, deletion, and PR-evidence rules for lean tickets | Keep; shared Phase-2 contract |


## Published-ticket snapshots

### `issue-166-final-tickets/`

The directory is retained temporarily as Phase-2 rewrite input. It contains:

- 27 files that are byte-for-byte copies of the current published GitHub issue bodies;
- one obsolete combined draft, `T3-effective-profile-additions-and-source-config.md`, superseded by the published split T3a/T3b issues.

These files are **non-canonical snapshots**. Live GitHub remains authoritative. Lean rewrites should go into a separate `issue-166-lean-tickets/` directory so old and proposed bodies cannot be confused. Once the new bodies are approved and published, this snapshot directory can be removed.

## Historical workflow artifacts

| File | Assessment | Status |
|---|---|---|
| `archive/issue-166-ticket-draft.md` | Decision-gate/tracer-bullet draft whose decisions are incorporated into #166 and the published issues | Historical; non-canonical |
| `archive/issue-33-to-profile-strategy-algebra.md` | Operational handoff already reflected in current #33; #165 is closed as superseded by #166 | Historical; non-canonical |


## Current and next layout

```text
handoff/
├── README.md
├── issue-166-content-deduplication-matrix.md
├── issue-166-ticket-index.md
├── issue-166-delivery.md                 # shared Phase-2 contract
└── issue-166-lean-tickets/               # later Phase-2 drafts
```

## Next gate

1. Use `issue-166-lean-ticket-worker-handoff.md` for exactly one ticket per fresh-context invocation, starting with T2/#168.
2. Review each lean result against its published snapshot before continuing.
3. Produce the remaining lean ticket drafts separately from the published-body snapshots.
4. Review content and only then reconsider ticket boundaries/dependencies.
5. Remove the snapshot directory after approved bodies are published.
