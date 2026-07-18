# Issue #166 — Phase-4 Ticket Rewrite Handoff

Status: **local handoff for a fresh or less-capable session — Phases 1–3 complete, Phase 4 not started**  
Date: **2026-07-18**

## 1. Purpose

Start **Phase 4: generate the final local ticket bodies and the complete GitHub migration manifest** from the approved 42-ticket target structure.

Phases 1, 2, and 3 are complete. Do not repeat their discovery work, recreate their deleted artifacts, or silently reopen their accepted decisions.

Phase 4 remains local. It must not implement product code or modify GitHub. GitHub publication and cleanup are Phase 5 and require a separate explicit approval.

Execution across fresh agents and resumable batches is defined in `handoff/issue-166-phase-4-batch-orchestration.md`. Read that companion playbook before delegating Phase-4 work. It defines the 15 batch groups, role boundaries, batch briefs, writer/reviewer/fix loops, progress artifacts, prompt templates, validation, and session recovery. The approved ticket boundaries and semantic DAG in this handoff and the Phase-3 structure remain authoritative.

## 2. Phase history

### Phase 1 — Original/Lean reconciliation and contract decisions: complete

Phase 1:

- fetched #166 and all 27 implementation issues from live GitHub read-only;
- compared every original ticket with its existing Lean draft;
- recorded preserved, moved, changed, stale, and lost contracts;
- created the conflict register;
- resolved all product and architecture conflicts with the user;
- recorded accepted normative decisions **D-001 through D-013**.

Primary results:

- all 27 original/Lean pairs have a reconciliation verdict;
- the former T12b budget-prefix contract loss is intentionally replaced by D-003;
- Retry/Pacing is deferred because no executable Retry capability exists;
- budgets are safety ceilings, not Bot-Detection or Prompt-Injection prevention;
- no GitHub state was changed.

Normative decision source:

- `handoff/issue-166-contract-decisions.md`

Evidence:

- `handoff/issue-166-original-to-lean-reconciliation.md`
- `handoff/issue-166-conflict-register.md`

### Phase 2 — Inventories and 47-slice upper-bound restructuring proposal: complete

Phase 2 created complete inventories for:

- the productive `sourceOverrides` hard-cut surface;
- the shared Browser seam, callers, adapters, tests, and deletion targets;
- every currently admitted Primitive family, schema/Serde mismatch, duplicate implementation, and proposed owner.

It then produced a 47-slice upper-bound target proposal and acyclic dependency graph.

Primary results:

- Direct Source Specialization uses retained final foundations followed by one atomic activation;
- Browser Acquisition uses one phase-neutral module with managed and scripted adapters followed by one cross-phase activation;
- every admitted Primitive family has an explicit owner and one implementation-free global gate;
- Candidate Resolution, finalized handoff, and atomic persistence have explicit boundaries;
- no Lean ticket or GitHub state was changed.

Primary artifacts:

- `handoff/issue-166-source-overrides-cut-inventory.md`
- `handoff/issue-166-browser-cut-inventory.md`
- `handoff/issue-166-primitive-owner-inventory.md`
- `handoff/issue-166-restructuring-plan.md`

### Phase 3 — Consolidation and final target structure: complete

Phase 3 reviewed all 47 proposed slices for:

- one retained observable outcome;
- one coherent module interface;
- one practical caller-facing test surface;
- one same-slice deletion owner;
- no temporary productive route or compatibility layer;
- direct blockers that actually produce consumed interfaces;
- reliable agent sizing.

The final recommendation is **42 implementation tickets** with an acyclic **85-edge semantic DAG**.

Structural corrections from 47 to 42:

1. merge proposed C02+C03;
2. merge proposed P06b+P06c;
3. split proposed B03 into B03a+B03b;
4. absorb G01 into A02;
5. merge proposed Q01+Q02;
6. combine proposed Q03+DB01+DB03 into one A03 activation;
7. retain proposed DB02 as final DB01 transaction foundation.

Final structure source:

- `handoff/issue-166-recommended-final-ticket-structure.md`

This document is authoritative for Phase-4 ticket boundaries, labels, direct semantic dependencies, Current-to-Final responsibility mapping, activation/deletion ownership, risks, and documentation owners. Its earlier GitHub reuse guidance is superseded for Phase 4/5 by the create-new-only publication rule recorded below.

## 3. Authority hierarchy for Phase 4

Phase 4 works exclusively from the reviewed local evidence package. Live GitHub issues, comments, labels, parents, dependencies, body digests, and issue-number reuse are outside the Phase-4 working context. This handoff’s create-new-only rule supersedes the Phase-3 document’s earlier recommendation to maximize issue reuse; the Phase-3 Current-to-Final table remains authoritative only for responsibility/contract mapping.

Use this order:

1. `handoff/issue-166-contract-decisions.md` — normative D-001–D-013 decisions. Do not reopen them silently.
2. `handoff/issue-166-recommended-final-ticket-structure.md` — approved Phase-3 target boundaries and semantic DAG.
3. Existing 27 Lean drafts — reusable local contract material for the relevant batch; their old boundaries and wording are not authoritative.
4. The three Phase-2 inventories — authoritative local evidence for current callers, old paths, variants, tests, and deletions.
5. Original-to-Lean reconciliation — reviewed original-contract disposition evidence; original GitHub bodies are not reread in Phase 4.
6. Conflict register — conflict evidence and accepted resolution ownership.
7. `handoff/issue-166-delivery.md` — shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.
8. `CONTEXT.md` — current domain vocabulary, subject to activation-time migrations explicitly assigned by D-001/D-013.
9. `docs/prd/declarative-profile-strategy-algebra.md` and ADRs where not refined by D-001–D-013.
10. `handoff/issue-166-ticket-index.md` — historical navigation/mapping input only, not live tracker evidence.

When an old PRD/ADR/Lean statement conflicts with D-001–D-013 or the final 42-ticket structure, the accepted decision/final structure wins. Record the required canonical-document migration in the owning final ticket; do not revive the old contract.

Do not fetch or reread GitHub original bodies during Phase 4. Do not restore deleted snapshots or archives.

## 4. Compact-first reading and evidence loading

Phase 4 must not load the complete 27 original bodies, all 27 Lean drafts, every inventory, and every canonical document into one orchestrator context. Phases 1–3 already produced reviewed compact evidence. Use context isolation and evidence-on-demand.

### 4.1 Orchestrator core reads

The active orchestrator reads these files completely before creating Phase-4 output:

1. `AGENTS.md`
2. `CONTEXT.md`
3. `handoff/issue-166-phase-4-ticket-rewrite-handoff.md`
4. `handoff/issue-166-phase-4-batch-orchestration.md`
5. `handoff/issue-166-contract-decisions.md`
6. `handoff/issue-166-recommended-final-ticket-structure.md`
7. `handoff/issue-166-delivery.md`
8. `handoff/issue-166-phase-4-progress.md` when resuming an existing Phase-4 run

These documents define the accepted decisions, final boundaries, DAG, workflow, and shared delivery rules.

### 4.2 Delegated evidence reads

Fresh context builders read the following completely only for their assigned domain/batch and return compact evidence into the batch brief:

- the relevant Source/Browser/Primitive inventory;
- relevant records from the Original-to-Lean reconciliation and conflict register;
- only the existing Lean drafts whose contracts feed the assigned final tickets;
- relevant PRD/ADR sections and current-domain clauses;
- relevant final upstream ticket bodies after they are frozen.

No batch writer needs all 27 Lean drafts. It reads only the Lean sources and evidence named in its batch brief.

The final coverage pass distributes the 27 original-ticket dispositions across domain reviewers and then validates the combined coverage matrix. It does not require one context to reread all original bodies.

### 4.3 No GitHub baseline in Phase 4

Do not fetch, refresh, compare, hash, or inspect live GitHub issues during Phase 4. Specifically:

- no GitHub baseline;
- no original-body reread;
- no comments, labels, parent or dependency refresh;
- no `updatedAt` or body-digest comparison;
- no planning to reuse existing GitHub issue numbers.

The reviewed local Lean drafts, reconciliation, decisions, inventories, and final structure are sufficient Phase-4 inputs. Live tracker state is refreshed only if and when Phase 5 publication receives separate approval.

Repository warning: unrelated staged/modified files already exist. Do not modify, unstage, discard, or attribute them to #166.

## 5. Fixed final ticket catalogue

Phase 4 writes exactly these **42 local ticket bodies** unless the user explicitly reopens Phase 3.

### Compiler and Source activation — 8

- C01 — Authoritative Effective Profile Compiler Boundary
- C02 — Deterministic Effective Profile Merge and Complete Additions
- C04 — Effective Source Config Contract
- C05 — Final Mandatory `first_accepted` Policy Foundation
- C06 — Final Internal and Compiled Phase Names
- C07 — Complete Effective Profile Provenance
- C08 — Canonical Schema-v3 Fingerprint Foundation
- A01 — Atomic Source Schema-v3 Activation and Old-path Removal

### Runtime and HTTP — 3

- R01 — Typed Strategy Set Kernel
- R02 — Cumulative Phase Allowances and Complete Reports
- H01 — Byte-preserving Phase-neutral HTTP Acquisition and Strict Decoding

### Primitive families — 12

- P01 — Canonical Template Grammar
- P02 — Canonical Parse Primitives
- P03 — Canonical Select Primitives
- P04 — Canonical Cardinality Primitives
- P05 — Canonical Transform Primitives
- P06a — Typed Value Context Foundation
- P06bc — Complete Value Execution and Composition
- P07 — Canonical Predicate Primitives
- P08 — Canonical Named Capture Primitive
- P09 — Canonical Authored HTTP Fetch
- P10 — Canonical Bounded HTTP/Browser Pagination
- P11 — Canonical Phase Acceptance

### Posting outputs, Policies, and Source Detail — 7

- O01 — Source-local Posting Occurrences and Discovery Value Semantics
- O02 — Requested Detail Patches and Conflict-safe Phase Reducers
- O03 — Shared Discovery/Detail Phase Outcome and Commit Boundary
- K01 — `all_required` Strategy Policy
- K02 — `at_least(count)` Strategy Policy
- K03 — `collect_all(minAccepted)` Strategy Policy
- S01 — Candidate-scoped Source Detail Outcome, Routing, and Execution Seam

### Browser and Detection — 8

- B01 — Browser Acquisition Contract and Scripted Adapter
- B02 — Managed Browser Acquisition Adapter
- B03a — Canonical Browser Fetch, Wait, and Interaction Primitives
- B03b — Discovery and Detail Browser Phase Adapters
- D01 — Reconciled Detection State
- D02 — URL and HTTP Detection Strategies
- D03 — Browser Detection Strategy
- A02 — Atomic Browser/Detection Activation and Residue Proof

### Candidate Resolution and persistence — 3

- Q01 — Resolve Source Candidates through one Bounded Batch Operation
- DB01 — Atomic Search Run/Match SQLite Transaction Foundation
- A03 — Activate Finalized-only Search Run Resolution, Merge, and Persistence

### Global gate — 1

- G02 — Global Primitive Completeness Gate

Do not merge R01/R02, K02/K03, C01/C02, B01/B02, H01/P09, O02/O03, or any productive activation. Do not recreate G01 as a separate ticket.

## 6. Three atomic activation owners

### A01 — Source schema-v3 activation

A01 alone makes Direct Source Specialization productive and deletes the complete old Source path. It owns every row of `issue-166-source-overrides-cut-inventory.md`, including:

- Rust/Schema/TypeScript/UI authored surfaces;
- compiler facade/callers and `compiler/overrides.rs`;
- commands, registry, validation, Search Run preparation, lazy Detail preparation;
- canonical fingerprint activation and old freshness deletion;
- Retry ghost removal;
- fixtures, tests, examples, active docs, ADR 0001/0009 and vocabulary migration;
- strict rejection/manual recreation of external old Source JSON.

No compatibility wrapper, translator, alias, fallback, or dual productive route.

### A02 — Browser/Detection activation

A02 alone migrates all productive Browser/Detection callers and deletes the complete old seam/route. It includes the residue proof formerly proposed as G01.

- B03a owns final Primitive behavior and nonproductive local cleanup.
- A02 owns productive `ProfileBrowserClient`, `render*`, old phase fetch branches, six construction sites, three leaf calls, fakes, exports, DTOs, commands, Source Live Check, final Search Run, posting/UI, runtime-admin smoke, old Detection evaluators/maps/builders, and guard evidence.

No later guard inherits known cleanup.

### A03 — Candidate/Search Run/persistence activation

A03 switches Search Run to Q01, performs the sole productive finalized conversion, retargets the existing cross-Source merge without a duplicate merger, invokes DB01 once, narrows constructors, deletes the old vector/candidate/import route, updates bounded smoke/artifacts, and supersedes conflicting ADR 0008 clauses.

A03 is atomic because a separate productive Search Run activation would require a temporary conversion back into the old broad merge/import model.

A02 and A03 overlap Search Run files but have no semantic dependency. Prefer A03 before A02 for no-rework scheduling, but do not invent a native blocker edge.

## 7. Non-negotiable contracts

- D-001–D-013 remain fixed.
- No provider-/ATS-/host-/company-/Source-key-/Profile-key-specific Rust execution branch.
- Search Request criteria remain outside Source Config, Source Profiles, Access Paths, and Source specialization.
- Runtime consumes immutable typed plans only.
- Cancellation is typed control flow, never Resolution Partial Completion, and releases no Resolution automatically.
- Source-local Posting Occurrence identity remains distinct from cross-Source Job Posting deduplication.
- Hints never become canonical persisted values.
- Budgets are safety ceilings, not traffic targets, Bot-Detection guarantees, or Prompt-Injection prevention.
- Retry/Pacing has no initial target capability or placeholder fields/counters.
- Only `Completed::Accepted` carries reduced phase payload.
- Every started phase terminal carries one complete usage report; no usage is reconstructed from Diagnostics.
- No compatibility wrappers, translators, aliases, fallback, or dual productive routes.
- Known migration/deletion belongs to the replacing/activating ticket, never a later guard.
- Tests use the highest practical final interface and real deterministic adapters/temporary SQLite as specified by the delivery contract.
- Proposed modules remain deepening candidates until implementation evidence satisfies the architecture criteria.

## 8. Phase-4 deliverables

Create four local artifacts.

### 8.1 Final ticket bodies

Suggested directory:

`handoff/issue-166-phase-4-tickets/`

Create exactly one file per final label, for example:

- `C01-authoritative-effective-profile-compiler.md`
- `A01-source-schema-v3-activation.md`
- `B03a-browser-fetch-primitives.md`
- `A03-finalized-search-run-persistence-activation.md`

Do **not** restore `handoff/issue-166-final-tickets/`; that deleted path was an obsolete original-ticket snapshot directory.

Each final body should contain only:

```md
# <Outcome-oriented title>

## Result
<one observable result>

## Readiness and direct blockers
- Parent #166
- local final blockers, by label until GitHub numbers exist
- readiness uncertainties that must be re-baselined

## Consumed contracts
- exact D-001–D-013 decisions and PRD/ADR sections
- only direct blocker interfaces actually consumed

## Current gap
- provisional current paths/callers/tests from the inventories
- explicitly re-baselined before implementation

## Contract delta
- caller-facing inputs/outputs
- invariants, ordering, error/control flow, bounds, Diagnostics
- what complexity callers no longer know

## Scope, migration, and deletion
- implementation responsibility
- production/test callers moved
- exact old paths/types/functions/exports/fakes/tests deleted
- ticket-specific deletion test

## Adjacent non-goals
- only realistic neighboring work

## Acceptance and validation
- compact case → outcome → test/static-check matrix
- intended interface/test seam
- deterministic production/test adapter strategy where applicable
- focused commands and residue searches

## Documentation ownership
- exact docs updated by this behavior slice

## Delivery gate
Follows `handoff/issue-166-delivery.md`.
```

Do not copy shared delivery/PR-attestation prose into every ticket.

### 8.2 Final contract-coverage matrix

Suggested path:

`handoff/issue-166-phase-4-contract-coverage.md`

For every original ticket-specific contract, record exactly one of:

- final owner label;
- shared canonical document;
- explicitly replaced by D-001–D-013;
- explicitly deferred;
- explicitly removed as unsupported/obsolete.

Required proofs:

- all 27 originals are covered;
- former T12b prefix output is replaced by D-003/O03;
- Retry/Pacing is deferred, not represented by placeholders;
- every Source/Browser/Primitive inventory row has one migration/deletion owner;
- every D-001–D-013 decision has one enforcement point;
- no final ticket silently reintroduces a removed conflict.

### 8.3 GitHub migration manifest

Suggested path:

`handoff/issue-166-github-migration-manifest.md`

This publication manifest is derived entirely from local Phase-4 artifacts. It must map every final label to one future action:

```text
final label
→ create new issue in Phase 5
→ final parent #166
→ direct blockers by final label (future issue numbers assigned only during publication)
→ old local/current issue responsibilities superseded
→ future reciprocal supersession links
→ final readiness rule
```

No existing issue number is reused or repurposed. Also map all 27 current issues from the local index/reconciliation to:

- the complete set of new final labels that supersede it;
- the point at which all replacements exist;
- a future **Not planned** closure with reciprocal supersession links;
- never silently abandoned.

Rules:

- plan **42 new issues**; no Reuse action exists;
- do not inspect live GitHub while writing the manifest;
- create all replacements for an old issue before closing it;
- absorbed issues later receive a comment `Superseded by #…` and reciprocal links;
- only fully superseded old issues close as Not planned;
- Phase 5 removes stale readiness before closure/publication work;
- all final implementation issues become children of #166;
- blocked tickets receive no readiness label;
- the final native graph contains only real semantic blockers;
- A02/A03 scheduling overlap is not a fake dependency.

Do not execute the manifest in Phase 4.

### 8.4 Phase-5 publication runbook

Suggested path:

`handoff/issue-166-phase-5-publication-runbook.md`

Describe the separately approved tracker workflow:

1. refresh live tracker state only after Phase-5 approval;
2. remove stale readiness;
3. publish the 42 new issues in topological batches;
4. set #166 as parent and install only blocker edges whose endpoints exist;
5. after every replacement for an old issue exists, add reciprocal supersession links;
6. close that fully superseded old issue as Not planned;
7. update #166 navigation/checklists;
8. validate all 42 responsibilities, labels, states, parents and DAG;
9. stop and report exact changed issues and residual risk.

The runbook is documentation only in Phase 4.

## 9. Required ticket-specific corrections

Apply these while drafting final bodies:

- C05 must remain nonproductive for schema-v3 Source authoring until A01.
- C06 is L/high change-volume and owns a direct rename, not aliases.
- P02 may not retain a permanent rejection-only `text` registration; final admission must be removed or genuinely executable.
- P07 must freeze ownership of Detection `contains`/status predicates.
- P08 must freeze named-capture selection and remove unnamed fallback.
- P11 must define `minResults` placement and remove `maxErrorRatio` absent evidence.
- S01 must own production and scripted implementations, one unchanged full Strategy Set/Policy invocation, UI/Live Check migration, and deletion of the description-era public Detail operation/result family.
- B03a owns Primitive-local nonproductive cleanup; A02 owns every productive old Browser seam deletion.
- Q01 contains protocol, parent allowance, state machine and finalization behind one Source-scoped operation; no public per-candidate loop.
- A03, not a temporary Q03/DB conversion, owns productive finalized conversion/merge/persistence activation.
- G02 owns no migration or deletion.

## 10. Canonical-document alignment to record

Some current canonical docs remain stale. Final tickets must assign their updates; Phase 4 should not silently cite stale clauses as target contracts.

At minimum record:

- old T14 dependency order → D01 before D02/D03 and A02 activation;
- unconditional Retry wording → D-012 conditional future invariant;
- T16 sample-limit gate → accepted value 10;
- Source Overrides/current phase vocabulary → A01 activation migration;
- Posting Occurrence/provider values/hints → O01;
- requested Source Detail outcome → O03/S01;
- Browser lifecycle/teardown → B01/B02/A02;
- durable Search Runs/Matches and ADR 0008 supersession → A03;
- SCHOTT/raw Candidate artifact correction → A03.

Do not edit tracked PRD/ADR files in Phase 4 unless the user separately approves that tracked documentation change. The ticket bodies and coverage matrix must nevertheless identify the exact owner.

## 11. Recommended Phase-4 workflow

1. Read the orchestrator core documents completely.
2. Create the compact global contract ledger and final-label checklist with all 42 labels/direct blockers from local evidence only.
3. Confirm that no Phase-4 task depends on live GitHub state or issue-number reuse.
4. Build one batch brief from fresh-context, domain-scoped evidence reads.
5. Draft that batch in dependency order with one writer.
6. Review, correct, validate and freeze the batch before loading the next batch evidence.
7. After all domain batches, run an adversarial cross-series pass over compiler → runtime → Primitive → Detection/Browser → Detail → Candidate Resolution → persistence.
8. Build the contract-coverage matrix through domain-scoped reviewers and one compact combined validation.
9. Build the exact GitHub migration manifest.
10. Build the Phase-5 runbook.
11. Validate file count, mapping count, DAG, links, and no unauthorized edits.
12. Present the complete local package to the user and stop for approval.

Do not let several writers edit the same ticket directory concurrently without isolated worktrees and deterministic merge ownership. A less-capable session should prefer sequential domain groups and explicit checklists over broad parallel rewriting.

## 12. Phase-4 validation

Before completion, prove:

- exactly 42 final ticket files exist;
- every file has one label/title/result/direct-blocker set;
- every direct blocker produces an interface consumed by the ticket;
- the final DAG matches `issue-166-recommended-final-ticket-structure.md` and is acyclic;
- all 27 current issues appear in the migration manifest;
- all 42 final labels have a `create new issue` publication action;
- every one of the 27 old issues has a complete future replacement and Not-planned supersession plan;
- all D-001–D-013 decisions have one owner/enforcement point;
- A01/A02/A03 own complete activation/deletion proof;
- G02 owns no known cleanup;
- no Retry/Pacing placeholder exists;
- no existing Lean draft, tracked product/canonical file, or GitHub state was modified; no GitHub data was fetched into the Phase-4 context;
- unrelated staged files remain untouched.

Useful structural checks may count files/labels, parse local dependency declarations, topologically validate the graph, and search for forbidden obsolete clauses. Do not run product tests because Phase 4 changes no product code.

## 13. Stop conditions

Stop and ask the user if drafting would require:

- reopening D-001–D-013;
- changing the approved 42-ticket boundaries;
- adding a compatibility or productive intermediate route;
- inventing Retry/Pacing/Bot-Detection behavior;
- exposing partial domain payload on budget/failure/Cancellation;
- promoting hints to canonical values;
- persisting non-final Candidate values;
- assigning known cleanup to G02 or another later guard;
- making a fake semantic dependency solely because files overlap;
- changing the accepted create-new-only publication policy or attempting to reuse/repurpose an existing issue number.

Ordinary final symbol/path/test names remain provisional and are re-baselined at implementation readiness; they do not require reopening architecture decisions.

## 14. Phase-4 completion criteria

Phase 4 is complete only when:

- 42 final local ticket bodies exist;
- contract coverage is complete and reviewed;
- the exact 42-create/27-supersede publication manifest exists;
- the Phase-5 publication/cleanup runbook exists;
- the final DAG is acyclic and unchanged semantically;
- every current issue is accounted for;
- every absorbed issue has a linked replacement plan;
- no product code, current Lean draft, tracked canonical document, or GitHub state changed without separate approval;
- residual implementation-readiness uncertainties are explicit;
- the user receives the complete package for approval before Phase 5.

## 15. Fresh-session starter prompt

> Lies `handoff/issue-166-phase-4-ticket-rewrite-handoff.md` und `handoff/issue-166-phase-4-batch-orchestration.md` vollständig. Lies danach die dort definierten Orchestrator-Core-Dokumente. Behandle Phase 1–3, D-001–D-013 und die 42-Ticket-Struktur als akzeptiert. Verwende in Phase 4 ausschließlich lokale Quellen; rufe GitHub nicht ab und plane keine Wiederverwendung alter Issue-Nummern. Erstelle pro Batch ein kompaktes Briefing aus den jeweils relevanten Lean Drafts, Inventar-/Reconciliation-Records und kanonischen Abschnitten. Schreibe, reviewe, korrigiere und friere immer nur einen Batch ein. Erstelle anschließend lokal die vollständigen 42 Ticket Bodies, Contract-Coverage-Matrix, das 42-Create/27-Supersede-Publikationsmanifest und den Phase-5-Publikationsrunbook. Implementiere nichts, ändere keine bestehenden Lean Tickets, keine tracked PRD-/ADR-Dateien und keine GitHub-Issues. Stoppe vor jeder GitHub-Änderung und lege das vollständige Phase-4-Paket zuerst zur Freigabe vor.
