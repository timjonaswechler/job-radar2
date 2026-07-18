# Issue #166 — Phase-4 Batch Orchestration Playbook

Status: **Phase 4 authorized — local batch work only; GitHub excluded until separate Phase-5 approval**  
Companion to: `handoff/issue-166-phase-4-ticket-rewrite-handoff.md`  
Date: **2026-07-18**

## 1. Purpose

This playbook explains how an orchestrating agent should execute the large Phase-4 rewrite safely across multiple fresh agent contexts and resumable batches.

It does not replace the Phase-4 handoff, D-001–D-013, or the approved final ticket structure. It defines process, delegation, batch ownership, progress recording, review loops, and recovery between sessions.

The user has approved Phase 4. Work remains local and batch-scoped. GitHub is not read or modified during Phase 4.

## 2. Phase-4 outcome

Phase 4 produces locally:

1. exactly 42 final ticket bodies under `handoff/issue-166-phase-4-tickets/`;
2. `handoff/issue-166-phase-4-contract-coverage.md`;
3. `handoff/issue-166-github-migration-manifest.md`;
4. `handoff/issue-166-phase-5-publication-runbook.md`.

Phase 4 does not:

- implement product code;
- edit existing 27 Lean drafts;
- edit tracked PRD/ADR documents without separate approval;
- create, edit, close, label, parent, or change dependencies of GitHub issues;
- execute the Phase-5 migration manifest.

## 3. Primary sources and locations

The orchestrator reads only the compact core documents listed in the Phase-4 handoff completely. Fresh context builders load inventories, Reconciliation records, relevant Lean drafts, and canonical evidence per batch. GitHub is completely excluded from the Phase-4 working context. This keeps the parent context small and uses the reviewed local evidence package as intended.

Core Phase-4 sources:

| Purpose | Source |
|---|---|
| Normative decisions | `handoff/issue-166-contract-decisions.md` |
| Approved 42-ticket catalogue and 85-edge DAG | `handoff/issue-166-recommended-final-ticket-structure.md` |
| Old productive Source route | `handoff/issue-166-source-overrides-cut-inventory.md` |
| Browser seam/caller/deletion evidence | `handoff/issue-166-browser-cut-inventory.md` |
| Primitive variants/owners/completeness evidence | `handoff/issue-166-primitive-owner-inventory.md` |
| Original-contract dispositions | `handoff/issue-166-original-to-lean-reconciliation.md` |
| Conflict evidence and resolutions | `handoff/issue-166-conflict-register.md` |
| Shared implementation delivery rules | `handoff/issue-166-delivery.md` |
| Historical issue-number/navigation mapping only | `handoff/issue-166-ticket-index.md` |
| Primary reusable local ticket material | `handoff/issue-166-lean-tickets/` |
| Product/architecture baseline | `docs/prd/declarative-profile-strategy-algebra.md` |
| Domain vocabulary | `CONTEXT.md` |
| Reviewed original-contract dispositions | `handoff/issue-166-original-to-lean-reconciliation.md` |

Authority order remains the hierarchy in the Phase-4 handoff. Existing Lean ticket boundaries are not authoritative.

## 4. Working artifacts created after `GO`

Use a dedicated local working area:

```text
handoff/
├── issue-166-phase-4-progress.md
├── issue-166-phase-4-working/
│   ├── global-contract-ledger.md
│   ├── final-ticket-catalogue.yaml
│   ├── dag-validation.md
│   ├── batches/
│   │   ├── batch-01-brief.md
│   │   ├── batch-01-review.md
│   │   ├── batch-01-handoff.md
│   │   └── ...
│   └── validation/
│       ├── ticket-structure.md
│       ├── forbidden-contract-search.md
│       ├── coverage-validation.md
│       └── migration-manifest-validation.md
└── issue-166-phase-4-tickets/
    └── <42 final ticket files>
```

Working files are local coordination artifacts. The four deliverables named in Section 2 remain the final review package.

## 5. Role model

### 5.1 Orchestrator

The parent/main agent is the sole cross-series decision-maker.

Responsibilities:

- complete the compact orchestrator-core reads;
- keep GitHub entirely outside the Phase-4 context;
- create the global contract ledger and ticket catalogue from compact reviewed local evidence;
- parse and validate the approved DAG;
- choose topological batch execution order;
- create one scoped batch brief at a time;
- launch one writer for the active batch;
- launch fresh read-only reviewers and validators;
- decide which findings are accepted;
- resume the same batch writer for corrections;
- freeze completed batches and update progress;
- synthesize coverage, migration manifest, runbook, and final report;
- stop on any unapproved product/architecture/boundary decision.

The orchestrator must not ask ordinary child agents to orchestrate further subagents. Delegation remains parent-owned.

### 5.2 Context builder

Read-only role used before a batch.

Responsibilities:

- inspect the batch’s final boundaries and direct blockers;
- read only the relevant Lean drafts, inventory sections, Reconciliation/conflict records and canonical clauses completely in its isolated context;
- map D-001–D-013 and original-contract dispositions to the batch;
- extract relevant inventory rows, current callers, tests, paths, and deletion targets;
- identify exact upstream interfaces consumed;
- identify stale names that must remain provisional;
- produce a compact batch context, not a new plan or architecture.

The context builder does not write final ticket bodies.

### 5.3 Batch writer

One fresh writer owns one batch and only that batch.

Responsibilities:

- read the batch brief, normative decisions, relevant final-structure sections, direct upstream frozen ticket bodies, and scoped evidence;
- write only the assigned final ticket files;
- follow the exact final-ticket template from the Phase-4 handoff;
- preserve final boundaries and direct blockers;
- report files written, assumptions, provisional names, and stop conditions encountered.

The batch writer may not:

- modify another batch;
- change ticket boundaries or DAG edges;
- reopen D-001–D-013;
- write the coverage matrix or GitHub manifest;
- edit current Lean drafts, product code, tracked canonical docs, or GitHub;
- create compatibility routes or speculative Retry/Pacing behavior.

### 5.4 Reviewer

Fresh-context and read-only.

Responsibilities:

- compare the complete batch against its brief and source evidence;
- check one observable result and coherent module/test boundary per ticket;
- verify every blocker produces an interface actually consumed;
- verify migration/deletion ownership and no deferred known cleanup;
- report only evidence-backed findings with ticket/file references;
- distinguish blocker, major, minor/reference cleanup, and no finding.

The reviewer does not modify final files.

### 5.5 Fix writer

Use the same batch-writer session when possible. If revival is impossible, use exactly one replacement writer with the complete original brief, review findings, and frozen boundaries.

Responsibilities:

- apply only findings accepted by the orchestrator;
- avoid unrelated rewriting;
- rerun batch structural checks;
- return a correction summary.

### 5.6 Validator

Read-only structural role or deterministic script.

Responsibilities:

- count files and labels;
- verify required sections;
- parse blockers and compare them to the approved DAG;
- detect cycles or missing/extra edges;
- search forbidden obsolete contracts;
- verify ticket/issue mappings and coverage cardinalities;
- produce machine-checkable evidence.

## 6. Single-writer rule at batch scope

There is no single writer for all 42 tickets. Instead:

- exactly one writer is active for one batch;
- different batch writers run sequentially unless isolated temporary drafts are used;
- no two writers modify `handoff/issue-166-phase-4-tickets/` concurrently;
- the parent does not edit the same files while a writer is active;
- reviewers and validators remain read-only;
- corrections return to one writer;
- a frozen batch changes later only through a recorded cross-series finding and one explicit fix pass.

Writers may create non-overlapping temporary draft files outside the final directory, but only the active batch writer or orchestrator-approved fix writer promotes content into final batch files.

## 7. Proposed batch partition

Batch membership is fixed by this playbook unless source evidence after `GO` exposes a direct contradiction with the approved DAG. Execution order is derived topologically from the Phase-3 structure; table order below is organizational, not authority over dependencies.

| Batch | Assigned final labels | Size |
|---|---|---:|
| B01 | C01, C02, C04, C05, C06 | 5 |
| B02 | R01, R02, H01 | 3 |
| B03 | P01, P02, P03, P04, P05 | 5 |
| B04 | P06a, P06bc, P07, P08 | 4 |
| B05 | P09, P10, P11 | 3 |
| B06 | O01, O02, O03 | 3 |
| B07 | K01, K02, K03, S01 | 4 |
| B08 | B01, B02, B03a, B03b | 4 |
| B09 | D01, D02, D03 | 3 |
| B10 | Q01, DB01 | 2 |
| B11 | C07, C08 | 2 |
| B12 | A01 | 1 large activation |
| B13 | A02 | 1 large activation |
| B14 | A03 | 1 large activation |
| B15 | G02 | 1 global gate |

Total: **42 final tickets**.

Important:

- A01, A02, and A03 each receive an isolated batch because their productive migration/deletion surfaces are large.
- G02 remains implementation-free and owns no known migration/deletion.
- Actual scheduling must respect every approved direct semantic blocker.
- File overlap between A02 and A03 may motivate scheduling A03 first, but never a fake dependency.

## 8. Global contract ledger

Before drafting, the orchestrator creates `global-contract-ledger.md` from the decisions/final structure plus compact domain reports produced by isolated context builders. The orchestrator does not ingest every full source document. The ledger has one row per contract/decision/inventory responsibility.

Minimum columns:

```text
contract ID/source
→ normative statement
→ final owner label
→ direct consumers
→ canonical document owner
→ inventory/deletion rows
→ disposition: retained/replaced/deferred/removed
→ validation evidence
```

Required entries include:

- D-001–D-013;
- all 27 original ticket-specific contract groups;
- former T12b prefix output replaced by D-003/O03;
- Retry/Pacing deferred without fields/counters;
- safety ceilings distinguished from traffic/Bot-Detection behavior;
- every Source Overrides inventory row;
- every Browser inventory row;
- every Primitive family/variant/duplicate/mismatch;
- A01/A02/A03 activation and deletion ownership;
- all canonical-document migrations.

The ledger is the compact cross-session memory. Writers consume scoped excerpts rather than reconstructing the full history.

## 9. Batch brief contract

Every `batch-NN-brief.md` must contain:

```md
# Batch NN — <domain>

## Assigned files
- label → exact filename → title

## Fixed boundaries
- one result per ticket
- responsibilities explicitly excluded

## Direct blockers
- label → produced interface → consuming ticket

## Normative decisions
- exact D-IDs and relevant PRD/ADR clauses

## Original-contract coverage
- relevant Lean sections and reviewed Reconciliation dispositions reused or replaced

## Inventory ownership
- exact rows/callers/paths/deletions owned here

## Upstream frozen interfaces
- exact final ticket references

## Required corrections
- Phase-4 handoff Section 9 items applicable to this batch

## Forbidden drift
- stop conditions and realistic adjacent non-goals

## Validation
- required sections, searches, structural checks, review angles

## Expected output
- assigned final files only
```

Do not give the writer the entire repository history when a scoped evidence set is sufficient. Do include every authoritative fact needed to avoid invention.

## 10. Per-batch lifecycle

Each batch passes these states:

```text
pending
→ context-ready
→ drafting
→ drafted
→ reviewing
→ corrections-required | review-clean
→ correcting
→ validated
→ frozen

Any non-final state may transition to:

blocked-awaiting-user
```

### Step 1 — Context

- Create batch brief.
- Validate assigned labels and blockers against catalogue/DAG.
- Confirm relevant upstream batches are frozen or that drafting can safely use approved fixed interfaces.

### Step 2 — Draft

- Launch one writer.
- Writer creates only assigned files.
- Writer reports provisional paths/names and any stop condition.

### Step 3 — Deterministic structure check

Check:

- expected file count and filenames;
- one label/title/result per file;
- all required sections exactly once;
- direct blockers present and valid;
- shared delivery referenced, not copied;
- no forbidden placeholders/obsolete clauses.

### Step 4 — Fresh review

Use at least two angles when the batch is large or activation-heavy:

1. contract/behavior/dependency correctness;
2. migration/deletion/test/documentation completeness.

Small batches may use one reviewer covering both angles.

### Step 5 — Synthesis

The orchestrator classifies findings:

- fix now;
- already satisfied/false positive;
- cross-series follow-up;
- unapproved decision → stop and ask user.

### Step 6 — Correction

Resume the same writer with accepted findings. Run no parallel writer on the batch.

### Step 7 — Freeze

Create `batch-NN-handoff.md` containing:

- frozen files;
- interfaces produced;
- direct consumers;
- exact decisions enforced;
- inventory/deletion ownership;
- provisional implementation-readiness names;
- canonical docs assigned for later implementation updates;
- residual risks or `none`;
- review/validation evidence.

Update the progress table.

## 11. Progress and resumability

`handoff/issue-166-phase-4-progress.md` is the first place a resumed orchestrator reads after the main Phase-4 handoff and this playbook.

Suggested format:

```md
# Phase-4 progress

Status: in progress
Local repository baseline: <commit and pre-existing dirty-state note>
GitHub access in Phase 4: prohibited/not performed

| Batch | Labels | State | Writer session/artifact | Review | Frozen handoff | Residual risk |
|---|---|---|---|---|---|---|
| B01 | C01…C06 | frozen | ... | clean | ... | none |
| B02 | R01…H01 | drafting | ... | pending | — | — |

## Next action
<one exact next step>

## Stop conditions awaiting user
- none

## Global validation status
- DAG: pending/passed
- ticket count: n/42
- decisions covered: n/13
- old local issue responsibilities mapped for future supersession: n/27
```

Never rely only on conversation history or child-session availability. All durable progress necessary for another orchestrator must exist in local artifacts.

## 12. Delegation procedure

Before launching any child:

1. inspect the available agent list;
2. choose only executable, non-disabled roles;
3. give a concrete goal, evidence paths, success criteria, hard constraints, validation, output path, and stop rules;
4. use fresh context for builders/reviewers;
5. use one writer at a time;
6. write child outputs to unique paths;
7. keep final synthesis and decision authority with the orchestrator.

Prefer asynchronous execution when the parent has independent validation or context work. When no independent work remains and the result is needed, wait for completion rather than abandoning the run.

Ordinary children must not delegate further. Only the parent orchestrates.

## 13. Batch writer prompt template

```text
Goal: Write the final local Phase-4 ticket bodies for batch {batch} only.

Read completely:
- {batch brief}
- handoff/issue-166-contract-decisions.md
- relevant sections of handoff/issue-166-recommended-final-ticket-structure.md
- named upstream frozen ticket bodies
- named inventory/reconciliation/Lean evidence in the brief
- handoff/issue-166-delivery.md

Output:
- exactly these files under handoff/issue-166-phase-4-tickets/: {files}

Success criteria:
- one observable Result and approved boundary per ticket;
- exact direct blockers from the approved DAG;
- all assigned contracts, migrations, deletions, tests, and documentation owners represented;
- exact Phase-4 ticket template used;
- shared Delivery Gate referenced rather than duplicated;
- paths/test names marked provisional where blockers have not landed.

Hard constraints:
- do not modify any other final ticket, existing Lean draft, product file, PRD/ADR, or GitHub state;
- do not change D-001–D-013, the 42-ticket boundaries, or semantic DAG;
- no compatibility/productive intermediate route;
- no Retry/Pacing placeholder or Bot-Detection behavior;
- no partial domain payload on budget/failure/Cancellation;
- no provider-specific Rust branch;
- stop and report if an approved boundary cannot be drafted without reopening a decision.

Validation:
- verify assigned file count, required sections, blocker references, and forbidden-contract searches;
- report files written, commands/checks, provisional names, and residual risk.
```

## 14. Reviewer prompt template

```text
Review the complete Phase-4 batch {batch} read-only.

Evidence:
- {batch brief}
- assigned final ticket files
- D-001–D-013
- approved final structure/DAG sections
- named inventories, reconciliation records, original/Lean evidence
- upstream frozen ticket bodies
- shared delivery contract

Check:
- one coherent observable result per ticket;
- every direct blocker produces an interface actually consumed;
- no missing, duplicate, or wrongly moved contract;
- correct control flow, complete usage report, Cancellation and payload rules;
- migration/deletion owner is the replacing/activating ticket;
- no compatibility route, Retry/Pacing placeholder, provider branch, hint promotion, or non-final persistence;
- practical final-interface test seam and focused acceptance evidence;
- exact documentation owner.

Return only evidence-backed findings ordered by severity, with ticket/file references and smallest safe correction. Do not edit project or final ticket files.
```

## 15. Fix-writer prompt template

```text
Apply only the orchestrator-accepted findings to batch {batch}.

Inputs:
- original batch brief
- current assigned files
- accepted findings
- rejected/deferred findings list

Do not alter boundaries, blockers, unrelated wording, other batches, existing Lean drafts, product files, PRD/ADRs, or GitHub.

After correction, rerun structural checks and report exact changes and residual risk.
```

## 16. Global validation after ticket batches

After all 42 files are frozen:

### 16.1 Ticket structure

Prove:

- exactly 42 files;
- exactly 42 unique final labels;
- exact catalogue match;
- required sections present once;
- no duplicate Result/title/label.

### 16.2 DAG

Parse direct blockers from final files and compare to the approved 85-edge semantic DAG:

- no missing edge;
- no extra semantic edge;
- no cycle;
- A02/A03 overlap does not become a blocker;
- each blocker’s produced interface is referenced by the consumer.

### 16.3 Forbidden contract search

Search final bodies for:

- old productive `sourceOverrides` compatibility;
- schema-v2 dual runtime;
- old phase names claimed as final;
- Retry/Pacing fields/counters/limits;
- Bot-Detection guarantees/evasion;
- budget/failure/Cancellation partial payload;
- hint-to-canonical promotion;
- Source-local identity used as cross-Source dedupe;
- known cleanup assigned to G02/later guard;
- provider-/host-/key-specific Rust dispatch;
- copied shared PR-attestation/DoD prose.

Every remaining textual hit must be classified as deletion evidence, historical identifier, non-goal, or error.

### 16.4 Adversarial cross-series review

Review the final chain:

```text
Compiler
→ Runtime/HTTP
→ Primitive families
→ Posting outputs/Policies/Source Detail
→ Browser/Detection
→ Candidate Resolution
→ finalized merge/SQLite persistence
→ global completeness gate
```

The reviewer reports interface gaps, duplicated ownership, incompatible terminals, deletion gaps, and fake dependencies.

## 17. Coverage-matrix batches

Build coverage after final tickets are frozen, grouped as:

1. Compiler/Source activation;
2. Runtime/HTTP;
3. Primitive families;
4. Posting outputs/Policies/Source Detail;
5. Browser/Detection activation;
6. Candidate Resolution/persistence activation;
7. D-001–D-013 and all three inventories.

Then perform a global check:

- every original ticket-specific contract has exactly one disposition;
- every retained contract has exactly one final owner or canonical document;
- every replacement names its D-decision;
- every deferral has no placeholder implementation shape;
- every inventory row has one owner;
- no final ticket owns the same productive deletion twice.

## 18. Publication-manifest batches

Build the future Phase-5 manifest entirely from local mappings. Do not inspect GitHub and do not plan issue-number reuse.

### Direction A — 42 final labels

For every final label:

```text
final label
→ create new issue
→ parent #166
→ direct blockers by final label
→ old local issue responsibilities superseded
→ future reciprocal links
→ readiness rule
```

Future issue numbers remain blank until Phase 5 creates them.

### Direction B — 27 old issue responsibilities

Using only the local ticket index, Reconciliation, and Current-to-Final mapping:

```text
old issue number/label
→ complete set of new final labels replacing it
→ close only after every replacement exists
→ future state reason Not planned
→ reciprocal supersession links
→ no silent abandonment
```

Cross-check for exactly 42 Create actions and 27 Supersede/Not-planned plans. Do not execute any action.

## 19. Phase-5 runbook drafting

Only after final bodies, coverage, and manifest validate:

- write the separately approved publication sequence;
- defer the first live tracker refresh until Phase 5;
- specify stale readiness removal first;
- create the 42 new issues in topological batches;
- set #166 parents and only dependencies whose endpoints exist;
- create every replacement before closing its old issue;
- add reciprocal supersession links;
- close only fully superseded old issues as Not planned;
- validate all 42 responsibilities and all 27 old issue dispositions;
- report exact GitHub changes and residual risk.

The runbook remains documentation until a separate Phase-5 approval.

## 20. Stop and escalation rules

Stop and ask the user when work would require:

- reopening D-001–D-013;
- changing a final ticket boundary or approved semantic dependency;
- adding a compatibility/productive intermediate route;
- inventing Retry/Pacing/Bot-Detection behavior;
- releasing partial payload on budget, failure, or Cancellation;
- promoting hints to canonical values;
- persisting non-final candidates;
- assigning known cleanup to G02 or a later guard;
- creating a fake dependency because files overlap;
- reusing or repurposing any existing GitHub issue number instead of creating the 42 new issues;
- editing tracked canonical docs or GitHub without separate approval.

Ordinary provisional implementation names do not require escalation. Record them for readiness re-baselining.

### 20.1 Required escalation checkpoint

Before stopping for a user decision, the orchestrator must leave the work resumable and unambiguous.

Required actions:

1. stop before making the unapproved decision or changing the affected boundary;
2. complete and freeze any earlier batch that is already review-clean and unaffected;
3. do not start a later batch or dependent ticket;
4. preserve safe current-batch work, but mark the batch `blocked-awaiting-user`, never `frozen`;
5. update `handoff/issue-166-phase-4-progress.md` with the exact blocked batch, completed files, incomplete files, and next action;
6. create `handoff/issue-166-phase-4-working/batches/batch-NN-escalation.md`;
7. report the decision request to the user with a concise recommendation and artifact path.

The escalation artifact must contain:

```md
# Batch NN — Escalation

## Decision required
<one precise question>

## Trigger
<which stop rule was reached>

## Evidence
- exact local source paths/headings
- affected D-decision/final boundary/DAG edge

## Why existing authority is insufficient
<what cannot be resolved by readiness re-baselining>

## Viable options
### Option A
- behavioral consequence
- affected tickets/interfaces/dependencies

### Option B
- behavioral consequence
- affected tickets/interfaces/dependencies

## Recommendation
<recommended option and reason, without applying it>

## Work completed safely
- frozen earlier batches
- current files drafted/reviewed
- validation already performed

## Work intentionally not performed
- affected edits
- dependent batches
- coverage/manifest consequences not yet applied

## Resume instructions
<exact files to read and exact next step after the user decides>
```

Do not discard valid earlier work. Do not mark partial current files as final. Do not continue an allegedly independent later batch by default: a boundary decision can have cross-series consequences. Continue elsewhere only after explicit user approval.

After the user decides:

1. record the accepted decision in the appropriate local normative/structure artifact before drafting against it;
2. update the global contract ledger, catalogue/DAG and affected batch brief;
3. resume the same writer when possible, otherwise give one replacement writer the brief, escalation artifact and user decision;
4. rerun the complete batch review and validation;
5. freeze the batch only after all affected findings are resolved;
6. propagate the approved consequence to later briefs, coverage and publication manifest.

## 21. Session recovery protocol

A new orchestrator session resumes in this order:

1. read `handoff/issue-166-phase-4-ticket-rewrite-handoff.md`;
2. read this playbook;
3. read `handoff/issue-166-phase-4-progress.md`;
4. read the global contract ledger and final ticket catalogue;
5. read only the latest relevant frozen batch handoffs;
6. verify only the local repository baseline and preserve unrelated changes;
7. do not refresh or inspect GitHub during Phase 4;
8. load new full local evidence only inside the next batch’s isolated context builder;
9. continue the exact `Next action` from the progress file.

Do not repeat completed Phase-1–3 discovery. Do not trust an old child session over persisted local artifacts.

## 22. Completion gate

The orchestrator may report Phase 4 complete only after:

- all 15 batches are frozen;
- exactly 42 final ticket bodies validate;
- the approved DAG validates unchanged and acyclic;
- contract coverage is complete;
- all 42 final labels have Create actions and all 27 old issues have Supersede/Not-planned plans in the publication manifest;
- the Phase-5 runbook exists;
- all D-decisions and inventory rows have owners;
- no unauthorized source, Lean, canonical-document, or GitHub edit occurred and no GitHub data was loaded during Phase 4;
- residual readiness uncertainty is explicit;
- the complete package is presented to the user for approval.
