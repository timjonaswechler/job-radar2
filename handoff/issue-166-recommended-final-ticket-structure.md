# Issue #166 — Recommended Final Ticket Structure

Status: **local restructuring recommendation for user approval — no Lean or GitHub changes approved**  
Baseline: **2026-07-18**

## 1. Review scope and live baseline

This document reviews the 47-slice upper bound in `handoff/issue-166-restructuring-plan.md` under fixed decisions D-001–D-013. It applies the architecture-language criteria for a module, interface, seam, adapter, test seam, deletion test, and deepening-candidate status.

Read-only live refresh confirms:

- #166 and all 27 implementation issues remain open;
- all implementation issues remain native children of #166;
- implementation issues have no comments; #166 has one historical comment;
- only #167 carries `ready-for-agent`;
- native dependencies still match the old ticket index;
- no GitHub state was changed.

The current #167 label is stale under D-001/D-011, but changing it requires a separately approved tracker workflow.

## 2. Recommendation

Adopt **42 final local implementation tickets**, five fewer than the 47-slice upper bound.

The reduction is not a ticket-count target. It follows six structural corrections:

| Proposed slices | Final disposition | Net effect | Reason |
|---|---|---:|---|
| C02 + C03 | **C02** | -1 | Existing-key merge and complete absent-key additions are two branches of one private keyed merger, one compiler test surface, and one deletion owner. Keeping them separate lands a temporary unknown-new-key rejection. |
| P06b + P06c | **P06bc** | -1 | Direct Value lookups and recursive composition use one compiled Value evaluator and jointly delete the raw/mixed runtime path. |
| B03 | **B03a + B03b** | +1 | Authored Browser Fetch/wait/interaction Primitive ownership is independent from O03-dependent Discovery/Detail phase projection. The old combined node creates false blockers for Pagination and Browser Detection. |
| A02 + G01 | **A02** | -1 | A residue guard is the same-slice deletion proof for A02, not a later implementation ticket. G01 has no independent module or deletion-test result. |
| Q01 + Q02 | **Q01** | -1 | Batch protocol, parent allowance, candidate state machine, and finalization form one Source-scoped Candidate Resolution interface and test surface. |
| Q03 + proposed DB01 + DB03 | **A03** | -2 | Search Run activation, finalized conversion/merge-input narrowing, productive persistence, bypass deletion, smoke/artifacts, and ADR supersession must switch together to avoid a temporary productive conversion. |
| Proposed DB02 | **DB01** | 0 | The SQLite schema/transaction remains an independently testable non-productive foundation. |

Arithmetic: `47 - 1 - 1 + 1 - 1 - 1 - 2 = 42`.

### Consolidations explicitly rejected

- **C01+C02:** compiler authority/result/lifecycle behavior has an independent caller-facing interface and test seam. Merge C02+C03 instead, where the temporary contract exists.
- **R01+R02:** policy transitions/Cancellation and cumulative allowance/report accounting have distinct interfaces, test surfaces, effect sites, and deletion proofs.
- **P06a+P06b:** typed context compilation is a retained compiler foundation; merging lookups with composition avoids the actual mixed runtime route.
- **K02+K03:** `at_least` and `collect_all` have independently orderable and observably different stopping semantics; shared enum files are merge coordination, not a semantic module.
- **B01+B02:** the scripted contract can land and unblock final Primitive work while managed-process teardown feasibility is proven separately.
- **H01+P09:** transport/decoding is a true external seam; authored HTTP Fetch is a separate Primitive family consuming it.
- **O02+O03:** reducers/patches and the closed report-bearing public phase outcome have distinct responsibilities and migrations.
- **Any productive split of A01, A02, or A03:** each would create a dual productive route, temporary compatibility path, or known cleanup-later state.

## 3. Final 42-ticket catalogue

All named modules remain **deepening candidates** until implementation evidence proves their interface, callers, adapters where justified, deletion test, and public test seam.

### 3.1 Compiler and Source activation — 8 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **C01** | Authoritative Effective Profile Compiler Boundary | — | M | Final `compile_source(authoritative Source, immutable registry)` outcome, lifecycle exclusion, Profile/Source-owned branches, no-specialization parity. No productive schema-v3 authoring or caller switch. |
| **C02** | Deterministic Effective Profile Merge and Complete Additions | C01 | L | One private keyed merger covers recursive existing entries and complete new Strategies/Access Paths, deterministic order, whole-array replacement, completeness, selected/unselected validation; deletes scalar/special-case and temporary unknown-key paths. |
| **C04** | Effective Source Config Contract | C02 | L | One constrained compiler/value validator shared by Compiler and Detection, including neutral violations and later incremental/final operations; deletes duplicate interpreters. |
| **C05** | Final Mandatory `first_accepted` Policy Foundation | C04 | M | Final typed authored/schema fragment and compiled Policy through the dormant compiler interface; current fallback parity becomes explicit. It is not productively schema-v3-authorable before A01. |
| **C06** | Final Internal and Compiled Phase Names | C05 | L/high change-volume | One direct internal/compiled `detection`/`discovery`/`detail` rename across modules, plans, exports, callers, tests, and diagnostics; no aliases. A01 later removes authored v2 vocabulary. |
| **C07** | Complete Effective Profile Provenance | C06 | L | Final-name, Policy-complete, schema-terminal provenance recorded in the compiler pipeline; no replay/diff, temporary paths, or value leakage. |
| **C08** | Canonical Schema-v3 Fingerprint Foundation | C07 | L | Final typed projections, component order/counts, runtime bindings, versions/globals, exclusions and SHA-256 preparation; not productively wired until A01. |
| **A01** | Atomic Source Schema-v3 Activation and Old-path Removal | C08 | **XL atomic** | Sole productive Source activation: backend/frontend/schema/UI/callers/fingerprints/fixtures/docs; strict old JSON rejection; Retry ghost removal; complete `sourceOverrides` and old compiler/freshness deletion. |

Compiler chain:

```text
C01 → C02 → C04 → C05 → C06 → C07 → C08 → A01
```

### 3.2 Runtime and HTTP acquisition — 3 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **R01** | Typed Strategy Set Kernel | A01 | L | One crate-private ordered-attempt kernel, typed Cancellation, deterministic stop behavior; deletes duplicate Discovery/Detail policy loops and Diagnostic-derived control. |
| **R02** | Cumulative Phase Allowances and Complete Reports | R01 | M | Tighten-only safety ceilings, atomic debit-before-effect, monotonic deadline, exact usage/report on every started terminal, no Retry dimension; deletes resettable/per-Strategy budget models. |
| **H01** | Byte-preserving Phase-neutral HTTP Acquisition and Strict Decoding | R02 | M | Domain-owned acquisition seam with production/scripted adapters, bounded bytes, response metadata, strict decoding, typed transport failure and sanitization; deletes phase-specific clients/DTOs/fakes and premature decoding. |

### 3.3 Primitive-family owners — 12 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **P01** | Canonical Template Grammar | A01 | M | One context-neutral compiled grammar/renderer; phase owners retain namespace admission; deletes duplicate parser/render wrappers. Readiness freezes whether this is a registered Primitive identity or exhaustively owned compiler infrastructure. |
| **P02** | Canonical Parse Primitives | H01 | M | JSON/XML/HTML, one Parsed Document, typed decoded-HTTP versus rendered-Browser input; deletes both phase parser switches. `text` must be removed from schema-v3 admission or become genuinely executable—never a permanent rejection registration. |
| **P03** | Canonical Select Primitives | P02 | L | Six selectors, shared selected-item model, syntax/context/placement checks; deletes phase/sitemap switches and duplicate traversal. |
| **P04** | Canonical Cardinality Primitives | P03 | M | `one`, `first`, `optional`, `all` over the landed selected/value sequence; deletes both phase cardinality implementations. The P03 edge must name the exact consumed sequence type at readiness. |
| **P05** | Canonical Transform Primitives | P03 | M | Ten canonical snake_case transforms and typed plans; deletes central raw transform dispatch and aliases. Readiness fixes split Serde names, `to_string`, and `url_decode` semantics. |
| **P06a** | Typed Value Context Foundation | P03 | M | Typed placement contexts, static availability, namespace admission, scalar/sequence support, and expression depth/node/candidate bounds; no claim that runtime raw expressions are already gone. |
| **P06bc** | Complete Value Execution and Composition | P06a, P04, P05, P01 | M/high | One compiled Value registry/evaluator for direct lookups, Value templates, combine/list, and bounded `first_non_empty`; deletes raw `FieldExpression` plans and all phase-local lookup/composition paths. |
| **P07** | Canonical Predicate Primitives | P06bc | M | Closed compiled Predicate set and syntax/context validation; deletes phase filter/equality switches and migrated Detection predicate behavior. Readiness freezes ownership of Detection `contains`/status checks. |
| **P08** | Canonical Named Capture Primitive | P06bc | M | One named regex-capture engine with immutable complete-map input and ordered outputs; deletes phase and migrated Detection capture loops, unnamed fallback, partial-map chaining and overwrite behavior. |
| **P09** | Canonical Authored HTTP Fetch | H01, P01 | M | GET/POST and JSON/text/form bodies over H01, typed templates and adapters, no Retry; deletes authored Fetch/compiler/runtime duplicate switches. Productive old Detection deletion remains A02-owned. |
| **P10** | Canonical Bounded HTTP/Browser Pagination | P09, B03a, P03, R02 | M | Page/offset/cursor/sitemap and parameter-location behavior over canonical Fetch/Select/allowance interfaces; deletes monolithic pagination dispatch and hidden total/cursor helpers. |
| **P11** | Canonical Phase Acceptance | O01, O02 | L/M | Retained `requiredFields`, `minDescriptionLength`, and context-valid `minResults`; deletes duplicate phase acceptance/stubs and raw plans. `maxErrorRatio` is absent unless separately evidenced. Add P07 as a blocker only if the final implementation genuinely consumes compiled Predicate plans. |

Primitive flow:

```text
H01 → P02 → P03 → P04 / P05 / P06a
P03 + P04 + P05 + P01 → P06bc
P06bc → P07 / P08 / O01
H01 + P01 → P09
P09 + B03a + P03 + R02 → P10
O01 + O02 → P11
```

### 3.4 Posting outputs, shared outcomes, Policies, and Source Detail — 7 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **O01** | Source-local Posting Occurrences and Discovery Value Semantics | P06bc | L/high integration | Sole occurrence model, provider-ID/normalized-URL identity, disjoint provider values/hints/postingMeta, no hint promotion; migrates old occurrence DTOs and temporary bridge; owns domain vocabulary. |
| **O02** | Requested Detail Patches and Conflict-safe Phase Reducers | O01 | L/high integration | Four-field requested-only patch, deterministic Discovery/Detail reducers, contribution provenance, conflicts/rejections; deletes duplicate accumulation/last-write behavior. |
| **O03** | Shared Discovery/Detail Phase Outcome and Commit Boundary | R02, O02, P11 | L/high atomic API cut | Exclusive D-003/D-010 owner: report-bearing outcomes/Cancellation, `PolicyUnsatisfiedCause`, reducer attachment, pre-commit Cancellation, exhaustive caller/export migration; only Accepted has payload. |
| **K01** | `all_required` Strategy Policy | O03 | M | Variant-specific universal fail-fast transition, exact Diagnostic/tests; no shared-result migration. Detection consumes this Policy. |
| **K02** | `at_least(count)` Strategy Policy | O03 | M | Positive threshold, earliest success/impossibility, exact Diagnostic/tests; independent of K03. |
| **K03** | `collect_all(minAccepted)` Strategy Policy | O03 | M | Execute-all/natural-completion semantics, exact Diagnostic/tests; independent of K02. |
| **S01** | Candidate-scoped Source Detail Outcome, Routing, and Execution Seam | O03 | L/high integration | Closed D-008/D-010 outcome, exact report projection, source validation, reuse/capability routing, dispositions only on Completed, **one unchanged full Strategy Set/Policy invocation**, production+scripted implementations, UI/Live Check migration, and same-slice deletion of the description-era public Detail operation/result family. |

### 3.5 Browser and Detection — 8 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **B01** | Browser Acquisition Contract and Scripted Adapter | R02 | M | Phase-neutral lifecycle/control/usage contract, typed infrastructure failure, teardown-before-return Cancellation, deterministic scripted adapter; no public teardown residue. |
| **B02** | Managed Browser Acquisition Adapter | B01 | M/high feasibility | Real Chromium launch/navigation/content plus bounded close→kill/reap→handler/session finalization and runtime-admin smoke parity. Must prove the pinned stack can establish the cleanup invariant. |
| **B03a** | Canonical Browser Fetch, Wait, and Interaction Primitives | B01, P01, R02 | M | Authored Browser Fetch/waits/interactions and compiled behavior; removes prohibited Serde-only script/eval/DOM/login/CAPTCHA variants and family-local nonproductive duplicates. |
| **B03b** | Discovery and Detail Browser Phase Adapters | B03a, O03 | M | Thin typed posting-phase adapters with exact allowance/report/outcome/Cancellation projection; does not own process lifecycle or authored Primitive semantics. |
| **D01** | Reconciled Detection State | A01 | M | Ordered contributions, conflict-safe incremental reducer, immutable state, C04 incremental/final validation, sole proposal construction; no aggregate translation. |
| **D02** | URL and HTTP Detection Strategies | D01, P01, P07, P08, P09, K01 | M | Consume reconciled state and emit native ordered contributions under `all_required`; no productive old-route deletion. |
| **D03** | Browser Detection Strategy | D01, R02, B01, B03a, P07, P08, K01 | M/high | Native contributions, invocation/profile/Strategy ceilings, rendered-byte checks and Detection projection; no process-lifecycle or posting-adapter ownership. |
| **A02** | Atomic Browser/Detection Activation and Residue Proof | D02, D03, B02, B03b | **XL atomic** | Migrates every productive Browser/Detection caller/test, replaces all fakes/construction sites, deletes complete old seam/mutable Detection route, and runs the call-graph/residue guard in the same slice. |

Important deletion split:

- **B03a** owns final authored/compiled Primitive behavior and only nonproductive family-local duplicate cleanup.
- **A02** exclusively owns productive `ProfileBrowserClient`, phase fetch branches, construction sites, convenience operations, exports, fakes, old runtime DTOs and old Detection evaluators.
- No later guard inherits known deletion.

### 3.6 Candidate Resolution and persistence — 3 tickets

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **Q01** | Resolve Source Candidates through one Bounded Batch Operation | S01, O03 | L/high | One Source-scoped deepening candidate: production/scripted Source execution, opaque continuation, protocol/post-batch `remaining`, private non-double-counting parent allowance, minimal Detail rounds, state machine, normalization/final rules, counts/samples, and sole normal `FinalizedCandidate` construction. No public per-candidate loop or Retry dimension. |
| **DB01** | Atomic Search Run/Match SQLite Transaction Foundation | Q01 | L | Final `search_runs`/`matches` schema and one internal transaction over real migrated temporary SQLite: IDs, constraints, rollback, reruns, retention, cascades. Accepts only already merged persistence input and terminal run facts; no merge or Candidate logic. |
| **A03** | Activate Finalized-only Search Run Resolution, Merge, and Persistence | Q01, DB01 | **XL atomic** | Moves Search Run to Q01; sole productive conversion from `SourceResolution.finalized`; retargets existing cross-Source merge without duplicate merger; invokes DB01 once; narrows constructors; deletes vector executor/broad inputs/counters/fakes/bypasses/raw artifacts; updates SCHOTT/bounded summaries; supersedes conflicting ADR 0008 clauses. |

A03 prevents an intermediate productive Search Run that converts final values back into the old broad merge/import model.

### 3.7 Global convergence — 1 ticket

| Label | Outcome title | Direct blockers | Risk | Interface/test/deletion focus |
|---|---|---|---|---|
| **G02** | Global Primitive Completeness Gate | P01, P02, P03, P04, P05, P06a, P06bc, P07, P08, P09, P10, P11, B03a, D02, D03, A02 | M | Implementation-free schema/Serde/compiled registration parity, synthetic missing/duplicate failures, exactly-one canonical owner, no behavior in dispatch, and known-residue proof. Owns no known migration/deletion. |

## 4. Complete semantic dependency DAG

An edge below means the downstream ticket directly consumes an interface or contract produced by the upstream ticket.

```text
C01 -> C02
C02 -> C04
C04 -> C05
C05 -> C06
C06 -> C07
C07 -> C08
C08 -> A01

A01 -> R01
R01 -> R02
R02 -> H01

A01 -> P01
H01 -> P02
P02 -> P03
P03 -> P04
P03 -> P05
P03 -> P06a
P06a -> P06bc
P04 -> P06bc
P05 -> P06bc
P01 -> P06bc
P06bc -> P07
P06bc -> P08
P06bc -> O01
H01 -> P09
P01 -> P09
P09 -> P10
B03a -> P10
P03 -> P10
R02 -> P10
O01 -> O02
O01 -> P11
O02 -> P11
R02 -> O03
O02 -> O03
P11 -> O03
O03 -> K01
O03 -> K02
O03 -> K03
O03 -> S01

R02 -> B01
B01 -> B02
B01 -> B03a
P01 -> B03a
R02 -> B03a
B03a -> B03b
O03 -> B03b

A01 -> D01
D01 -> D02
P01 -> D02
P07 -> D02
P08 -> D02
P09 -> D02
K01 -> D02
D01 -> D03
R02 -> D03
B01 -> D03
B03a -> D03
P07 -> D03
P08 -> D03
K01 -> D03
D02 -> A02
D03 -> A02
B02 -> A02
B03b -> A02

S01 -> Q01
O03 -> Q01
Q01 -> DB01
Q01 -> A03
DB01 -> A03

P01 -> G02
P02 -> G02
P03 -> G02
P04 -> G02
P05 -> G02
P06a -> G02
P06bc -> G02
P07 -> G02
P08 -> G02
P09 -> G02
P10 -> G02
P11 -> G02
B03a -> G02
D02 -> G02
D03 -> G02
A02 -> G02
```

Series completion waits for the semantic sinks:

```text
K02 + K03 + A03 + G02 -> #166 completion
```

K01 is consumed by Detection and therefore reaches G02 through A02.

### Scheduling constraints that are not semantic dependencies

1. **Prefer A03 before A02.** This lets A02 migrate the final Search Run Browser caller once. Do not publish an A03→A02 or A02→A03 native dependency: neither consumes the other’s domain interface.
2. **A02 and A03 overlap Search Run files.** Schedule serially on fresh baselines or merge-coordinate; file overlap is not a dependency.
3. Merged slices C02, P06bc, Q01, and A03 preserve internal responsibility order in their checklists; consolidation removes tracker handoffs, not semantic sequencing.
4. K01/K02/K03 may be semantically parallel after O03, but their shared closed enum/compiler/kernel files require merge coordination.

## 5. Current-to-Final mapping for all 27 issues

| Current ticket | Primary action | Final owner(s) | Retained/moved/replaced work |
|---|---|---|---|
| **T1 / #167** | Split | C01, A01 | C01 retains compiler authority/result; A01 owns every productive caller migration, old facade deletion and authoring activation. Drop productive scalar dual-route transition. |
| **T2 / #168** | Merge | C02 | Existing-key merge joins T3a additions in one final keyed merger. |
| **T3a / #169** | Merge | C02 | Complete additions become the absent-key branch of C02; no temporary unknown-key rejection. |
| **T3b / #170** | Keep | C04 | Constrained schema/shared validator; correct stale T3a ownership attribution. |
| **T4a / #171** | Move | C07 | Lands after Policy and final names; complete provenance once. |
| **T4b / #175** | Move/Split | C08, A01 | C08 builds dormant canonical fingerprints; A01 wires them and deletes old freshness. |
| **T5 / #172** | Move | C05 | Mandatory Policy foundation before final names/provenance; no productive schema-v3 authoring before A01. |
| **T6 / #173** | Move | C06 | Final internal names before provenance; authored-v2 deletion remains A01. |
| **T7 / #174** | Merge | A01 | Becomes the atomic Source activation with T1 caller cut, T4b wiring, UI/schema/docs migration, Retry-ghost and old-path deletion. |
| **T8 / #176** | Keep | R01 | Typed Strategy Set kernel only. |
| **T9 / #177** | Keep | R02 | Phase safety ceilings and complete reports; no Retry/Pacing/Bot-Detection responsibility. |
| **T10 / #178** | Split | H01, P09, A01 | H01 transport/decoder; P09 authored HTTP Fetch; A01 removes Retry ghost. |
| **T11a / #179** | Keep | P02 | Parse family only; `text` requires clean final admission decision. |
| **T11b / #180** | Keep | P03 | Select and selected-item consolidation; correct predecessor reference. |
| **T11c / #192** | Split | P06a, P06bc | Typed Value contexts followed by complete lookup/composition evaluator; other ownerless Primitive families remain explicit P slices. |
| **T12a / #193** | Keep | O01 | Posting Occurrence/provider/hint identity and trust boundary. |
| **T12b / #195** | Split | O02, O03 | Reducers/patches versus exclusive shared outcome/report/commit migration; D-003 intentionally replaces prefix-on-budget output. |
| **T13a / #202** | Keep/Narrow | K01 | `all_required` transition/Diagnostic/tests only; remove first-lander work and add common commit proof. |
| **T13b / #203** | Keep/Narrow | K02 | `at_least` only; no sibling/result ownership. |
| **T13c / #204** | Keep/Narrow | K03 | `collect_all` only; no duplicate union/result ownership. |
| **T14a / #205** | Split | D02, A02 | Final URL/HTTP Strategies in D02; all productive migration/deletion in A02; drop transitional overwrite. |
| **T14b / #206** | Move | D01 | Reducer/state/validation/proposal foundation precedes all Strategies; drop aggregate translation. |
| **T14c / #207** | Split | B01, B02, B03a, B03b, D03, A02 | Shared lifecycle/adapters, Primitive ownership, phase adapters, Detection Strategy and one cross-phase activation; drop public residue and impossible fallback. |
| **T14d / #218** | Merge | A02 acceptance | Guard/residue proof becomes A02 same-slice completion evidence; owns no cleanup ticket. |
| **T15 / #219** | Keep/Replace outcome | S01 | Complete typed Source Detail seam, full-Policy routing, UI/Live Check migration and old Detail API deletion. |
| **T16 / #233** | Split/Consolidate | Q01, A03 | Batch protocol and Candidate core consolidate into Q01; productive Search Run cut/summaries/artifacts move to A03; Retry/Pacing deferred. |
| **T17 / #234** | Split/Consolidate | DB01, A03 | SQLite transaction foundation in DB01; finalized conversion/merge/persistence activation, constructor narrowing, artifacts and ADR cut in A03. |

No current issue is wholly dropped because each retains accepted behavior. Obsolete clauses and unsupported capabilities are removed as listed below.

## 6. Deferred and dropped work

### Deferred without a target ticket

- executable Retry;
- pacing/rate limiting;
- `Retry-After` handling;
- concurrency policy.

A future capability requires separate evidence, semantics, safety ceilings, accounting, Diagnostics, fingerprints, and dependencies under D-002/D-012.

### Explicitly removed from the initial target

- compatibility wrappers, translators, aliases and dual productive routes;
- T13 first-lander result migration;
- public Browser teardown residue;
- impossible recovered-later-Strategy Detection cases;
- Retry placeholders/always-zero counters;
- Serde-only script/eval/DOM/login/CAPTCHA interactions;
- Transform camelCase aliases;
- `maxErrorRatio` without truthful denominator evidence;
- raw Candidate artifacts;
- URL/hint promotion to canonical values;
- the published T12b reduced-prefix-on-budget payload, replaced explicitly by D-003.

## 7. Activation and same-slice deletion owners

### A01 — Source schema-v3 activation

Owns every row of `issue-166-source-overrides-cut-inventory.md`: Rust/Schema/TS/UI Source documents, compiler callers/facade/override implementation, commands/registry/validation, Search Run and lazy Detail compiler preparation, canonical freshness wiring and old fingerprint deletion, fixtures/tests/resources, active domain/PRD/ADR/agent docs, strict old JSON rejection, Retry ghost removal, and generated residue classification.

### A02 — Browser/Detection activation and proof

Owns every productive/deletion row of `issue-166-browser-cut-inventory.md`: three leaf calls, six managed construction sites, all old implementations/fakes/DTOs/exports/convenience operations, Source Live Check, final Search Run, posting/UI, commands, runtime-admin smoke, mutable Detection maps/evaluators/proposal builders, parity gaps, and the residue guard.

### A03 — Finalized Search Run/persistence activation

Owns the productive Search Run switch to Q01, sole finalized conversion, cross-Source merge-input narrowing, DB01 invocation, broad constructor/import bypass deletion, vector executor/counters/fakes/artifacts removal, bounded Resolution smoke updates, durable run/Match assertions, post-commit artifact boundary, and ADR 0008 supersession.

### Family/module slices

Each P/B/D/O/R/S/Q/DB foundation owns its named final behavior and same-slice local duplicate deletion. G02 verifies but never inherits known cleanup.

## 8. Documentation ownership

| Documentation/contract | Owner |
|---|---|
| `CONTEXT.md`, older Source Profile PRD, ADR 0001/0009, Source examples, production-agent guidance, schema-v3 activation vocabulary | A01 |
| Compiler/merge/schema/Policy/naming/provenance/fingerprint implementation details | C01–C08 respectively; A01 only activates final vocabulary and hard-cut docs |
| Phase safety ceilings, complete reports, no phase Retry dimension | R02 |
| HTTP byte/metadata/decoder/sanitization | H01 |
| Authored HTTP Fetch no-retry contract | P09 |
| Primitive catalog, registrations, admission/removal, Template, `text`, Transform aliases/fields, named captures, Acceptance keys | Corresponding P/B03a family; G02 documents only completeness procedure |
| Browser lifecycle, bounded teardown, phase-neutral seam and managed/scripted adapters | B01/B02; A02 updates productive caller/deletion language |
| Detection contributions, reconciled state, validation checkpoints and final Strategies | D01–D03; A02 updates productive-route language |
| Posting Occurrence/provider values/hints | O01 |
| Shared phase outcome/report/commit and Source Detail outcome | O03 and S01 |
| Policy variants | K01, K02, K03 |
| Candidate Resolution, parent allowance, post-batch `remaining`, sample limit 10, conditional future Retry wording | Q01 |
| Bounded Resolution/SCHOTT smoke, finalized merge/persistence, durable runs/Matches, ADR 0008, post-commit artifacts | A03 |
| Handoff README and missing-template navigation | Local metadata cleanup after target approval; no implementation ticket/template yet |

Before final Lean bodies cite the canonical PRD, its obsolete T14 order, unconditional Retry wording, and stale T16 sample-limit gate must be corrected to D-006/D-012 and accepted limit 10. This is canonical documentation alignment, not reopening a decision.

## 9. Scope and implementation-readiness risks

### Atomic cuts

- **A01 — XL atomic:** all final compiler foundations must leave only wiring, authored-surface switch, fixture/docs migration and deletion.
- **A02 — XL atomic:** Browser/Detection foundations must leave only final caller switching, parity and complete deletion/proof.
- **A03 — XL atomic:** Q01 and DB01 must leave only Search Run wiring, finalized conversion/merge retargeting, one transaction call, bypass/artifact deletion and docs/ADR migration.

These may require coordinated implementation internally, but each has one productive merge/release boundary.

### Feasibility and readiness uncertainties

1. **B02:** prove bounded graceful close, forced terminate/reap, handler completion/abort and session cleanup with the pinned Browser stack; inability is typed infrastructure failure.
2. **Browser-free plans:** define the final typed representation after deleting `UnavailableProfileBrowserClient`; no compatibility fake.
3. **Template:** decide registered Primitive identity versus exhaustively parity-owned compiler infrastructure.
4. **Parse `text`:** remove authored admission or implement a real executable capability; no rejection-only registration.
5. **Predicate catalog:** freeze whether Detection `contains` and status comparison are independent P07 identities or parent-owned options using canonical operations.
6. **Capture:** freeze named-group selection and missing-group behavior; no unnamed fallback.
7. **Transform:** fix split Serde names and decide real `to_string`/`url_decode` semantics.
8. **Acceptance:** define `minResults` placement; remove `maxErrorRatio` unless truthful semantics are separately evidenced.
9. **S01:** preserve one unchanged complete Strategy Set/Policy invocation; do not prune Strategies per field. Freeze the production/scripted seam and complete old API deletion list.
10. **Q01:** keep batch protocol/private parent allowance inside one Source-scoped operation; no public per-candidate loop or second mutable ledger.
11. **DB01/A03:** freeze durable Search Run ID projection, foreign-key enforcement, and the exact existing posting/source identity behavior preserved from ADR 0008/current importer.
12. **External app-data:** old Source JSON is strictly rejected/manual-recreated; no migration or translator.
13. **Existing staged files:** all later work must re-baseline and preserve unrelated changes.

Any readiness resolution requiring a compatibility path, dual productive route, Retry placeholder, public teardown residue, payload on budget/failure/Cancellation, hint promotion, or persistence of non-final values must stop and request explicit reopening of the relevant D-001–D-013 decision.

## 10. Required GitHub publication and cleanup workflow

Final ticket publication and cleanup form **one separately approved tracker migration**, not unrelated later housekeeping. Before that migration, the final Lean bodies and an exact `final label → reused issue / new issue / superseded issue` manifest must be reviewed locally.

The approved tracker migration runs in this order:

1. remove stale readiness labels, especially `ready-for-agent` from #167;
2. create every required new final issue with #166 as parent and without readiness unless its real blockers are complete;
3. rewrite reused current issues to their approved final responsibility;
4. install the final native direct dependency graph and verify parent links;
5. add reciprocal supersession links between replacement and absorbed issues;
6. close only fully absorbed issues with state reason **Not planned** and a comment `Superseded by #…`; never close an issue before its replacement exists;
7. update #166 navigation/checklists to the final set;
8. verify all 42 final responsibilities exist exactly once, the native graph is acyclic, blocked tickets have no readiness label, and no obsolete dependency remains.

Current issues are reused whenever their retained responsibility matches a final ticket. Split issues normally keep one current issue and create additional issues. Merge/absorption closes only the redundant issue after its complete contract has moved and is linked. This preserves tracker history while ensuring obsolete issues do not remain open.

No partial publication should leave duplicate active responsibilities, missing replacement links, stale readiness, or a half-migrated dependency graph.

## 11. Review gate

This recommendation stops before:

- rewriting any Lean ticket;
- creating a new template;
- editing GitHub bodies, labels, parents, or dependencies;
- implementing product code.

Approval requested: **42-ticket catalogue, Current-to-Final mapping, semantic DAG, A01/A02/A03 activation boundaries, documented readiness uncertainties, and the atomic GitHub publication/cleanup workflow above.**
