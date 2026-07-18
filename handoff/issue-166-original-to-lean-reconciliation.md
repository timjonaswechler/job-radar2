# Issue #166 — Original-to-Lean Reconciliation

Status: **local Phase-2 review artifact — no GitHub changes approved**  
Baseline: **2026-07-18**

## Scope and evidence

This review compares the 27 live GitHub implementation-ticket bodies with the unpublished lean drafts. The comparison is behavioral, not textual: responsibility, caller-visible behavior, invariants, error and control flow, bounds, Diagnostics, tests, migration/deletion ownership, and non-goals were checked.

Read-only baseline:

- live issues #166, #167–#180, #192, #193, #195, #202–#207, #218, #219, #233, and #234, including all comments;
- native GitHub parent and dependency metadata;
- `CONTEXT.md`, the Strategy Algebra PRD, ADRs 0001/0008/0009/0010, the shared delivery contract, the Phase-1 deduplication matrix, and all 27 lean drafts.

All 27 implementation issues are open children of #166. Only #167 currently has `ready-for-agent`. The implementation tickets have no comments; #166 has one historical decomposition comment. Native dependencies match the local ticket index. Live GitHub remains the sole source for original bodies and tracker state.

Verdict meanings:

- **Equivalent:** the lean draft preserves the original ticket-specific contract; readiness re-baselining may still be mandatory.
- **Equivalent after reference fix:** behavior is preserved, but a stale or incorrect ownership/navigation/document reference must be corrected.
- **Decision required:** the shortening is substantially faithful, but an unresolved contract or transition prevents approval.
- **Contract loss:** a binding original contract disappeared without a canonical destination or explicit decision.

Every blocked ticket’s paths, symbols, and focused test names remain provisional until its direct blockers land; this is recorded below as readiness re-baselining rather than contract loss.

## Verdict summary

| Verdict | Tickets | Count |
|---|---|---:|
| Equivalent | T2, T3a, T4b, T5, T6, T8, T9, T10, T11a, T11c, T12a, T14a, T17 | 13 |
| Equivalent after reference fix | T3b, T7, T11b, T14d | 4 |
| Decision required | T1, T4a, T13a, T13b, T13c, T14b, T14c, T15, T16 | 9 |
| Contract loss | T12b | 1 |

A `contract loss` verdict means the lean draft removed a published contract without a canonical destination. It may still become the desired target, but only through an explicit replacement decision—not editorial shortening.

## Compiler and Effective Source Profile

### T1 / #167 — Effective Profile Compiler

- **Original outcome:** `compile_source` accepts one authoritative concrete Source and immutable registry snapshot, produces one fully validated `CompiledSource`, exposes the Effective Source Profile for profile-based access, and compiles one direct scalar specialization while keeping lifecycle admission outside.
- **Lean outcome:** Same compiler boundary, authority rule, validation order, Profile/Source-owned result split, immutable-plan runtime boundary, caller migration, and old-entry-point deletion.
- **Preserved ticket-specific contracts:** A same-key Source in the snapshot cannot replace the supplied Source; no registry port; one existing Strategy scalar is specialized; forbidden identity/Detection/Search Request/persistence/`null` shapes reject; rejection exposes neither partial plan nor partial Effective Profile; profile regressions remain data-driven.
- **Moved contracts:** General compiler architecture → Strategy Algebra PRD Decisions 12–23 and module decision 1; Search Request separation → `CONTEXT.md`/PRD Decision 11; readiness, hard cut, tests, migration, deletion, DoD, and PR evidence → `handoff/issue-166-delivery.md`.
- **Intentional removals:** Publication/decomposition history, repeated dependency-category tutorial, generic test-seam prose, duplicated checklists and PR attestation.
- **Lost-contract candidates:** None caused by shortening.
- **Semantic changes:** The lean makes no intended outcome change, but both versions leave the old executable `sourceOverrides` path until T7 while direct specialization becomes executable here.
- **Readiness re-baseline:** Current paths/tests predate T1. `ready-for-agent` is not defensible until the dual-specialization transition is resolved.
- **Verdict:** **Decision required.** Resolve C166-001 before assignment.

### T2 / #168 — Effective Profile Merge

- **Original outcome:** Expand T1 to deterministic recursive merge of admitted existing keyed entries, preserving base order and replacing non-keyed arrays whole.
- **Lean outcome:** Same merge semantics, diagnostics, validation, and immutable output; complete new keys remain T3a work.
- **Preserved ticket-specific contracts:** Stable-key Access Path/Strategy merge; tagged-object behavior; scalar/object replacement; structural `null` prohibition versus legal typed JSON-null values; duplicate/unknown cardinality and deterministic JSON Pointer/`strategyKey` diagnostics; Source Config set-intersection ordering; no partial output.
- **Moved contracts:** Stable-key/array/`null` rules → PRD Decisions 12–21 and 38; shared readiness/testing/deletion → delivery contract.
- **Intentional removals:** Placeholder history, predecessor restatement, dependency/seam tutorials, duplicated DoD/PR evidence.
- **Lost-contract candidates:** None.
- **Semantic changes:** The original’s vague future “global ceiling specialization” wording is normalized to the accepted tighten-only rule; T3a applies landed checks but does not let Sources raise ceilings.
- **Readiness re-baseline:** All paths/types/tests must be checked after #167.
- **Verdict:** **Equivalent.**

### T3a / #169 — Complete Source-added Strategies and Access Paths

- **Original outcome:** Permit complete new Discovery/Detail Strategies, Detail steps, and Access Paths; append them deterministically and allow path selection only after full validation.
- **Lean outcome:** Same addition, append, completeness, validation-gate, and selection behavior.
- **Preserved ticket-specific contracts:** New entries inherit nothing; required members and sorted `missingFields`; contextual `name`; no Source Config Schema fragments; no borrowing another path’s config keys; invalid selected or unselected additions reject all output; inherited entries retain order and new entries append.
- **Moved contracts:** Keyed append/no-delete/no-reorder semantics → PRD Decisions 16–20 and 38; compiler sequence/runtime boundary → Decisions 20–22; shared security/testing/deletion → delivery contract.
- **Intentional removals:** Authoring history, repeated predecessor specification, generic dependency/test/PR prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** None. Numeric/current implementation facts are correctly treated as landed-baseline inputs rather than T3a-owned immutable decisions.
- **Readiness re-baseline:** Current completeness uses transitional `postingDiscovery`/`postingDetail`, complete title/company/URL Discovery, and description-only Detail. Preserve that only as staged baseline, not final schema-v3 semantics.
- **Verdict:** **Equivalent.**

### T3b / #170 — Effective Source Config Schema

- **Original outcome:** Establish one constrained Effective Source Config Schema and shared in-process validator for compiler and Detection with context-specific diagnostics and zero probes for invalid profile schema definitions.
- **Lean outcome:** Same supported grammar, merge/composition, validation, diagnostic mapping/order, and whole-profile eligibility gate.
- **Preserved ticket-specific contracts:** Root/property grammar; scalar types; `pattern`, scalar `enum`, absolute-URI `format`, `minimum`, profile-only `title`; unsupported keyword rejection; unique/declared `required`; whole `required`/`enum`; closed `additionalProperties`; same-location merge before composition; compiler/source-validation/detection categories; Source-owned reuse.
- **Moved contracts:** Authoritative subset → PRD Decision 47; Source Config/Search Request boundary → `CONTEXT.md`, ADR 0001’s still-valid separation rule, and PRD Decision 11; generic delivery → shared delivery contract.
- **Intentional removals:** Tracker history, repeated built-in/frontend evidence narrative, generic seam/test/DoD/attestation sections.
- **Lost-contract candidates:** No behavior is lost, but the lean incorrectly says T3a owns the final profile/path Source Config composition rule. T3b and PRD Decision 47 own it.
- **Semantic changes:** None beyond that ownership attribution.
- **Readiness re-baseline:** Exact schema/parser/registry paths and tests follow #169.
- **Verdict:** **Equivalent after reference fix.** Correct the T3a attribution.

### T4a / #171 — Effective Profile Provenance

- **Original outcome:** Add complete, deterministic, value-minimized Effective Profile provenance to the compiler result, with typed stable-key paths and no partial provenance on rejection.
- **Lean outcome:** Same Profile/Source-owned variants, terminal coverage, origins, path segments, ordering, serialization, invariant diagnostic, and data minimization.
- **Preserved ticket-specific contracts:** Same-pipeline recording rather than replay/diff; exact terminal-value coverage including arrays and inherited `title`; closed origins; base locator keys remain base-origin; new keyed entries are direct throughout; separate schema locations; no concrete Source Config values; no runtime branching on origin.
- **Moved contracts:** Compiler and hard-cut behavior → PRD Decisions 12–22 and 36–38; shared testing/migration → delivery contract.
- **Intentional removals:** Publication history, pre-T1 code inventory, dependency/test tutorials, duplicated PR evidence.
- **Lost-contract candidates:** None from shortening.
- **Semantic changes:** Both original and lean promise Policy provenance even though authored Policy arrives in independent sibling T5 and is not a blocker.
- **Readiness re-baseline:** Entire current gap and pre-v3 provenance segments must be rechecked after #170 and renamed in T7.
- **Verdict:** **Decision required.** Resolve C166-002.

### T4b / #175 — Schema-v3 Source Live Check Fingerprints

- **Original outcome:** Fingerprint canonical schema-v3 Source behavior through fixed, granular SHA-256 components, one compiler outcome, exact ordering/counts, runtime-binding dependencies, behavior versions, immutable globals, and metadata exclusion.
- **Lean outcome:** Same component partitions, canonical projections, order/counts, version/global tokens, rejected-branch behavior, activation reuse, and data minimization.
- **Preserved ticket-specific contracts:** Existing Check Report v1 semantics; exact Profile/Source-owned/rejected component sets; typed runtime dependencies; `profile-compiler/v1`, `profile-runtime/v1`, `immutable-globals/v1`; closed initial global inventory; no secrets/raw material; one compile/preparation; no pre-v3 fingerprint migration.
- **Moved contracts:** Canonical-v3/no-migration → PRD Decisions 33 and 36–37; Source Live Check role → ADR 0010/`CONTEXT.md`; shared delivery → delivery contract.
- **Intentional removals:** Commit hash, verbose path inventory, generic dependency/test/PR prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** None. The original and lean already bind later owners to canonical plan material plus the `profile_compiler`, `profile_runtime`, and `immutable_globals` partitions; T9/T10 consume those rules without adding an inventory row.
- **Readiness re-baseline:** Compiler/provenance/schema-v3 types, constants, security owners, and rejection persistence are future/conditional until #171 and #174 land.
- **Verdict:** **Equivalent.** C166-003 records the already accepted maintenance rule for later enforcement, not a new decision.

## Phase naming, runtime, budgets, and transport

### T5 / #172 — Compiled `first_accepted`

- **Original outcome:** Every compiled Discovery/Detail set has mandatory typed `FirstAccepted`; public phase operations preserve sequential accepted-first fallback, recovery, exhaustion, bounds, Diagnostics, and Cancellation.
- **Lean outcome:** Same mandatory plan field, strict authored boundary, compiler paths, and caller-visible behavior.
- **Preserved ticket-specific contracts:** Acceptance differs from transport success; rejected/failed attempts may recover; failed partial output is discarded; exhaustion returns no output plus one terminal; Cancellation suppresses later work/exhaustion; no cumulative budget yet; phase-private attempt models remain until T8.
- **Moved contracts:** Shared readiness, tests, migration, deletion, and PR evidence → delivery contract; common Policy concepts → PRD Decisions 2–4, 22, and 26.
- **Intentional removals:** Publication and dirty-state history, generic dependency/test/DoD prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** None.
- **Readiness re-baseline:** Paths/types/tests follow #170.
- **Verdict:** **Equivalent.**

### T6 / #173 — Internal Phase Module Renaming

- **Original outcome:** Rename internal/public Rust phases to `detection`, `discovery`, and `detail` directly, while authored v2 fields and observable v2 Diagnostic vocabulary remain until T7.
- **Lean outcome:** Same two-boundary transition and runtime parity.
- **Preserved ticket-specific contracts:** Final Rust/compiled names; no aliases/wrappers/duplicate modules; authored `detect`/`postingDiscovery`/`postingDetail` temporarily remain; compiled serialization changes now; runtime behavior, local bounds, and Cancellation do not change.
- **Moved contracts:** Hard-cut rules → PRD Decisions 1 and 36–38 plus delivery contract.
- **Intentional removals:** Branch/dirty-tree history and repeated migration/test exposition.
- **Lost-contract candidates:** None.
- **Semantic changes:** None; “retry bounds” only preserves any landed local behavior and does not authorize retry execution/accounting.
- **Readiness re-baseline:** Inventory exact #172 names and conditional T4a imports.
- **Verdict:** **Equivalent.**

### T7 / #174 — Authored Schema-v3 Hard Cut

- **Original outcome:** Leave exactly one active schema-v3 Source/Profile model with canonical phase names, mandatory authored Policy, direct typed Source fragments as sole specialization, strict v2 rejection, and deletion of old overrides/docs/resources.
- **Lean outcome:** Same hard cut, compiler validation order, diagnostics rename, caller/resource migration, and deletion ownership.
- **Preserved ticket-specific contracts:** Internally tagged `{ "type": "first_accepted" }`; existing fragments may inherit Policy; new complete entries must supply it; forbidden identity/Detection/Search Request/persistence/`title`/`null` shapes; no compatibility runtime; ADR/glossary migration.
- **Moved contracts:** Schema-v3 architecture → PRD Decisions 1, 12–22, 36–38, and 47; generic migration/testing → delivery contract.
- **Intentional removals:** Publication/dirty-tree history and duplicated shared evidence.
- **Lost-contract candidates:** None.
- **Semantic changes:** None. The series-level T1→T7 dual-specialization conflict remains external to the shortening.
- **Readiness re-baseline:** T6 must first leave only authored/Diagnostic/document v2 surfaces.
- **Verdict:** **Equivalent after reference fix.** Remove the lean reference to deleted “published ticket snapshots”; live GitHub is the original source.

### T8 / #176 — Typed Strategy Set Runtime

- **Original outcome:** One crate-private closed kernel runs Discovery/Detail `FirstAccepted`, while typed phase operations/adapters retain their domain inputs, outputs, acceptance, reducers, and diagnostics.
- **Lean outcome:** Same private states, phase ownership, attempt privacy, terminal behavior, caller migration, and duplicate-loop deletion.
- **Preserved ticket-specific contracts:** No public generic executor; `Completed(Accepted/Rejected)`, `Failed`, typed `Cancelled`; no Diagnostic-code control flow; exact exhaustion/cancellation ordering; Search Run cancellation cardinality; runtime-attempt identity remains distinct from merge provenance/fingerprints.
- **Moved contracts:** Runtime architecture → PRD module decision 2; generic delivery → shared delivery contract.
- **Intentional removals:** Repetitive private type sketches and shared test/DoD text.
- **Lost-contract candidates:** None.
- **Semantic changes:** None; no cumulative ledger or new completion is introduced.
- **Readiness re-baseline:** Exact T7 phase paths and landed local bounds.
- **Verdict:** **Equivalent.**

### T9 / #177 — Cumulative Strategy Set Budgets

- **Original outcome:** Add one invocation-wide parent/child ledger for seven dimensions, component-wise tighten-only limits, debit-before-side-effect, exact usage/completion, deterministic terminal precedence, and a one-request-per-invocation Source Live Check change.
- **Lean outcome:** Same dimensions/ceilings, charging units, report, precedence, caller change, and retry/byte exclusions.
- **Preserved ticket-specific contracts:** Attempts, requests, produced items, duration, pages, browser actions, fan-out; checked atomic debit; equality succeeds; no refunds/resets; exact source/dimension ordering; prefix/accounting rules; Cancellation and already-completed acceptance precedence; no retry field until an executable retry exists.
- **Moved contracts:** General boundedness/Cancellation → PRD Decisions 27–28 and delivery contract; fingerprint maintenance references T4b’s existing partitions.
- **Intentional removals:** Historical current-state narration and duplicated delivery prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** Intentional Source Live Check tightening from one request per Strategy to one cumulative request per invocation is preserved and must not be described as parity.
- **Readiness re-baseline:** Actual charge sites and browser seam follow T8; numeric ceilings are accepted, not provisional.
- **Verdict:** **Equivalent.** Native GitHub already owns downstream relationships; adding a compact navigation line is optional documentation polish, not a required reference fix.

### T10 / #178 — Byte-preserving HTTP Responses

- **Original outcome:** One byte-preserving Discovery/Detail HTTP seam, bounded collector, cumulative 64 MiB byte dimension, strict explicit decoding, sanitized Diagnostics, and deletion of string/phase-specific transport paths.
- **Lean outcome:** Same shared boundary, response facts, accounting, decoding precedence, failure/cancellation behavior, and migration.
- **Preserved ticket-specific contracts:** Repeated non-lossy headers; final URL/status/content type/exact bytes; reqwest decompression disabled; exact-boundary EOF; prefix accounting on failure/cancel; no accepted output on byte exhaustion; authored→BOM→HTTP→UTF-8 selection after validating every declaration; no lossy decoding; sensitive types non-serializable/redacted; localhost production-adapter test.
- **Moved contracts:** Generic budget/Cancellation/testing/deletion → PRD/delivery; parser registry remains T11a.
- **Intentional removals:** Repeated seam rationale, generic checklists, and editorial examples.
- **Lost-contract candidates:** None.
- **Semantic changes:** None; Browser/Detection and retries remain excluded.
- **Readiness re-baseline:** Exact T8/T9 result/ledger/client names and reqwest features.
- **Verdict:** **Equivalent.**

## Shared Primitive families and posting phase outputs

### T11a / #179 — Shared Parse Primitives

- **Original outcome:** Discovery and Detail share canonical JSON/XML/HTML parse implementations and a family-scoped registry; HTTP consumes only bounded decoded text, Browser remains distinct, and authored `text` is rejected at compile time.
- **Lean outcome:** Same ownership, parse inputs, failure behavior, `text` rejection, completeness proof, and caller migration.
- **Preserved ticket-specific contracts:** Canonical owner files; registry-only dispatch; one `ParsedDocument`; no partial parse output; no parse after decode/budget/cancel terminal; exact compiler Diagnostic for `text`; family-only parity/synthetic/inventory completeness; no browser-to-HTTP fabrication.
- **Moved contracts:** Global one-owner principle → PRD Decisions 39–40; generic testing/deletion → delivery contract.
- **Intentional removals:** Publication history, alternatives, dependency tutorials, repeated DoD/attestation.
- **Lost-contract candidates:** None.
- **Semantic changes:** None.
- **Readiness re-baseline:** `BoundedDecodedBody`, browser response types, parser call sites, and tests after T10.
- **Verdict:** **Equivalent.** Global remaining-family ownership is tracked separately in C166-005.

### T11b / #180 — Shared Select Primitives

- **Original outcome:** Six evidenced Select types compile/execute through canonical owners with static syntax/context checks, existing XML grammar, tightly scoped `sitemap_urls`, and phase-owned cardinality/queue behavior.
- **Lean outcome:** Same six keys, placement rules, XML semantics, omission behavior, runtime diagnostics, and URL-component deferral.
- **Preserved ticket-specific contracts:** Typed plans only; JSONPath/CSS/regex validation; literal case-sensitive non-XPath XML grammar; `sitemap_urls` only in two XML Discovery sitemap placements; omitted child/posting selectors have exact behavior; no duplicate value/cardinality/output logic.
- **Moved contracts:** Common registry/testing/deletion rules → PRD/delivery.
- **Intentional removals:** Publication/alternatives/dependency tutorials and duplicated evidence prose.
- **Lost-contract candidates:** No behavior loss; one predecessor reference is wrong.
- **Semantic changes:** Lean says T11a supplies shared parsed-document **and item** representations, while T11a explicitly leaves `RuntimeItem` movement to T11b.
- **Readiness re-baseline:** Select enums, clone helpers, XML evidence, and test names after T11a.
- **Verdict:** **Equivalent after reference fix.** T11a supplies `ParsedDocument`; T11b owns shared selected-item/`RuntimeItem` consolidation.

### T11c / #192 — Shared Value Primitives

- **Original outcome:** Compile every admitted Discovery/Detail Field Expression into typed value plans, enforce four contexts and immutable expression bounds, add strict scalar `first_non_empty`, hard-cut capture-source chaining, and prevent resolved values leaking into artifacts.
- **Lean outcome:** Same thirteen value owners, contexts, fallback semantics, capture hard cut, bounds, const parity, data minimization, and family completeness.
- **Preserved ticket-specific contracts:** Four exact contexts; Source/Source Config/posting/postingMeta availability; captures become visible only after complete-map construction; postingMeta union admission; `first_non_empty` candidate semantics including `0`/`false`; depth 16, 1,024 nodes, 16 candidates; scalar-only constants; selector delegation; no raw authored runtime expressions.
- **Moved contracts:** Shared primitive architecture → PRD Decisions 39–40; generic delivery → shared contract.
- **Intentional removals:** Approval history and numeric rationale narrative while retaining accepted values/counting scopes.
- **Lost-contract candidates:** None.
- **Semantic changes:** None.
- **Readiness re-baseline:** Existing variants, placements, selector/value helpers, and tests after T11b.
- **Verdict:** **Equivalent.** Global remaining-family ownership is C166-005.

### T12a / #193 — Source-local Posting Occurrences

- **Original outcome:** Discovery emits one typed `PostingOccurrence` with separate reference, provider values, hints, and postingMeta; identity is Source-local provider-ID-first then normalized absolute URL; incomplete occurrences remain invisible to pre-T16 Search Run bridging.
- **Lean outcome:** Same representation, identity, URL policy, trust boundary, single occurrence type, pre-T16 bridge, and docs migration.
- **Preserved ticket-specific contracts:** Source key cannot be authored; mixed identity kinds do not correlate; title/company/location never define identity; hints never become provider/canonical values; only explicit `search_prefilter` authorization exists; lossless provider locations; URL userinfo/fragment and normalization rules; sanitized item diagnostics; incomplete occurrences do not change old counts/statuses or trigger Detail.
- **Moved contracts:** Identity and hint/provider separation → PRD Decisions 24, 31–32, and 46; generic delivery → shared contract; new vocabulary later enters `CONTEXT.md`.
- **Intentional removals:** Approval/options narrative and duplicated seam/DoD/attestation prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** The lean corrects a stale dependency fact (`url = "2"` already exists); no behavioral change.
- **Readiness re-baseline:** DTO paths, expression outputs, summaries, and docs follow T11c; old `postingDiscovery`/`postingDetail` vocabulary remains transitional only.
- **Verdict:** **Equivalent.** The corrected dependency fact and provisional paths require no lean-body repair beyond normal readiness re-baselining.

### T12b / #195 — Requested Detail Patches and Phase Reducers

- **Original outcome:** Discovery/Detail expose reduced occurrence/requested-only patch envelopes with contribution provenance, conflicts, rejections, usage, and Diagnostics; reducers are conflict-safe and preserve completed accepted contributions at a budget stop.
- **Lean outcome:** Reducer/provenance/conflict behavior remains, but standalone usage becomes the complete T9 budget report and cumulative budget exhaustion exposes no reduced payload/provenance/conflict/rejection data.
- **Preserved ticket-specific contracts:** Four Detail fields; non-empty requested set; URL not patchable; exact Source-local grouping; field-local quarantine; whole-group rejection for same provider ID with different required URLs; atomic location-vector comparison; normalized fallback URL public result; minimal coordinate-only contribution origins; exact diagnostics/order; no public reducer or test Policy.
- **Moved contracts:** Requested-only Detail and conflict-safe reducers → PRD Decisions 10, 24–25; provenance distinctions/testing/migration → delivery contract.
- **Intentional removals:** Publication/alternatives/dependency tutorials and duplicated evidence sections.
- **Lost-contract candidates:** **Confirmed contract loss.** The published original says budget stops reduce completed accepted inputs and expose their contribution/provenance envelope. The lean removes that payload, and no PRD, ADR, `CONTEXT.md`, or shared-delivery section is its canonical destination.
- **Semantic changes:** One indivisible `StrategySetBudgetReport`; no reduced payload on exhaustion; current signature sketch does not express the required report-bearing terminal outside the phase payload. This may align better with T9, but it is a replacement decision, not deduplication.
- **Readiness re-baseline:** All occurrence/report/result names follow T9/T12a.
- **Verdict:** **Contract loss at the Phase-2 baseline; resolved by accepted replacement D-003.** The published prefix-reduction contract is intentionally replaced by one closed, report-bearing, no-payload budget terminal owned by T12b and propagated through T13/T15/T16.

## Additional Strategy Policies

### T13a / #202 — `all_required`

- **Original outcome:** Strict sequential universal acceptance; first rejection/failure stops later work; accepted prefixes stay private; reducer runs once only after all accept; policy-unsatisfied result and exact terminal Diagnostic are payload-free.
- **Lean outcome:** Same fail-fast, reducer, report, budget, Cancellation, and Diagnostic behavior, plus a first-lander shared-result migration rule.
- **Preserved ticket-specific contracts:** Strict non-parameterized shape; accepted attempts rather than transport success; no prefix reduction; exact `strategy_policy_all_required_unsatisfied`; budget/Cancellation suppress policy/fallback terminals; reducer conflicts do not retroactively fail the Policy.
- **Moved contracts:** Shared Policy/kernel/budget/reducer architecture → PRD Decisions 2–5 and 22–28; generic migration/testing → delivery contract.
- **Intentional removals:** Publication/dirty-tree/alternatives and duplicated evidence sections.
- **Lost-contract candidates:** No Policy behavior loss. Post-reduction/pre-envelope Cancellation is less explicit than in siblings.
- **Semantic changes:** Adds non-exclusive “first sibling to land owns shared algebra” migration responsibility.
- **Readiness re-baseline:** Result/report/Cancellation placement, paths, callers, and tests after #177/#195.
- **Verdict:** **Decision required.** Resolve C166-007 and C166-008.

### T13b / #203 — `at_least(count)`

- **Original outcome:** Positive static threshold; stop successfully at the Nth accepted attempt or unsuccessfully at earliest mathematical impossibility; equality remains reachable; reduce accepted outputs once; exact payload-free terminal.
- **Lean outcome:** Same cardinality, earliest-stop equations, reducer gating, budget/Cancellation precedence, result privacy, and Diagnostic, plus possible first-lander ownership.
- **Preserved ticket-specific contracts:** Final merged cardinality validation; accepted attempts only; `count == cardinality` remains distinct from `all_required`; exact `strategy_policy_at_least_unsatisfied`; pre-commit Cancellation discards computed reduction.
- **Moved contracts:** Common Policy/runtime/delivery rules → PRD and shared delivery contract.
- **Intentional removals:** Historical and generic delivery prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** Replaces ambiguous reuse of #202-approved algebra with an unsafe first-lander rule.
- **Readiness re-baseline:** Exact blocker-landed types, diagnostics, exhaustive callers, and tests.
- **Verdict:** **Decision required.** Resolve C166-007.

### T13c / #204 — `collect_all(minAccepted)`

- **Original outcome:** Execute every Strategy despite reaching or losing the threshold; decide only at natural completion; reduce all accepted outputs once on success; budget/Cancellation discard retained/computed output.
- **Lean outcome:** Same execute-all, natural-completion, identity/reducer, no-output terminal, report, and exact Diagnostic behavior.
- **Preserved ticket-specific contracts:** Positive final-cardinality threshold; no early success/impossibility; Source-local Discovery identity and first-seen order; requested-only Detail patches; exact `strategy_policy_collect_all_unsatisfied`; no partial phase completion.
- **Moved contracts:** Common architecture/delivery → PRD and shared contract.
- **Intentional removals:** Publication/alternatives/dependency and duplicated evidence prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** Existing landed-state/first-owner rule remains unresolved rather than newly introduced.
- **Readiness re-baseline:** Reducer/result/report names and sibling landed state after blockers.
- **Verdict:** **Decision required.** Resolve C166-007.

## Detection convergence

### T14a / #205 — URL/HTTP Detection Strategy Sets

- **Original outcome:** Compile one ordered Detection Strategy Set per reusable profile, run URL plus HTTP Strategies under `all_required`, preserve outer proposal/status/support/browser behavior, and delete flat/imperative URL/HTTP paths.
- **Lean outcome:** Same URL alternatives, HTTP order, fail-fast semantics, cumulative bounded transport, cancellation, profile aggregation, and transition bridge.
- **Preserved ticket-specific contracts:** First matching URL alternative; explicit pass-through; optional expected-status behavior; earlier captures available to later HTTP Strategies; profile evidence seeded once; per-profile ledger; transitional same-key capture replacement; sanitized terminals; no alternate Policies.
- **Moved contracts:** Kernel/HTTP/value/budget architecture → blockers/PRD; generic hard cut/test/PR rules → delivery contract.
- **Intentional removals:** Approval/current-state narrative, alternatives, generic seam/DoD sections.
- **Lost-contract candidates:** None.
- **Semantic changes:** None.
- **Readiness re-baseline:** All proposed module/test names follow #178/#192/#202.
- **Verdict:** **Equivalent.** Its transitional outputs constrain T14b/T14c conflicts.

### T14b / #206 — Detection Reduction and Proposal Provenance

- **Original outcome:** One incremental conflict-safe reducer consumes URL/HTTP/proposal-preparation/transitional-browser contributions, constructs the sole proposal, and exposes complete value/evidence provenance without last-write-wins.
- **Lean outcome:** Same reducer, atomic pointer semantics, contribution order/origins, required authored contribution shape, validation, proposal constructor, provenance DTO, and migration.
- **Preserved ticket-specific contracts:** Equal-value origin union; conflicting captures/recommendations/overlapping Source Config values fail the profile; complete atomic values; first-value retention; stable evidence identity; profile metadata origin; direct-Source rejection; mandatory serialized provenance.
- **Moved contracts:** Generic reducer/test/deletion rules → PRD/delivery; Detection retains distinct domain types.
- **Intentional removals:** Approval/rejected-alternative narrative and duplicated evidence prose.
- **Lost-contract candidates:** None caused by shortening; “recommended” correctly becomes required.
- **Semantic changes:** Existing transition remains under-specified: current browser aggregates can overwrite same-key evidence before translation, while browser templates require reconciled pre-browser Source Config.
- **Readiness re-baseline:** Exact T14a contribution/context/diagnostic/browser bridge after #205.
- **Verdict:** **Decision required.** Resolve C166-009, C166-010, and C166-015.

### T14c / #207 — Bounded Browser Detection

- **Original outcome:** Browser acquisition becomes a typed Detection Strategy behind a lifecycle boundary with exact multi-scope ceilings, atomic accounting, byte checks, typed Cancellation, and bounded teardown; old probe/render path is removed.
- **Lean outcome:** Same lifecycle stages, ceilings, residue projection, teardown slices, accounting, reducer integration, and deletion goals.
- **Preserved ticket-specific contracts:** Managed/scripted parity; tests cannot inject control state; one primary outcome; residue cannot change acceptance; exact Strategy/profile/operation ceilings; teardown reserve and forced termination; byte-before-parse; no secret/PID/path/raw-error leakage; no provider branches.
- **Moved contracts:** Generic runtime/reducer/cancellation/delivery → PRD and shared contract.
- **Intentional removals:** Method-level pseudocode and repeated architecture/test prose while retaining stage checkpoints and acceptance cases.
- **Lost-contract candidates:** None clearly caused by shortening.
- **Semantic changes:** The ticket deletes/replaces a browser seam shared by non-Detection callers declared out of scope; asks for recovered fallback under an `all_required` route; leaves operation-ledger nesting and cancellation-residue projection unclear.
- **Readiness re-baseline:** Full `ProfileBrowserClient` call graph, process ownership, terminals, and tests after #206.
- **Verdict:** **Decision required.** Resolve C166-011 through C166-014.

### T14d / #218 — Remove Replaced Detection Execution

- **Original outcome:** Prove one typed Detection operation remains, delete residual wrappers/evaluators/fakes/dispatch, and optionally add a NUL-safe self-tested convergence guard; no product change if already converged.
- **Lean outcome:** Same sole-operation invariants, residual deletion, conditional guard-only outcome, and guard mechanics.
- **Preserved ticket-specific contracts:** Deterministic adapters through same operation; no URL-only overload; immutable compiled plan; typed Cancellation; no new DTO/status/limit/policy/reducer; exact guard baseline/producers/NUL handling and residual classification.
- **Moved contracts:** Generic delivery/testing/PR evidence → shared contract.
- **Intentional removals:** Publication history, duplicate guard exposition, and repeated blocker contracts.
- **Lost-contract candidates:** None.
- **Semantic changes:** Retains a recovered-attempt acceptance case that cannot occur under the declared `all_required` Detection route.
- **Readiness re-baseline:** All residual symbols and guard exclusions after #207.
- **Verdict:** **Equivalent after reference fix.** No inner fallback owner exists under mandatory `all_required`; remove the impossible recovered-attempt row. This is a deterministic contract correction, not an open product choice.

## Detail, Candidate Resolution, and persistence

### T15 / #219 — Field-requested Candidate-scoped Detail

- **Original outcome:** A Source-identity-safe request reuses trusted occurrence values, classifies unsupported fields without I/O, invokes the complete Detail set once for supported missing fields, returns four-field dispositions plus exact phase evidence, and migrates UI/Live Check callers.
- **Lean outcome:** Same Source check, field/capability routing, one invocation, reuse, five dispositions, exact evidence, caller migration, and cancellation behavior.
- **Preserved ticket-specific contracts:** Source mismatch before I/O; non-empty canonical request; compiler-derived capabilities; requested-only values; `Reused`, `Produced`, `Unsupported`, `Unavailable`, `Conflicted`; no per-field Strategy loop; UI persisted-description short circuit and temporary-SQLite update rules; Live Check description-only request.
- **Moved contracts:** Requested-detail/laziness → PRD Decisions 10 and 41–42; generic delivery → shared contract.
- **Intentional removals:** Publication/alternatives/dependency tutorials and duplicated evidence prose.
- **Lost-contract candidates:** No T15-local omission.
- **Semantic changes:** Its ordinary `Unavailable` projection cannot represent lean T12b’s payload-free budget terminal honestly, and its success-or-cancellation result cannot supply T16’s typed candidate execution failure.
- **Readiness re-baseline:** All description-era types/callers/results after #195.
- **Verdict:** **Decision required.** Resolve C166-006 and C166-016 before readiness.

### T16 / #233 — Candidate Resolution

- **Original outcome:** One Source-scoped operation processes bounded Discovery batches, enforces protocol/identity, requests minimal Detail fields, normalizes/evaluates Search Request rules, and releases only finalized candidates with typed completion, exact counts/usage, and bounded Diagnostics.
- **Lean outcome:** Same batch protocol, private states, evaluation rounds, completion/counts, sample limit 10, Source/Search Run visibility, cancellation/abort rules, and finalized-only handoff.
- **Preserved ticket-specific contracts:** Opaque continuations; exact exhaustion/remaining semantics; cross-batch Source-local uniqueness; sequential order; authorized-hint reject-only prefilter; central normalization/final rules; minimal Detail rounds; no-progress/conflict handling; exact count equations; `Complete` despite individual unresolved/failed; immutable sample limit 10 and complete per-code counts; no automatic release on cancellation/abort; no new statuses.
- **Moved contracts:** Candidate architecture and completion/count rules → PRD Decisions 41–45/module decision 3; generic delivery → shared contract.
- **Intentional removals:** Publication/dependency tutorials and duplicated DoD/PR evidence.
- **Lost-contract candidates:** None caused by shortening.
- **Semantic changes:** None relative to the original, but the original/lean both claim retry limits/usage without an executable retry owner, rely on an unmapped T15 Detail failure, and leave `remaining` recurrence ambiguous.
- **Readiness re-baseline:** Replace current vector executor/counters/artifacts only after T15 lands; exact type/usage ownership remains provisional.
- **Verdict:** **Decision required.** Resolve C166-016, C166-017, and C166-020.

### T17 / #234 — Finalized-only Deduplication and Persistence

- **Original outcome:** Only T16 finalized values enter cross-Source merge and atomic SQLite persistence; durable Search Runs and Matches record committed terminal results, while failed/cancelled runs get no Matches; ADR 0008 is superseded where conflicting.
- **Lean outcome:** Same finalized-only route, merge semantics, SQL model, transaction, rerun/retention/cascade behavior, artifact boundary, and ADR ownership.
- **Preserved ticket-specific contracts:** Complete and executable Partial resolutions share the persistence path; cancellation/ordinary abort releases no Resolution; Source-local counts remain independent of cross-Source collapse; one Match per post-merge posting; one transaction for run/posting/sources/Matches/last-run metadata; failed/cancelled durable run rows but no candidate-derived import; rollback; post-commit artifact is non-authoritative; no durable candidate/diagnostic/usage/checkpoint tables.
- **Moved contracts:** Finalization/counts → T16/PRD Decisions 43–46; Match semantics → `CONTEXT.md`; generic migration/testing → delivery contract.
- **Intentional removals:** Approval history and superseded compact-draft narrative; repeated evidence prose.
- **Lost-contract candidates:** None.
- **Semantic changes:** None. T17 intentionally supersedes only ADR 0008’s no-history/no-request-link claims, preserving its work-item/manual-state and existing dedupe/update decisions unless separately changed.
- **Readiness re-baseline:** Exact T16 finalized type, importer return IDs, schema migration, artifact path, and sole production construction path.
- **Verdict:** **Equivalent.** The lean already makes ADR 0008 supersession a T17 implementation/documentation obligation; stale PRD sample-limit wording is separate cleanup.

## Series-wide moved-contract map

| Contract cluster | Canonical destination |
|---|---|
| Domain vocabulary and Search Request separation | `CONTEXT.md` |
| Strategy Set, compiler, runtime, Candidate Resolution, immutable plans, bounds, identity/hints/finalization | `docs/prd/declarative-profile-strategy-algebra.md` and #166 |
| Source Config separation and constrained schema transition | ADR 0001’s surviving separation rule; PRD Decision 47 for the target schema/specialization model |
| Declarative DSL and hard cut | ADR 0009 after T7’s required update/supersession; PRD Decisions 1 and 36–40 meanwhile |
| Concrete Source operational confidence | ADR 0010 |
| Durable Search Run/Match target | T17 accepted decision and the T17-owned ADR 0008 supersession |
| Readiness, migration/deletion, seams, test policy, security evidence, DoD, PR evidence | `handoff/issue-166-delivery.md` |
| Runtime budgets | Safety ceilings for bounded termination/resource containment; never Bot-Detection guarantees or target traffic patterns. Pacing/rate limits require a separate evidenced generic capability; see C166-032. |
| Live parent/dependency/readiness state and original ticket bodies | Native GitHub |

## Required metadata/document cleanup after review approval

Do not restore deleted artifacts. A later approved documentation cleanup should:

1. update `handoff/README.md` so GitHub is the sole original-ticket source;
2. remove references to `issue-166-final-tickets/`, `archive/`, and `issue-166-lean-ticket-worker-handoff.md`;
3. state that all 27 lean drafts exist and Phase 2 is reconciliation/conflict discovery;
4. remove the missing old-template reference from the Phase-1 matrix;
5. defer a new short template until restructuring establishes the necessary sections;
6. update PRD Decision 49 to record that T16’s sample limit is accepted as 10.

## Residual uncertainty

- Exact implementation names and test targets remain provisional for every blocked ticket.
- Several open items are architecture/result-boundary decisions, not documentation polish; they are enumerated in `handoff/issue-166-conflict-register.md`.
- No product code or GitHub state was changed by this review.
