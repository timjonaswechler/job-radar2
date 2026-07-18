# Issue #166 — Phase-1 Decisions Handoff

Status: **local handoff for a fresh session — Phase 1 complete, no GitHub changes approved**  
Date: **2026-07-18**

## Goal of the next session

Continue with **Phase 2: restructuring preparation**.

Do not repair the existing lean tickets one by one yet. First produce the inventories and target structure needed to decide which current tickets should be kept, merged, split, moved, deferred, or dropped. Then present the proposed target ticket map and dependency DAG for review before rewriting lean ticket bodies or changing GitHub.

## What Phase 1 accomplished

Phase 1 reconciled all 27 live GitHub implementation issues with their unpublished lean drafts, registered all cross-series conflicts, and resolved every open product/architecture decision with the user.

Created local artifacts:

- `handoff/issue-166-original-to-lean-reconciliation.md`
- `handoff/issue-166-conflict-register.md`
- `handoff/issue-166-contract-decisions.md`

Phase-1 outcome:

- all 27 original/lean pairs have a reconciliation verdict;
- semantic changes and the former T12b contract loss are explicit;
- all formerly open conflicts now have accepted decisions;
- 7 remaining items are documentation-only cleanup;
- no product code or GitHub issue/dependency state was changed;
- no ticket merge, split, move, deferral, drop, or final dependency change has yet been approved.

`handoff/` is gitignored and intentionally contains local planning artifacts.

## Authority for Phase 2

Use this hierarchy:

1. Live GitHub issues, comments, parent links, labels, and native dependencies remain authoritative for the **original ticket bodies and tracker state**.
2. `handoff/issue-166-contract-decisions.md` contains the **user-accepted Phase-1 replacement and boundary decisions D-001–D-013**. Apply them during restructuring; do not silently reopen them.
3. `CONTEXT.md` is authoritative for current domain vocabulary, subject to documentation migrations explicitly accepted in D-001/D-013.
4. `docs/prd/declarative-profile-strategy-algebra.md`, #166, and applicable ADRs contain the accepted architecture baseline, as refined by D-002/D-012 and D-013 where current wording is stale.
5. `handoff/issue-166-original-to-lean-reconciliation.md` proves where original contracts went and identifies semantic changes.
6. `handoff/issue-166-conflict-register.md` records conflict evidence, ownership, deadlines, and resolution status.
7. `handoff/issue-166-delivery.md` contains shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.
8. `handoff/issue-166-lean-tickets/` contains unpublished drafts and reusable contract material, but current ticket boundaries are not accepted architecture.
9. `handoff/issue-166-ticket-index.md` is navigation only.

Do not restore or expect:

- `handoff/issue-166-final-tickets/`
- `handoff/archive/`
- `handoff/issue-166-lean-ticket-worker-handoff.md`
- the deleted old verbose ticket template

Live GitHub is the sole original-ticket source.

## Required first reads

Read completely before restructuring analysis:

- `AGENTS.md`
- `CONTEXT.md`
- `handoff/issue-166-phase-2-preparation-handoff.md`
- `handoff/issue-166-original-to-lean-reconciliation.md`
- `handoff/issue-166-conflict-register.md`
- `handoff/issue-166-contract-decisions.md`
- `handoff/issue-166-delivery.md`
- `handoff/issue-166-content-deduplication-matrix.md`
- `handoff/issue-166-ticket-index.md`
- every file under `handoff/issue-166-lean-tickets/`
- `docs/prd/declarative-profile-strategy-algebra.md`
- `docs/adr/0001-source-config-as-json-schema.md`
- `docs/adr/0008-persist-job-postings-as-work-items.md`
- `docs/adr/0009-declarative-source-profile-dsl.md`
- `docs/adr/0010-source-live-checks-as-operational-confidence.md`

Then refresh #166 and all 27 implementation issues from GitHub read-only, including comments and native dependency metadata. Compare current tracker state with the Phase-1 baseline; do not edit GitHub.

## Accepted Phase-1 decisions

The full normative wording is in `handoff/issue-166-contract-decisions.md`. This section is navigation, not a replacement for reading it.

### D-001 — Clean Direct Source Specialization hard replacement

- Direct Source Specialization is the sole target model.
- No compatibility wrapper, old-to-new translator, alias, fallback, or dual productive path.
- Final foundation modules may land before activation but remain non-authorable/non-productive.
- One atomic activation migrates all callers and deletes the complete `sourceOverrides` cross-stack surface.

### D-002 — Budgets are safety ceilings

- Request/page/byte/action/duration/candidate limits are termination/resource-containment ceilings.
- They are not Bot-Detection or Prompt-Injection prevention and not target traffic patterns.
- Pacing/rate limits require a separate evidence-backed generic capability.
- Anti-Bot evasion, CAPTCHA bypass, and fingerprint manipulation remain excluded.

### D-003 — One closed phase outcome and commit boundary

- Only `Completed::Accepted` exposes reduced phase payload.
- `PolicyUnsatisfied`, `BudgetExhausted`, `ExecutionFailed`, and Cancellation expose no phase payload.
- T12b owns the shared phase outcome/result migration.
- T13 tickets reuse it and have no first-lander ownership rule.
- Reducers are pure; typed Cancellation is checked before envelope commit.
- This intentionally replaces the published T12b reduced-prefix-on-budget contract.

### D-004 — Policy before provenance

- The mandatory authored/compiled Policy lands before complete Effective Profile provenance.
- D-011 refines the order to `T5 → T6 → T4a` responsibilities.
- T4a creates the final provenance representation once, including Policy terminals and final internal phase names.

### D-005 — Explicit Primitive-family ownership and global gate

- Every admitted authored Primitive variant has exactly one canonical implementation owner.
- T11a/T11b/T11c remain focused on Parse/Select/Value.
- Remaining families need explicit owners and same-slice duplicate deletion.
- One implementation-free global registry-completeness gate blocks #166 completion.
- Consumers depend only on families they actually use.

### D-006 — Incremental reconciled Detection state

- Every URL/HTTP/Browser Strategy emits ordered contributions before information can be overwritten.
- One conflict-safe reducer produces immutable `ReconciledDetectionState` snapshots.
- Dependent Strategies read only reconciled state.
- The same Source Config validator supports incremental available-value validation and final complete validation.
- No translation of the lossy current browser aggregate.

### D-007 — Shared Browser Acquisition and cross-phase hard cut

- One phase-neutral Browser Acquisition module has managed and scripted adapters.
- Detection/Discovery/Detail retain phase-specific typed adapters and limits.
- One cross-phase activation migrates all browser callers and deletes `ProfileBrowserClient`, `render*`, old fakes/exports, and old implementations without wrappers.
- Teardown residue remains private; cleanup failure is typed infrastructure failure.
- Impossible recovered-later-Strategy rows under Detection `all_required` are removed.

### D-008 — Typed Source Detail → Candidate Resolution mapping

- `SourceDetailOutcome` distinguishes Completed, BudgetExhausted, CandidateExecutionFailed, SourceExecutionFailed, and SourceMismatch; Cancellation remains outside.
- Field dispositions exist only on Completed.
- `Unavailable` never means budget, failure, or Cancellation.
- T16 maps outcomes without reading Diagnostic text.

### D-009 — Candidate Resolution parent allowance and post-batch `remaining`

- T16 owns one private non-double-counting Resolution parent allowance.
- Child phase work is constrained through T9 caller tightening and committed once from exact child reports.
- Candidate-only dimensions debit only the parent.
- Mid-candidate budget exhaustion yields `unresolved`; emitted unstarted candidates become `budgetSkipped`.
- `remaining` is exact post-batch not-yet-emitted count or `None`.

### D-010 — Complete usage report on every started terminal

- Completed, BudgetExhausted, ExecutionFailed, and typed Cancellation carry the complete report independently from domain payload.
- Cancellation usage is transient operational evidence and is not persisted as Resolution completion.
- Usage is never reconstructed from Diagnostics or Attempt History.

### D-011 — Serial compiler foundation before activation

Use this responsibility order as restructuring input:

```text
T1 → T2 → T3a → T3b → T5 → T6 → T4a → T4b → Activation
```

The labels identify current responsibilities only. Final ticket boundaries remain open.

- T6 final names precede T4a provenance.
- T4b canonical schema-v3 fingerprint foundation precedes activation.
- Activation switches Source authoring/callers/fingerprints atomically and deletes the old specialization/freshness path.

### D-012 — Retry boundedness is conditional

- Every actually executable retry must be bounded and accounted.
- Initial #166 plans/results/schemas contain no retry dimension because no executable retry capability exists.
- Remove unconditional retry fields/counters/tests from PRD and target tickets.
- A future retry capability needs its own evidence, owner, semantics, pacing relationship, Diagnostics, budgets, and fingerprint effect.

### D-013 — Finalized-only merge and atomic persistence

- Only `SourceResolution.finalized` crosses into cross-Source dedupe and persistence.
- `FinalizedCandidate` conversion is the sole productive construction path.
- Source-local identity remains distinct from cross-Source dedupe; hints never become canonical values.
- Non-cancellation bounded Partial may persist earlier finalized values; Cancellation/Source abort releases no Resolution.
- One SQLite transaction writes Search Run, Job Postings, source occurrences, Matches, and last-run metadata.
- T17 atomically supersedes ADR 0008’s no-history/no-request-link clauses while preserving unaffected work-item decisions.
- Raw Candidate artifacts and URL-derived SCHOTT canonical expectations are removed.

## Non-negotiable constraints

- Do not implement product code during restructuring preparation.
- Do not edit, close, relabel, create, or change dependencies of GitHub issues.
- Do not treat current ticket numbers, boundaries, or order as accepted architecture.
- Do not rewrite lean ticket bodies until the target map/DAG has been reviewed.
- No provider-/ATS-/host-/company-/Source-key-/Profile-key-specific Rust execution branches.
- Search Request criteria remain outside Source Config, Source Profiles, Access Paths, and Source specialization.
- Runtime consumes immutable typed plans, never raw authored profile/source JSON.
- Cancellation remains typed control flow, not Resolution Partial Completion, and does not automatically release finalized values after abort.
- Source-local Posting Occurrence identity remains distinct from cross-Source Job Posting deduplication.
- Hints never become canonical persisted values.
- Proposed modules remain deepening candidates until implementation evidence satisfies architecture-language acceptance criteria.
- Prefer final modules and atomic replacement; no compatibility wrapper or cleanup-later ticket for known migrated surfaces.

## Phase-2 required inventories

Create these local read-only analysis artifacts first.

### 1. `sourceOverrides` hard-cut inventory

Suggested path:

`handoff/issue-166-source-overrides-cut-inventory.md`

Record every current:

- Rust document/type/compiler path;
- Source JSON Schema and DSL schema reference;
- TypeScript document type;
- create/edit/details UI path;
- Source validation and compiler caller;
- Source Live Check/fingerprint path;
- fixture and test;
- active documentation/domain/ADR reference;
- persisted/example Source document assumption;
- production caller and deletion target.

For each item state the retained final replacement owner, activation migration, and deletion proof. Do not invent a wrapper.

### 2. Browser caller/deletion inventory

Suggested path:

`handoff/issue-166-browser-cut-inventory.md`

Inventory `ProfileBrowserClient`, `render*`, process/lifecycle implementations, production/scripted adapters, exports, tests, and every caller grouped by:

- Detection;
- Discovery;
- Detail;
- Source Live Check;
- Search Run;
- posting/UI paths;
- commands;
- production adapter;
- scripted fakes and tests.

For each caller identify its final phase adapter, budget owner, output projection, parity evidence, activation owner, and deletion target.

### 3. Primitive variant/owner inventory

Suggested path:

`handoff/issue-166-primitive-owner-inventory.md`

For every admitted authored Primitive variant record:

- schema/Serde variant and authored key;
- compiled representation/registration;
- current implementation path(s);
- production consumers/phases;
- duplicate dispatch/helpers/tests;
- proposed family and canonical file owner;
- direct blockers;
- same-slice deletion targets;
- global-gate evidence.

Do not create placeholder owners or silently assign all omitted families to T11c/T14a.

## Phase-2 restructuring deliverables

After completing and reviewing the inventories, create:

### 4. Current-to-target restructuring plan

Suggested path:

`handoff/issue-166-restructuring-plan.md`

For every current ticket record:

- action: keep, merge, split, move, defer, or drop;
- reason based on accepted decisions and inventories;
- contracts retained;
- contracts moved and new canonical owner;
- work that would otherwise be implemented and later discarded;
- proposed final observable outcome;
- proposed activation/deletion responsibility;
- expected scope/size risk;
- direct prerequisites;
- affected current tickets and docs.

Apply these rules:

- merge when one ticket creates an intermediate model immediately removed by another, or when one atomic hard cut cannot be split without dual productive routes;
- split when a ticket combines independent deep modules, unrelated caller migrations, or independently orderable decisions;
- move work when its required input contract does not yet exist;
- defer speculative capabilities without evidence, especially Retry/Pacing;
- drop duplicated/editorial work whose contract has a canonical destination;
- keep one observable outcome, one canonical owner, one clear test surface, and one deletion responsibility per target ticket where practical;
- atomic hard cut does not automatically mean one oversized implementation PR: use retained final foundation slices, then one bounded activation cut.

### 5. Proposed target dependency DAG

Include in the restructuring plan or a separate artifact:

- target ticket names/IDs as local labels only;
- direct blockers based on consumed interfaces, not current numbering;
- compiler/schema-v3 activation chain from D-011;
- Primitive-family dependencies and final convergence gate from D-005;
- shared Browser foundation plus Browser/Detection activation from D-006/D-007;
- T12b shared result ownership before T13/T15;
- T15 before T16 and T16 before T17;
- documentation/ADR migrations tied to the behavior slice that makes them true.

Do not edit native GitHub dependencies yet.

## Review and validation requirements

Before presenting the restructuring proposal:

1. prove every original contract has exactly one target owner or an explicit accepted replacement;
2. prove the former T12b prefix contract is intentionally replaced by D-003;
3. prove no target ticket creates a compatibility wrapper or productive dual route;
4. prove every known old path has one activation/deletion owner;
5. prove no target ticket depends on a contract that its blockers do not produce;
6. identify target tickets too large for reliable agent execution and split only along retained final-module seams;
7. run an adversarial pass across compiler, fingerprints, runtime reports, Detection state, Browser lifecycle, Detail outcomes, Candidate counts/budgets, finalized handoff, and persistence;
8. report residual uncertainty and decisions that would require reopening D-001–D-013 rather than silently changing them.

## Pending documentation cleanup

Do not mix this cleanup into analysis artifacts unless needed for correctness. Record target owners in the restructuring plan:

- update `handoff/README.md` to remove deleted snapshot/archive/worker-handoff references;
- remove the missing old-template reference from the Phase-1 matrix;
- update PRD sample-limit wording to accepted value 10;
- apply D-012 retry wording;
- update `CONTEXT.md`/ADRs for Direct Source Specialization and schema-v3 phase vocabulary at activation;
- supersede ADR 0008 as part of T17/D-013;
- update SCHOTT smoke/artifacts with T16/T17 behavior.

Do not create a new lean ticket template until the target ticket structure has been reviewed.

## Repository-state warning

The repository had unrelated pre-existing staged/modified files during Phase 1. Do not modify, unstage, discard, or attribute those changes to #166. Establish a fresh `git status` baseline before writing local handoff artifacts.

## Completion criteria for Phase 2 restructuring preparation

Phase 2 is ready for user review only when:

- all three inventories are complete;
- every current ticket has a proposed keep/merge/split/move/defer/drop action;
- every accepted D-001–D-013 contract has a target owner and enforcement point;
- the proposed DAG has only real direct blockers;
- activation and deletion cuts are explicit and sized;
- no Lean or GitHub ticket has yet been rewritten;
- no product code has changed;
- residual risks and any required decision reopenings are explicit;
- the user receives the proposed target structure before any ticket-body rewrite.

## Fresh-session starter prompt

> Lies `handoff/issue-166-phase-1-decisions-handoff.md` vollständig und führe die dort beschriebene Phase-2-Restrukturierungsvorbereitung aus. Lies insbesondere `handoff/issue-166-contract-decisions.md` mit den akzeptierten Entscheidungen D-001 bis D-013 und öffne diese Entscheidungen nicht stillschweigend erneut. Arbeite zunächst read-only gegen Codebase und Live-GitHub-State. Erstelle zuerst die vollständigen Inventare für `sourceOverrides`, den gemeinsamen Browser-Seam und alle authorisierten Primitive-Familien. Erstelle daraus anschließend einen lokalen Current-to-Target-Restrukturierungsplan und einen vorgeschlagenen Dependency-DAG mit Keep/Merge/Split/Move/Defer/Drop-Entscheidungen. Implementiere nichts, ändere keine GitHub-Issues und schreibe die bestehenden Lean Tickets noch nicht um. Stoppe vor Ticket-Body- oder GitHub-Änderungen und lege mir zuerst Inventare, Zielstruktur, DAG, Scope-Risiken und verbleibende Unsicherheiten zur Review vor.
