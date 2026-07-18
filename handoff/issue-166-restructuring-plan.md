# Issue #166 — Current-to-Target Restructuring Plan

Status: **local Phase-2 proposal for review — no Lean or GitHub changes approved**  
Baseline: **2026-07-18**

## 1. Scope, authority, and tracker baseline

This plan applies the accepted decisions in `handoff/issue-166-contract-decisions.md` **D-001 through D-013**. It does not reopen them. Current ticket labels identify existing responsibilities only; target labels below are local planning IDs, not proposed GitHub issue numbers.

Evidence:

- all required handoff, domain, PRD, ADR, delivery, reconciliation, conflict, and lean-ticket documents;
- refreshed live bodies/comments for #166 and all 27 implementation issues;
- refreshed native parent/dependency metadata;
- current repository call graphs and the three completed inventories:
  - `handoff/issue-166-source-overrides-cut-inventory.md`;
  - `handoff/issue-166-browser-cut-inventory.md`;
  - `handoff/issue-166-primitive-owner-inventory.md`.

Live tracker state is unchanged from Phase 1:

- all 27 implementation issues are open children of #166;
- only #167 has `ready-for-agent`;
- implementation issues have no comments; #166 has one historical decomposition comment;
- native dependencies still match `handoff/issue-166-ticket-index.md`.

No current issue is ready under the accepted target structure. Removing #167’s readiness label requires a later separately approved GitHub workflow.

## 2. Inventory conclusions that constrain restructuring

### 2.1 `sourceOverrides` hard cut

The old model is one productive cross-stack path spanning:

- Rust Source/override documents and exports;
- `compiler/overrides.rs`, selected-path mutation, old compiler facade, and four direct productive compiler callers;
- Source JSON Schema and frontend schema catalog;
- TypeScript Source DTO, Create/Edit models/hooks/drawers/editor, and Details;
- registry/filesystem round trips and Source validation;
- Search Run, lazy Detail, Source Live Check, old freshness components;
- fixtures, compiler/registry/live-check/UI tests, active domain/PRD/ADR/agent guidance.

D-001/D-011 therefore require retained final foundations followed by one atomic Source schema-v3 activation. No wrapper, translator, old/new productive path, or cleanup ticket is permitted.

### 2.2 Shared Browser seam

The current seam has:

- three leaf calls: Detection `render`, Discovery `render_with_context`, Detail `render_with_context`;
- six managed production-construction sites;
- eight current trait implementations/fakes;
- no shared scripted Browser adapter;
- productive callers in Detection, Source Live Check, Search Run, lazy posting Detail, commands, and runtime-admin smoke;
- teardown outside the current timeout, no proven forced terminate/reap, discarded cleanup failures, and no complete action/wait/byte/duration reporting.

D-006/D-007 require a final phase-neutral Browser Acquisition module, managed and scripted adapters, typed phase adapters, native Detection contributions, then one cross-phase activation deleting the complete old seam. T14d remains guard-only.

### 2.3 Primitive ownership

There is currently no global Primitive registry. Fetch and Pagination compile to typed enums, while Parse/Select and most Filter/Capture/Value/Transform/Acceptance documents are copied into plans and dispatched in phase-specific switches.

Explicit target owners are required for:

- Template grammar;
- HTTP Fetch and request bodies;
- Browser Fetch/waits/interactions;
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

The initial target removes rather than registers:

- the Retry ghost under D-012;
- Serde-only script/eval/DOM/login/CAPTCHA Browser interactions;
- camelCase Transform aliases;
- unsupported `maxErrorRatio`, unless separate evidence and approval exists before its owner is implemented;
- placeholder registrations for future predicates or formats.

### 2.4 Registry identity rule

To avoid ticket/file explosion while preserving D-005:

- a **registered Primitive identity** is a top-level or independently dispatched executable authored DSL type with its own context/compilation/execution semantics;
- nested options such as HTTP method, request-body shape, pagination parameter location, wait/interaction subtype, or Acceptance property remain exhaustively covered by Schema/Serde/compiled parity but are owned by their parent family unless they demonstrably dispatch independently across callers;
- every admitted nested option still has exactly one implementation owner and deletion proof;
- the global gate derives the final identity set from the landed schema rather than hard-coding today’s provisional classification.

This is an implementation-classification rule under D-005, not a reopening of its single-owner requirement.

## 3. Proposed target catalogue

The proposal contains **47 local target slices**: 13 current responsibilities remain focused, 8 current tickets split, 5 move, and 1 merges into an activation; the remaining labels expose work that current tickets left ownerless, chiefly Primitive families, Browser adapters, activation cuts, and convergence gates. This is an upper-bound reliability decomposition for review—not approval to create 47 GitHub issues. Adjacent slices may be merged later only when they share one retained module interface, one test surface, and one deletion owner without recreating an XL or dual-route ticket.

All proposed modules remain **deepening candidates** until implementation evidence satisfies the architecture-language acceptance criteria.

### 3.1 Compiler and Source activation

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **C01 Compiler Boundary** | Final authoritative `compile_source(source, registry)` interface and final result branches exist; Direct Source Specialization remains non-authorable/non-productive. | — | M |
| **C02 Keyed Merge** | Recursive existing-entry merge, deterministic order, whole non-keyed-array replacement, no implicit deletion. | C01 | M |
| **C03 Complete Additions** | Complete new Strategies/Access Paths append deterministically and all selected/unselected additions validate before selection. | C02 | M |
| **C04 Effective Source Config Contract** | One constrained schema compiler/validator shared by compiler and Detection, including incremental/final operations. | C03 | L |
| **C05 Mandatory `first_accepted`** | Final authored/compiled mandatory Policy exists and current fallback parity is preserved. | C04 | M |
| **C06 Final Internal Phase Names** | Internal/compiled `detection`, `discovery`, `detail` names land directly without aliases. | C05 | L |
| **C07 Effective Profile Provenance** | Complete final-name provenance, including Policy and schema terminals, is created once. | C06 | L |
| **C08 Canonical Schema-v3 Fingerprints** | Final component projection/version/global protocol exists but is not productively wired. | C07 | L |
| **A01 Source Schema-v3 Activation** | Atomically activates Direct Source Specialization and canonical fingerprints across backend/frontend/callers; strictly rejects old JSON; deletes every `sourceOverrides`/old compiler/freshness inventory row; and removes the authored/Serde/compiled Retry ghost so initial schema v3 contains no Retry dimension. | C08 | **XL atomic** |

C01–C08 are retained final foundations. Before A01, only the old authored/productive Source route exists; after A01, only schema v3 exists.

### 3.2 Typed runtime and HTTP acquisition

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **R01 Strategy Set Kernel** | One crate-private typed kernel replaces duplicate Discovery/Detail fallback loops and Diagnostic-derived Cancellation. | A01 | L |
| **R02 Phase Allowance and Report** | Cumulative safety ceilings, debit-before-effect, and one complete indivisible report on every started terminal; no Retry dimension. | R01 | L |
| **H01 Byte-preserving HTTP Acquisition** | One phase-neutral production/scripted HTTP acquisition seam, bounded byte collector, response metadata, strict decoder, and typed transport failure. No phase-specific interpretation. | R02 | L |

H01 repairs the current false dependency where T14a expects a Detection-capable HTTP interface that current T10 explicitly does not produce.

### 3.3 Canonical Primitive-family owners

Each family moves its behavior to final `profile_dsl/primitives/<family>/...` owners, migrates real callers, and deletes duplicate behavior in the same slice.

| Label | Family outcome | Direct blockers | Risk |
|---|---|---|---|
| **P01 Template** | One context-neutral template grammar/compiler/renderer; phase owners retain namespace admission. | A01 | M |
| **P02 Parse** | Canonical JSON/XML/HTML owners, shared Parsed Document, and owned compile-time `text` context contract. | H01 | M |
| **P03 Select** | Six canonical Select owners, shared selected-item model, syntax/context/placement checks. | P02 | L |
| **P04 Cardinality** | Canonical `one`, `first`, `optional`, `all`; duplicate phase implementations removed. | P03 | M |
| **P05 Transform** | Ten canonical snake_case transforms; aliases and schema/Serde mismatches removed. | P03 | L |
| **P06a Value Context Foundation** | Typed Discovery/Detail value contexts, static availability, shared registry shape, depth/node/candidate bounds, and no raw authored runtime context. | P03 | M |
| **P06b Value Lookups** | Canonical direct lookup variants, const parity, cardinality/transform application, and complete removal of duplicate phase lookup switches. | P06a, P03, P04, P05 | L |
| **P06c Value Composition** | Canonical template/combine/list composition and real bounded `first_non_empty`; capture chaining remains impossible. | P06b, P01 | L |
| **P07 Predicate** | Canonical non-empty, regex, equality, and evidenced Detection predicates; syntax validates before I/O. | P06c | M |
| **P08 Capture** | One named regex-capture engine; no phase duplicates, unnamed fallback, mutable overwrite, or partial-map chaining. | P06c | M |
| **P09 HTTP Fetch** | Canonical authored HTTP Fetch, GET/POST and JSON/text/form body behavior over H01; Discovery/Detail callers migrate and Detection consumes a thin typed adapter. A01 has already removed Retry; this slice proves no Retry residue returns. | H01, P01 | L |
| **P10 Pagination** | Canonical page/offset/cursor/sitemap behavior across HTTP or Browser Fetch and parent-owned parameter-location options. | P09, B03, P03, R02 | L |
| **P11 Acceptance** | Canonical retained Acceptance checks and placement validation; `maxErrorRatio` initial admission removed absent evidence. | P07, O01, O02 | L |

### 3.4 Posting output, shared outcome, Policies, and Source Detail

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **O01 Posting Occurrence** | One Source-local occurrence model with disjoint provider values/hints/postingMeta and conservative URL identity. | P06c | L |
| **O02 Requested Patch and Reducers** | Four-field requested Detail patch plus conflict-safe Discovery/Detail reducers and contribution provenance through final interfaces. | O01 | L |
| **O03 Shared Phase Outcome** | Exclusive D-003/D-010 owner: closed report-bearing outcomes, `PolicyUnsatisfiedCause`, pure reducer attachment, pre-commit Cancellation, exhaustive caller migration. Only Accepted has payload. | R02, O02, P11 | L |
| **K01 `all_required`** | Policy-specific compiled variant, fail-fast transition, terminal Diagnostic/tests only. | O03 | M |
| **K02 `at_least(count)`** | Policy-specific threshold/reachability transition and tests only. | O03 | M |
| **K03 `collect_all(minAccepted)`** | Policy-specific execute-all/natural-completion transition and tests only. | O03 | M |
| **S01 Source Detail Outcome** | Closed D-008 outcome, requested-field capability/reuse/routing, dispositions only on Completed, exact report projection, UI/Live Check caller migration. | O03 | L |

O03 explicitly implements the accepted replacement of the published T12b reduced-prefix-on-budget rule. K01–K03 contain no first-lander result migration.

### 3.5 Shared Browser foundation and phase adapters

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **B01 Browser Acquisition Contract + Scripted Adapter** | Final phase-neutral interface, bounded lifecycle/control model, typed infrastructure failures, usage contract, and one deterministic scripted adapter; non-productive. | R02 | L |
| **B02 Managed Browser Adapter** | Real Chromium lifecycle with bounded graceful close/forced terminate/reap/handler/filesystem finalization and retained runtime-admin behavior. | B01 | **L/high feasibility risk** |
| **B03 Browser Fetch and Posting-phase Adapters** | Canonical authored Browser Fetch/waits/interactions and thin Discovery/Detail adapters over B01; prohibited Serde-only interactions removed. No productive old-seam cut yet. | B01, P01, O03 | L |

Filesystem residue may remain private when safely quarantined; inability to establish process termination/reap is the typed infrastructure failure. B02 must not blindly turn every harmless `remove_dir_all` residue into a domain failure.

### 3.6 Detection foundations and activation

D-006 requires the state/reducer interface before Strategies can emit/consume it; this reverses the current T14a→T14b implementation assumption.

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **D01 Reconciled Detection State** | `DetectionContribution`, conflict-safe incremental reducer, immutable state, incremental/final C04 validation, and sole proposal constructor; non-productive. | A01 | L |
| **D02 URL/HTTP Detection Strategies** | Final URL and HTTP Strategies consume reconciled state and emit native ordered contributions under `all_required`; no productive old-path deletion. | D01, P01, P07, P08, P09, K01 | L |
| **D03 Browser Detection Strategy** | Final Browser Strategy, native contributions, invocation/profile/Strategy ceilings, rendered-byte checks, and phase projection; no aggregate translation. | D01, R02, B01, B03, P07, P08, K01 | L |
| **A02 Browser/Detection Activation** | One atomic cut migrates Detection/Discovery/Detail, Source Live Check, final Search Run, posting/UI, commands, runtime smoke/admin internals, and all deterministic tests; deletes the complete old Browser seam and mutable Detection route. | D02, D03, B02, B03, Q03 | **XL atomic** |
| **G01 Detection Residue Guard** | Implementation-free call-graph/residue proof; owns no known migration/deletion. | A02 | S |

A02 waits for Q03 so the current vector Search Run Browser caller is not migrated and immediately discarded.

### 3.7 Candidate Resolution

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **Q01 Source Execution/Batch Protocol** | Final Source execution seam, opaque continuation, post-batch exact `remaining`, protocol validation, production/scripted adapters, and child allowance/report handoff; non-productive. | S01, O03 | L |
| **Q02 Candidate Resolution Core** | Source-scoped state machine, private non-double-counting parent allowance, minimal Detail rounds, final rules, exact counts/usage/samples, sole `FinalizedCandidate` constructor, Complete/Partial/Abort. | Q01 | L/high |
| **Q03 Search Run Resolution Activation** | Moves Search Run to Q02; exposes Source/Search Run resolution summaries; deletes vector executor, duplicate counters/fakes, raw Candidate artifact, and old SCHOTT expectations. | Q02 | L atomic caller cut |

Q02 contains no Retry dimension. Mid-candidate budget exhaustion yields unresolved; emitted unstarted candidates become `budgetSkipped`; Cancellation releases no Resolution.

### 3.8 Finalized merge and persistence

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **DB01 Finalized Merge Boundary** | Non-productive final conversion from `FinalizedCandidate` plus two-stage identity/cross-Source merge behavior exists through its final interface; the old productive constructors remain until DB03. | Q02 | M |
| **DB02 Atomic SQLite Foundation** | Final `search_runs`/`matches` schema and one transaction implementation over migrated temporary SQLite; not yet called productively. | DB01 | L |
| **DB03 Persistence Activation** | Routes Q03 results through DB01/DB02; makes the finalized conversion the sole productive path; narrows constructors; atomically persists run/postings/sources/matches/last-run; deletes bypass/raw artifacts; and supersedes conflicting ADR 0008 clauses. | DB02, Q03 | L atomic |

### 3.9 Global Primitive gate

| Label | Observable outcome | Direct blockers | Risk |
|---|---|---|---|
| **G02 Primitive Completeness Gate** | Implementation-free Schema/Serde/compiled-registration parity, synthetic missing/duplicate failures, exactly-one canonical owner, no Primitive behavior in dispatch, and known duplicate-residue checks. | P01–P05, P06a–P06c, P07–P11, B03, D02, D03, A02 | M |

G02 blocks #166 completion, not consumers that use only a subset of families.

## 4. Current-to-target map

The action is the primary restructuring action for the current ticket. No whole current ticket is dropped: each contains a retained contract. Obsolete clauses and capabilities are dropped/deferred explicitly.

| Current ticket | Action | Target owner(s) | Retained/moved/discarded work and deletion owner |
|---|---|---|---|
| **T1 / #167** | **Split** | C01, A01 | Retain final authoritative compiler interface in C01. Move all productive caller migration, old facade deletion, and authored activation to A01. Drop productive scalar specialization beside `sourceOverrides`. |
| **T2 / #168** | **Keep** | C02 | Retain keyed merge/order/array/null/Diagnostics; delete scalar-only special cases in C02; activate only at A01. |
| **T3a / #169** | **Keep** | C03 | Retain complete additions/completeness/selection; remove unknown-new-key transitional rejection. |
| **T3b / #170** | **Keep** | C04 | Retain constrained schema/shared validator; correct stale T3a ownership attribution; delete duplicate compiler/Detection interpreters. |
| **T4a / #171** | **Move** | C07 | Move after C05→C06; retain complete provenance once with Policy/final names; drop temporary/implicit Policy origins. |
| **T4b / #175** | **Move** | C08, A01 | Build canonical fingerprints before activation; A01 alone wires them and deletes old freshness. |
| **T5 / #172** | **Move** | C05 | Retain mandatory Policy after C04 and before C06/C07; R01 later centralizes execution. |
| **T6 / #173** | **Move** | C06 | Retain direct final-name cut before provenance; A01 later removes authored v2 vocabulary. |
| **T7 / #174** | **Merge** | A01 | Merge authored hard cut with T1 caller migration, T4b wiring, complete Source/UI/schema/fixture/docs migration, and all inventory deletion. Do not re-own Policy. |
| **T8 / #176** | **Keep** | R01 | Retain one private kernel/typed Cancellation; delete duplicate loops/wrappers. |
| **T9 / #177** | **Keep** | R02 | Retain cumulative safety ceilings/reports. Move common phase envelope to O03. **Defer** Retry/Pacing; drop Bot-Detection claims and resettable counters. |
| **T10 / #178** | **Split** | H01, P09, A01 | H01 becomes phase-neutral HTTP acquisition/decoder usable by Detection; P09 owns authored Fetch and phase adapters. A01 removes the current Retry ghost before schema-v3 activation; P09 proves it stays absent. Drop phase-only seam assumptions. |
| **T11a / #179** | **Keep** | P02 | Retain Parse only; global completeness moves to G02. |
| **T11b / #180** | **Keep** | P03 | Retain Select and selected-item consolidation; correct predecessor reference. |
| **T11c / #192** | **Split** | P06a, P06b, P06c | Split the oversized Value work only along retained final seams: typed contexts/bounds, direct lookups, then recursive composition/`first_non_empty`. Cardinality, Transform, Predicate, Capture, and Template remain explicit owners rather than hidden T11c scope. |
| **T12a / #193** | **Keep** | O01 | Retain occurrence/value/hint/identity trust boundary; remove old DTOs and any hint promotion. |
| **T12b / #195** | **Split** | O02, O03 | O02 owns patches/reducers/provenance; O03 owns D-003/D-010 shared outcome/report/cause/commit and caller migration. Drop the old reduced-prefix-on-budget contract by explicit D-003 replacement. |
| **T13a / #202** | **Keep** | K01 | Policy variant/transition/Diagnostic/tests only; drop first-lander result migration and add common pre-commit Cancellation proof. |
| **T13b / #203** | **Keep** | K02 | Threshold Policy only; drop sibling landing-order ownership. |
| **T13c / #204** | **Keep** | K03 | Execute-all Policy only; reuse O02/O03; drop duplicate union/result ownership. |
| **T14a / #205** | **Split** | D02, A02 | Retain final URL/HTTP Strategies in D02 after real Template/Predicate/Capture/HTTP owners. Move productive migration/deletion to A02. Drop transitional capture overwrite. |
| **T14b / #206** | **Move** | D01 | State/reducer/validator/proposal foundation must precede D02/D03. Drop lossy Browser aggregate translation and second mutable maps. |
| **T14c / #207** | **Split** | B01, B02, B03, D03, A02 | Separate shared lifecycle/adapters, posting-phase Browser primitives/adapters, Detection Strategy, and cross-phase activation. Drop public teardown residue, cleanup-success assumptions, and impossible recovered fallback. |
| **T14d / #218** | **Keep** | G01 | Guard/residue proof only. Drop known migration/deletion and impossible fallback case. |
| **T15 / #219** | **Keep** | S01 | Retain Source Detail capability/reuse/routing/callers; replace result with D-008/D-010 and drop Diagnostic-derived control/false `Unavailable`. |
| **T16 / #233** | **Split** | Q01, Q02, Q03 | Separate batch/execution protocol, deep resolver, and productive Search Run cut. **Defer** Retry/Pacing. Drop vector executor/raw artifacts/URL-derived SCHOTT canonical values. |
| **T17 / #234** | **Split** | DB01, DB02, DB03 | Separate dormant finalized conversion/merge, tested transaction foundation, and productive persistence/ADR cut. DB03 alone narrows productive constructors and deletes non-atomic import/bypasses. |

### Explicitly deferred or dropped work

- **Defer, no target ticket now:** executable Retry, pacing/rate limiting, `Retry-After`, concurrency policy. A future capability requires new evidence and approval under D-002/D-012.
- **Drop:** compatibility wrappers/translators/aliases; productive intermediate specialization/Detection routes; T13 first-lander clauses; public Browser teardown residue; impossible Detection recovered-later-Strategy cases; Retry placeholders; prohibited Browser script/login/CAPTCHA variants; Transform aliases; unsupported `maxErrorRatio`; raw Candidate artifacts; URL-derived canonical hints.

## 5. Proposed direct dependency DAG

```text
C01 → C02 → C03 → C04 → C05 → C06 → C07 → C08 → A01
                                                          │
                         ┌────────────────────────────────┼→ R01 → R02 → H01
                         │                                │           │
                         │                                ├→ P01      ├→ B01 → B02
                         │                                └→ D01      │      └→ B03
                         │                                            │
H01 → P02 → P03 → P04 ─┐
                 ├→ P05 ├→ P06a → P06b → P06c → P07
                 └──────┘                         └→ P08
P01 ───────────────────────────────────────→ P06c

H01 + P01 → P09
P09 + B03 + P03 + R02 → P10

P06c → O01 → O02
P07 + O01 + O02 → P11
R02 + O02 + P11 → O03 → K01 / K02 / K03
                         └→ S01 → Q01 → Q02 → Q03

D01 + P01 + P07 + P08 + P09 + K01 → D02
D01 + R02 + B01 + B03 + P07 + P08 + K01 → D03
D02 + D03 + B02 + B03 + Q03 → A02 → G01

Q02 → DB01 → DB02
Q03 + DB02 → DB03

P01..P05 + P06a..P06c + P07..P11 + B03 + D02 + D03 + A02 → G02

#166 completion waits for: G01, G02, K02, K03, DB03
```

### Why the non-obvious edges are real

- **C04→C05→C06→C07→C08:** D-011 requires final schema, Policy, names, provenance, and fingerprints exactly once before activation.
- **H01→P09→D02:** Detection needs a phase-neutral HTTP acquisition and canonical Fetch behavior; current T10 does not provide it.
- **P06c→P07/P08→D02/D03:** Detection consumes real Value-backed predicates/captures rather than assuming one Value ticket produced them.
- **O02/P11/R02→O03:** the common outcome commits reduced payloads, attempt acceptance classification, and exact reports; no sibling Policy owns it.
- **Q03→A02:** Browser activation targets the final Search Run caller once, avoiding migration of the vector executor that Q03 deletes.
- **Q02→DB01 and Q03+DB02→DB03:** the dormant persistence foundation consumes the final finalized type, while DB03 alone makes that conversion productive after the final Search Run result exists.
- **A02→G02:** the gate verifies old Browser/Detection duplicate dispatch is actually gone; it cannot inherit deletion work.

### Parallelizable branches

- After A01: R01, P01, and D01.
- After R02: H01 and B01.
- After P03: P04, P05, and P06a.
- After P06c: P07 and P08.
- After O03: K01, K02, K03, and S01.
- After B01: B02 and B03.
- After Q02: Q03 and DB01.
- After Q03 and their other prerequisites: A02 and DB03 are semantically parallel, but both touch Search Run and should be scheduled serially or merge-coordinated without inventing a false dependency.

## 6. Decision enforcement matrix

| Decision | Target owner/enforcement point |
|---|---|
| **D-001** | C01–C08 final foundations; A01 sole productive Source activation and complete deletion. |
| **D-002** | R02/H01/B01–B03/Q02 describe safety ceilings only; Retry/Pacing deferred. |
| **D-003** | O03 sole phase outcome/commit owner; K01–K03/S01 reuse it; old T12b prefix explicitly replaced. |
| **D-004** | C05→C06→C07; no implicit/temporary Policy provenance. |
| **D-005** | P01–P05, P06a–P06c, P07–P11, B03, D02/D03 explicit owners; G02 global implementation-free gate. |
| **D-006** | D01 precedes D02/D03; A02 activates only native ordered contributions/reconciled state. |
| **D-007** | B01/B02/B03 foundations; A02 migrates all productive callers/deletes old seam; G01 guard-only. |
| **D-008** | O03 owns unsatisfied cause; S01 owns Source Detail outcome; Q02 owns typed mapping. |
| **D-009** | Q01 protocol/report handoff; Q02 private parent allowance/counts/post-batch `remaining`. |
| **D-010** | R02 report; O03 envelope; S01 exact projection; Q02 one-time child-report commit. |
| **D-011** | C01→C08→A01 exact serial chain. |
| **D-012** | A01 removes the current authored/Serde/compiled Retry ghost before initial schema-v3 activation; P09/R02/Q02 contain no Retry dimension; canonical PRD updates land with the behavior slice. |
| **D-013** | Q02 finalized constructor; DB01 boundary; DB02 transaction; DB03 sole productive persistence/ADR/artifact cut. |

Every original contract has a target owner or an accepted replacement. The former T12b contract is explicitly replaced by D-003 at O03; it is not silently lost.

## 7. Activation and deletion ownership

### A01 Source activation

Owns every row in `issue-166-source-overrides-cut-inventory.md`:

- Source Rust/Schema/TS/UI authored surfaces and removal of the current Retry Serde/plan ghost before initial schema-v3 activation;
- all four productive compiler callers plus registry/commands;
- canonical fingerprints and old freshness deletion;
- fixtures/tests/resources and active domain/ADR/docs;
- old types/modules/functions/exports/diagnostics/editor/helpers.

C01–C08 must leave A01 with wiring, final authored-surface switch, fixture/doc migration, and deletion—not compiler design work.

### A02 Browser/Detection activation

Owns every productive/deletion row in `issue-166-browser-cut-inventory.md`:

- three old seam leaf calls and six managed construction sites;
- all old trait implementations/fakes/DTOs/exports/convenience wrappers;
- Source Live Check, final Search Run, posting/UI, commands, runtime smoke/admin internals;
- old URL/HTTP/Browser Detection evaluators, mutable maps, proposal builders;
- missing Browser parity tests and exact residue searches.

B01–B03/D01–D03 must remove lifecycle/phase/Detection uncertainty before A02 starts.

### Family slices and gates

Each P-family owns same-slice duplicate deletion. G01/G02 may detect residue but never inherit known cleanup.

### Q03 and DB03

Q03 alone deletes the old Search Run vector/candidate execution path. DB03 alone activates finalized-only atomic persistence and supersedes ADR 0008’s conflicting clauses.

## 8. Documentation ownership

- **A01:** `CONTEXT.md`, older Source Profile PRD, ADR 0001/0009, production-agent guidance, schema-v3 Source examples.
- **R02/P09/Q02:** safety-ceiling/conditional-Retry language and PRD sample limit 10 in the behavior slice that makes it true.
- **O01/S01:** Posting Occurrence, provider/hint, requested Detail vocabulary in `CONTEXT.md`/canonical PRD.
- **Q03:** bounded Resolution smoke summaries, no raw Candidate artifact, no URL-derived SCHOTT canonical values.
- **DB03:** ADR 0008 supersession and durable Match/run smoke assertions.
- **After target-map approval, local metadata only:** update `handoff/README.md` and remove the missing-template reference. Do not create a new ticket template yet.

## 9. Scope risks

### Atomic cuts

- **A01 is XL:** backend documents/compiler callers, schema, frontend Create/Edit/Details, fingerprints, fixtures, tests, and active docs. It cannot be productively split without dual Source routes. Readiness requires every final foundation and a row-by-row inventory checklist.
- **A02 is XL:** all phases, six production constructors, runtime smoke, commands, duplicate fakes, and Detection deletion. It cannot be productively split without dual Browser seams. B01–B03/D01–D03/Q03 must leave only caller switching/deletion/parity.

### High-risk foundations

- **B02:** current Chromium close/handler teardown is outside timeout and no forced reap path is proven. Feasibility must be investigated at B02 readiness; failure blocks A02 and must not weaken D-007 silently.
- **Primitive admission:** `text`, Template registration identity, nested-option classification, split option names, named capture semantics, and `maxErrorRatio` removal must be resolved against final schema/Serde within their assigned owners before G02 freezes the set.
- **O03/S01/Q02:** exhaustive terminal/report mapping is broad. Tests must trace Accepted, PolicyUnsatisfied, BudgetExhausted, candidate/source failure, mismatch, and Cancellation without Diagnostic parsing.
- **P06a/P06b/P06c:** Value migration is split along retained typed-context, direct-lookup, and composition seams; no public generic Value executor or raw-expression bridge is introduced.
- **Q02/Q03:** Candidate core and Search Run activation are separated to avoid an unreliable XL ticket, but Q02 must remain one deep Source-scoped operation rather than exposing a public per-candidate loop.
- **DB02/DB03:** schema/transaction foundation is separable; productive persistence remains one atomic cut.

### Worktree and external data

- Existing unrelated staged/modified files must remain untouched and be re-baselined per implementation slice.
- External app-data Source JSON cannot be inventoried. D-001 requires strict rejection/manual recreation, not migration.

## 10. Residual uncertainties

These do **not** reopen D-001–D-013, but must be resolved at target-ticket readiness:

1. exact final Rust/TS module/type names;
2. whether Template is a registered Primitive identity or compiler infrastructure with exhaustive parity ownership;
3. whether pagination parameter location and Browser wait/interaction subtypes are separate registrations or parent-family options;
4. final `text` Parse admission/placement wording under T11a;
5. exact named-capture semantics after removing unnamed fallback;
6. direct Serde parity for Transform split fields and other requiredness mismatches;
7. practical forced terminate/reap mechanism in the pinned Browser stack;
8. safe classification of bounded filesystem cleanup residue versus inability to establish process cleanup;
9. exact representation for phases with no Browser Strategy after `UnavailableProfileBrowserClient` deletion;
10. implementation scheduling for A02 and DB03 because they touch overlapping Search Run files without a semantic dependency.

Any resolution that would require a compatibility path, productive dual route, Retry placeholder, public teardown residue, payload on budget/failure/Cancellation, hint promotion, or persistence of non-final values must stop and explicitly request reopening the relevant accepted decision.

## 11. Validation before ticket-body rewriting

Before rewriting any Lean body or proposing GitHub changes:

1. map every inventory row to A01, A02, a P-family, Q03, DB03, G01, or G02;
2. verify each original-ticket contract against the reconciliation and this owner matrix;
3. generate the direct edge list and prove each blocker produces the consumed interface;
4. verify no guard owns known deletion;
5. trace all phase/Source Detail/Candidate terminals and exact reports end to end;
6. trace only `SourceResolution.finalized` into DB01/DB03;
7. review A01/A02 scope checklists and missing parity tests;
8. refresh live GitHub once more immediately before any tracker edit.

## 12. Review gate

This proposal intentionally stops before:

- rewriting any existing lean ticket;
- creating a new ticket template;
- editing GitHub bodies, labels, parents, or dependencies;
- implementing product code.

The next step requires user review of the target catalogue, 27-ticket action map, DAG, atomic-cut sizing, Primitive-family granularity, and residual uncertainties.
