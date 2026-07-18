# Issue #166 — Phase-2 Restructuring Handoff

Status: **local handoff for a fresh/newer session — Phase 2 preparation complete, target structure awaiting user review**  
Date: **2026-07-18**

## Goal of the next phase

Review and consolidate the proposed target ticket structure before rewriting any Lean ticket body or changing GitHub.

The next session must treat the accepted decisions D-001–D-013 as fixed, verify that the proposed 47 local target slices are sized appropriately, identify safe consolidations, and present a recommended final ticket set and direct dependency DAG to the user.

Only after explicit user approval may a later workflow rewrite Lean ticket bodies. GitHub changes remain a separate approval gate.

## Authority

Use this hierarchy:

1. Live GitHub issues, comments, labels, parent links, and native dependencies are authoritative for current original-ticket and tracker state.
2. `handoff/issue-166-contract-decisions.md` is normative for accepted decisions D-001–D-013. Do not silently reopen them.
3. `CONTEXT.md` is authoritative for current domain vocabulary, subject to the accepted activation-time migrations.
4. `docs/prd/declarative-profile-strategy-algebra.md`, #166, and applicable ADRs are the architecture baseline where not refined by D-001–D-013.
5. `handoff/issue-166-restructuring-plan.md` is the reviewed Phase-2 target proposal.
6. The three Phase-2 inventories are authoritative local evidence for current old paths, callers, variants, deletions, and scope.
7. `handoff/issue-166-original-to-lean-reconciliation.md` proves original-contract disposition.
8. `handoff/issue-166-conflict-register.md` records conflict evidence and accepted resolution ownership.
9. `handoff/issue-166-delivery.md` contains shared readiness, hard-cut, test, migration, deletion, and PR-evidence rules.
10. Existing Lean drafts contain reusable contract material, but their ticket boundaries and wording are not approved.

Live GitHub remains the sole source for original issue bodies. Do not restore deleted snapshots, archives, worker handoffs, or the old verbose template.

## Phase-2 artifacts

Created locally under gitignored `handoff/`:

- `issue-166-source-overrides-cut-inventory.md`
- `issue-166-browser-cut-inventory.md`
- `issue-166-primitive-owner-inventory.md`
- `issue-166-restructuring-plan.md`

Supporting Phase-1 artifacts:

- `issue-166-phase-1-decisions-handoff.md`
- `issue-166-contract-decisions.md`
- `issue-166-original-to-lean-reconciliation.md`
- `issue-166-conflict-register.md`
- `issue-166-delivery.md`
- `issue-166-content-deduplication-matrix.md`
- `issue-166-ticket-index.md`

## Work completed in Phase 2

### Read-only tracker refresh

At the Phase-2 baseline:

- #166 and all 27 implementation issues are open;
- every implementation issue remains a native child of #166;
- implementation issues have no comments;
- #166 has one historical decomposition comment;
- only #167 carries `ready-for-agent`;
- native dependencies still match the local ticket index.

No GitHub state was modified. Under the accepted target structure, #167 is not actually ready; correcting its label requires a separately approved tracker workflow.

### `sourceOverrides` inventory

The inventory traces the complete productive old specialization path across:

- Rust Source documents, override types, compiler implementation, exports, and diagnostics;
- four direct productive compiler callers plus indirect registry flow;
- Source Schema and frontend schema catalog;
- TypeScript DTOs and Source Create/Edit/Details UI;
- filesystem persistence, Source validation, Search Run, lazy Detail, and Source Live Check;
- old fingerprint/freshness components;
- fixtures, Rust/TypeScript tests, active domain docs, PRDs, ADRs, and agent guidance.

Conclusion: retained final compiler/schema/provenance/fingerprint foundations must precede one atomic schema-v3 Source activation. That activation owns every old-path migration and deletion. No wrapper, translator, alias, fallback, or dual productive route is allowed.

### Browser inventory

The current Browser seam has:

- three direct leaf calls (`render` in Detection and `render_with_context` in Discovery/Detail);
- six managed production-construction sites;
- eight current implementations/fakes;
- no shared scripted Browser adapter;
- callers in Detection, Discovery, Detail, Source Live Check, Search Run, posting/UI, commands, and runtime-admin smoke;
- teardown outside the current timeout, no proven forced terminate/reap, discarded cleanup failures, and incomplete usage accounting.

Conclusion: create one phase-neutral Browser Acquisition module with managed and scripted adapters, retain typed phase adapters, then perform one cross-phase Browser/Detection activation deleting the complete old seam. T14d remains guard-only.

Lifecycle clarification: inability to establish bounded process termination/reap is typed infrastructure failure. Safely quarantined bounded filesystem residue may remain private evidence and does not automatically replace a successful primary result.

### Primitive inventory

No global Primitive registry exists today. Behavior is distributed across schemas, Serde documents, compiled enums/raw documents, compiler switches, phase runtimes, Detection, and duplicate helpers.

Explicit families requiring owners include:

- Template;
- HTTP and Browser Fetch;
- Pagination;
- Parse;
- Select;
- Cardinality;
- Transform;
- Value;
- Predicate;
- Capture;
- Acceptance;
- final Detection Strategy discriminators.

Initial schema-v3 removes rather than registers:

- Retry ghost fields and counters;
- Serde-only script/eval/DOM/login/CAPTCHA Browser interactions;
- camelCase Transform aliases;
- unsupported `maxErrorRatio` absent new evidence;
- placeholder future variants.

The final restructuring plan clarifies registry granularity: executable nested options are fully inventoried and parity-checked, but become independent registry identities only when independently dispatched semantics justify that classification. Parent-family ownership is otherwise sufficient.

## Proposed restructuring outcome

Every current ticket has exactly one primary action:

- **13 Keep**
- **8 Split**
- **5 Move**
- **1 Merge**
- no current ticket is wholly dropped because each retains at least one accepted contract.

Retry and pacing/rate limiting are deferred without a target ticket. Obsolete compatibility, transition, first-lander, teardown-residue, Retry-placeholder, unsafe Browser, hint-promotion, and raw Candidate behavior is explicitly dropped.

The proposal currently contains **47 local target slices**. This is an upper-bound reliability decomposition, not approval to create 47 GitHub issues. The next review should identify safe consolidations only where one retained module interface, one test surface, and one deletion owner remain coherent.

## Core target chains

### Compiler and Source activation

```text
C01 → C02 → C03 → C04 → C05 → C06 → C07 → C08 → A01
```

- C01–C08 are final, non-productive foundations.
- A01 atomically activates schema-v3 Direct Source Specialization, canonical fingerprints, strict old-JSON rejection, Retry-ghost removal, caller/UI/schema/docs migration, and complete old-path deletion.

### Runtime and phase outcomes

```text
A01 → R01 → R02 → H01
P06c → O01 → O02
P07 + O01 + O02 → P11
R02 + O02 + P11 → O03
O03 → K01 / K02 / K03
O03 → S01
```

O03 is the exclusive D-003/D-010 result owner. Only Completed/Accepted carries reduced payload. The published T12b reduced-prefix-on-budget contract is intentionally replaced.

### Candidate Resolution and persistence

```text
S01 → Q01 → Q02 → Q03
Q02 → DB01 → DB02
Q03 + DB02 → DB03
```

- DB01 is a dormant final conversion/merge foundation.
- DB03 alone makes finalized conversion productive, narrows constructors, activates the atomic transaction, deletes bypasses/artifacts, and supersedes conflicting ADR 0008 clauses.

### Browser and Detection

```text
D01 → D02 / D03
R02 → B01 → B02 / B03
D02 + D03 + B02 + B03 + Q03 → A02 → G01
```

A02 targets Q03’s final Search Run caller so no old vector caller is migrated and immediately deleted.

### Global Primitive gate

```text
P01–P05 + P06a–P06c + P07–P11 + B03 + D02 + D03 + A02 → G02
```

G02 is implementation-free and owns no known cleanup.

## Adversarial review result

The final correction review passed:

- all 27 current tickets are represented exactly once;
- D-001–D-013 each have one owner/enforcement point;
- no known activation/deletion work is assigned to G01/G02;
- Retry removal occurs before initial schema-v3 activation;
- DB01 is non-productive and DB03 owns productive narrowing;
- Pagination depends on Browser Fetch as well as HTTP Fetch;
- Detection Browser Strategies depend on the phase allowance;
- oversized Value work is split into context, lookup, and composition foundations;
- the proposed 47-node dependency graph is acyclic;
- no accepted decision was silently reopened.

## Required first reads for the next session

Read completely before proposing consolidation or final ticket boundaries:

1. `AGENTS.md`
2. `CONTEXT.md`
3. `handoff/issue-166-phase-2-restructuring-handoff.md`
4. `handoff/issue-166-phase-1-decisions-handoff.md`
5. `handoff/issue-166-contract-decisions.md`
6. `handoff/issue-166-restructuring-plan.md`
7. all three Phase-2 inventories
8. `handoff/issue-166-original-to-lean-reconciliation.md`
9. `handoff/issue-166-conflict-register.md`
10. `handoff/issue-166-delivery.md`
11. `handoff/issue-166-ticket-index.md`
12. every current Lean draft
13. `docs/prd/declarative-profile-strategy-algebra.md`
14. ADRs 0001, 0008, 0009, and 0010

Then refresh all live issue bodies/comments and native metadata read-only before any tracker proposal.

## Next-phase tasks

### 1. Review target granularity

For every proposed target slice, verify:

- one retained observable outcome;
- one canonical owner/module interface;
- one practical test surface;
- one same-slice deletion responsibility;
- no productive dual route or compatibility layer;
- no dependency on a contract its blockers do not produce;
- scope suitable for reliable agent implementation.

### 2. Propose safe consolidation

Review all 47 local slices and identify only evidence-backed merges. Do not merge merely to reduce issue count. Reject a merge when it:

- combines independent modules or adapters;
- recreates an XL non-atomic ticket;
- couples unrelated migrations;
- makes one ticket own several independently orderable blockers;
- obscures a hard-cut deletion owner;
- introduces work that the next ticket discards.

### 3. Produce the recommended final ticket set

Present to the user:

- final local ticket labels and outcome titles;
- Current-to-Final mapping for all 27 issues;
- Keep/Merge/Split/Move/Defer/Drop decisions;
- direct blocker list with consumed-interface justification;
- activation and deletion owners;
- scope/risk estimate;
- documentation owner per behavior slice;
- remaining implementation-readiness uncertainties.

### 4. Stop for approval

Do not yet:

- rewrite Lean ticket bodies;
- create a new template;
- edit, relabel, close, create, or change dependencies of GitHub issues;
- implement product code.

After explicit approval, the following phase may generate the final Lean ticket bodies from the approved structure and re-run original-contract coverage before any GitHub publication workflow.

## Non-negotiable constraints

- D-001–D-013 remain accepted and must not be reopened silently.
- No provider-/ATS-/host-/company-/Source-key-/Profile-key-specific Rust execution branch.
- Search Request criteria remain outside Source Config, Source Profiles, Access Paths, and Source specialization.
- Runtime consumes immutable typed plans only.
- Cancellation is typed control flow, not Resolution Partial Completion, and releases no Resolution automatically.
- Source-local Posting Occurrence identity remains distinct from cross-Source Job Posting deduplication.
- Hints never become canonical persisted values.
- Budgets are safety ceilings, not Bot-Detection or Prompt-Injection prevention.
- Retry/Pacing has no initial target capability.
- No compatibility wrappers, translators, aliases, fallback, or dual productive routes.
- Known migration/deletion work belongs to the activating/replacing slice, never a later guard.
- Proposed modules remain deepening candidates until implementation evidence satisfies acceptance criteria.

## Scope risks to review

1. **A01 is necessarily XL and atomic.** Foundations must leave only wiring, authored-surface switch, fixture/docs migration, and deletion.
2. **A02 is necessarily XL and atomic.** Browser foundations must leave only final caller migration, parity, and deletion.
3. **B02 is feasibility-sensitive.** The pinned Browser stack must prove bounded forced terminate/reap before A02 becomes ready.
4. **Primitive identities remain readiness-sensitive.** Template, `text`, nested options, named captures, and schema/Serde mismatches must be frozen by their owner.
5. **A02 and DB03 overlap Search Run files without a semantic dependency.** Schedule serially or coordinate merge baselines; do not invent a false graph edge.
6. **External app-data Sources cannot be inventoried.** D-001 requires strict rejection/manual recreation, not migration.
7. **The repository contains unrelated pre-existing staged changes.** Do not modify, unstage, discard, or attribute them to #166.

## Completion criteria for the next phase

The next phase is complete only when:

- the user has reviewed the proposed 47-slice upper bound;
- every accepted consolidation preserves owner/interface/test/deletion clarity;
- a recommended final ticket count and set exists;
- every current ticket maps to the final set;
- every D-001–D-013 decision still has exactly one owner and enforcement point;
- the final DAG is acyclic and contains only real direct blockers;
- A01/A02/Q03/DB03 activation boundaries remain explicit;
- no Lean or GitHub ticket has been modified;
- residual uncertainties are explicit;
- the user receives the final structure for approval before ticket-body rewriting.

## Fresh-session starter prompt

> Lies `handoff/issue-166-phase-2-restructuring-handoff.md` vollständig. Lies anschließend `handoff/issue-166-contract-decisions.md`, die drei Phase-2-Inventare und `handoff/issue-166-restructuring-plan.md` vollständig. Behandle D-001 bis D-013 als akzeptiert und öffne sie nicht stillschweigend erneut. Aktualisiere zunächst den Live-GitHub-State read-only. Reviewe dann die vorgeschlagenen 47 lokalen Zielslices auf sichere Konsolidierungen und erstelle eine empfohlene endgültige Ticketstruktur mit Current-to-Final-Mapping, direkten Dependencies, Aktivierungs-/Lösch-Ownern, Scope-Risiken und verbleibenden Unsicherheiten. Implementiere nichts, ändere keine Lean Tickets und keine GitHub-Issues. Stoppe vor Ticket-Body- oder Tracker-Änderungen und lege mir die endgültige Struktur zuerst zur Freigabe vor.
