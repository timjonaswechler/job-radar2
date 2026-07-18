# Issue #166 — Conflict Register

Status: **local Phase-2 review artifact — no GitHub changes approved**  
Baseline: **2026-07-18**

## Use

This register records contradictions and unresolved transitions found by comparing all live originals, all lean drafts, native dependencies, the Strategy Algebra PRD, `CONTEXT.md`, ADRs, shared delivery rules, current documentation, and relevant current call boundaries. It does not approve ticket splits, merges, moves, deferrals, drops, or dependency edits.

Statuses are limited to:

- **open** — a behavioral/ownership decision is still required;
- **resolved by existing accepted decision** — target behavior is decided, though implementation or documentation may still be pending;
- **documentation-only cleanup** — no product/architecture decision remains.

## Priority overview

| ID | Short name | Status | Decision deadline |
|---|---|---|---|
| C166-001 | Direct specialization vs active `sourceOverrides` | resolved by existing accepted decision | Apply D-001 during restructuring |
| C166-002 | Policy provenance owner | resolved by existing accepted decision | T5 blocks T4a under D-004 |
| C166-003 | Fingerprint ownership across T8–T10 | resolved by existing accepted decision | Enforce in T8–T10 |
| C166-004 | Transitional phase/value vocabulary | documentation-only cleanup | At each readiness review |
| C166-005 | Global Primitive completeness owner | resolved by existing accepted decision | Apply D-005 during restructuring |
| C166-006 | Budget-terminal phase result algebra | resolved by existing accepted decision | Apply D-003 in T12b |
| C166-007 | T13 shared result owner | resolved by existing accepted decision | T12b owns shared algebra |
| C166-008 | Common pre-commit Cancellation point | resolved by existing accepted decision | Apply D-003 to every Policy |
| C166-009 | Pre-browser Source Config availability | resolved by existing accepted decision | Apply D-006 in T14b |
| C166-010 | Browser overwrite loses contributions | resolved by existing accepted decision | No aggregate translation under D-006 |
| C166-011 | Shared browser seam migration | resolved by existing accepted decision | Shared foundation + cross-phase cut under D-007 |
| C166-012 | Impossible recovered fallback | resolved by existing accepted decision | Remove rows under D-007 |
| C166-013 | Detection operation/profile ledger nesting | resolved by existing accepted decision | Parent/profile/Strategy scopes under D-007 |
| C166-014 | Cancellation teardown residue projection | resolved by existing accepted decision | Residue private under D-007 |
| C166-015 | Validation before dependent Detection work | resolved by existing accepted decision | Incremental + final shared validation under D-006 |
| C166-016 | T15 terminal → T16 candidate failure mapping | resolved by existing accepted decision | Apply D-008 in T12b/T15/T16 |
| C166-017 | Retry accounting without executable retry | resolved by existing accepted decision | Remove/defer under D-002 |
| C166-018 | SCHOTT smoke contradicts target trust/bounds | resolved by existing accepted decision | Update in T16/T17 slices |
| C166-019 | ADR 0008 vs durable Search Runs/Matches | resolved by existing accepted decision | Same slice as T17 |
| C166-020 | T16 `remaining` recurrence | resolved by existing accepted decision | Post-batch under D-009 |
| C166-021 | Finalization bypass through broad constructors | resolved by existing accepted decision | T17 completion review |
| C166-022 | Stale/deleted handoff references | documentation-only cleanup | After review approval |
| C166-023 | Stale T16 sample-limit gate | documentation-only cleanup | Before restructuring uses PRD gate text |
| C166-024 | T3b misattributes schema composition | documentation-only cleanup | Before lean body publication |
| C166-025 | T11b misattributes item representation | documentation-only cleanup | Before lean body publication |
| C166-026 | T9 omits downstream tracker navigation | documentation-only cleanup | Optional before lean body publication |
| C166-027 | Occurrence identity vs cross-Source dedupe | resolved by existing accepted decision | Enforce T12a→T17 |
| C166-028 | Provider values vs hints | resolved by existing accepted decision | Enforce T12a→T17 |
| C166-029 | T14 slice-local deletion vs T14d cleanup | resolved by existing accepted decision | Every T14 completion review |
| C166-030 | T7 deleted-snapshot reference | documentation-only cleanup | Before lean body publication |
| C166-031 | Candidate Resolution parent-budget accounting | resolved by existing accepted decision | Parent/child contract under D-009 |
| C166-032 | Safety ceilings vs pacing/Bot Detection | resolved by existing accepted decision | Apply during restructuring |
| C166-033 | Failure/Cancellation terminals omit usage report | resolved by existing accepted decision | Apply D-010 in T12b/T15/T16 |
| C166-034 | Compiler activation graph and fingerprint cut | resolved by existing accepted decision | Apply serial D-011 foundation |
| C166-035 | Persistence contract absent from decision log | resolved by existing accepted decision | Apply D-013 in T16/T17 |

## Compiler and schema transitions

### C166-001 — Direct Source specialization versus active `sourceOverrides`

- **Involved:** T1–T3, T7; PRD Decisions 12–21 and 37; ADRs 0001/0009; `CONTEXT.md` Source Overrides; shared delivery §3.
- **Conflicting claims:** T1 makes direct root fragments executable while T7 owns final deletion of the currently active wrapper/operation-list `sourceOverrides` path. The accepted PRD says no wrapper model and no between-ticket compatibility runtime; moved slices delete what they replace.
- **Behavioral consequence:** Two specialization models can be executable with ambiguous precedence, validation, fingerprints, caller behavior, and tests.
- **Affected callers/downstream:** Profile Compiler, Source validation, Source Live Check, Search Run, Detail, T2–T7, and T4b fingerprints.
- **Viable options:** (A) delete/invalidate the old executable path in the first direct-specialization slice; (B) retain only an explicitly non-executable archival shape until T7; (C) let both run. C is rejected; B needs proof it is not a compatibility runtime.
- **Recommendation:** Apply accepted decision D-001 from `handoff/issue-166-contract-decisions.md`: build only retained final modules, expose no second production route, and make the first activation slice atomically migrate every caller and delete the complete old cross-stack path.
- **Proposed canonical owner:** One explicit activation/hard-cut owner; preceding foundation tickets may establish only final modules and interfaces, never wrappers or production dual routing.
- **Latest resolution point:** Decision is accepted; encode it in ticket boundaries before #167 is assigned or keeps `ready-for-agent`.
- **Likely boundary/dependency effect:** Current T1 activation and T7 deletion ownership must be restructured so incomplete fragments are never activated and old deletion is not deferred after activation.
- **Status:** **resolved by existing accepted decision**.

### C166-002 — T4a Policy provenance before authored Policy exists

- **Involved:** T4a/#171, T5/#172, T3b/#170, PRD Decisions 2–3, native dependency graph.
- **Conflicting claims:** T4a promises provenance for Policies. T4a and T5 are independent siblings after T3b, so no authored Policy is guaranteed to exist when T4a lands.
- **Behavioral consequence:** T4a must invent provenance for implicit/compiler-derived behavior, omit a promised terminal, or reopen its serialized contract later.
- **Affected callers/downstream:** Compiler inspection, fingerprinting, T7 provenance renaming, tests and serialized diagnostics/tooling.
- **Viable options:** Exclude compiler-derived policy until T5; move Policy provenance to T5/follow-up; or order T5 before T4a.
- **Recommendation:** Apply accepted decision D-004: T5 lands the final authored/compiled Policy shape first; T4a then freezes complete provenance including Policy terminals.
- **Proposed canonical owner:** T5 for Policy shape; T4a for the complete provenance representation.
- **Latest resolution point:** Decision is accepted; encode T5→T4a before readiness/serialization freeze.
- **Likely boundary/dependency effect:** Apply D-011’s serial order: T5→T6→T4a. Final phase naming lands before complete provenance serialization.
- **Status:** **resolved by existing accepted decision**.

### C166-003 — T4b closed fingerprint inventory versus later runtime semantics

- **Involved:** T4b/#175, T8/#176, T9/#177, T10/#178; PRD Decisions 27–30 and 33.
- **Conflicting claims:** T4b freezes exact components, three behavior-version tokens, and a closed global inventory before T8–T10 replace runtime behavior, add cumulative limits, and add byte/decoding semantics.
- **Behavioral consequence:** Reports can remain falsely fresh if later semantic changes neither change canonical plan material nor bump the correct partition; adding arbitrary global rows would violate T4b.
- **Affected callers/downstream:** Source Live Check preparation/report freshness, activation, compiler/runtime owners.
- **Viable options:** Bind later owners to T4b’s existing partition rules; move T4b after runtime foundation; or extend inventory through a separately accepted decision.
- **Recommendation:** Enforce the existing partition model: plan/compiler changes update canonical material or `profile_compiler`; execution changes bump `profile_runtime`; listed global material/inventory follows T4b’s `immutable_globals` rules. No fourth row without a new decision.
- **Proposed canonical owner:** T4b owns the protocol; T8/T9/T10 own same-PR updates for their behavior.
- **Latest resolution point:** Already decided in T4b; enforce in each later ticket’s readiness/completion review.
- **Likely boundary/dependency effect:** No blocker edge is required; landing order must be inspected at readiness.
- **Status:** **resolved by existing accepted decision**.

### C166-034 — Compiler activation graph and fingerprint cut were incomplete

- **Involved:** D-001, D-004, current T1/T4a/T4b/T6/T7 responsibilities.
- **Conflicting claims:** Early drafts activate direct fragments before old-path deletion; T4a and T6 could land in parallel and force provenance renaming; T4b was placed after the hard cut, leaving activation ownership for old/new fingerprint transition unclear.
- **Behavioral consequence:** Dual productive specialization, reworked provenance, or active schema-v3 Sources without canonical freshness evidence.
- **Affected callers/downstream:** Compiler, Source schema/UI, provenance, Source Live Check, fingerprints, activation and every runtime caller.
- **Viable options:** One oversized rewrite; serial final foundations then atomic activation; temporary wrappers/dual routing. Wrappers/dual routing are rejected.
- **Recommendation:** Apply D-011’s serial foundation: current T1→T2→T3a→T3b→T5→T6→T4a→T4b responsibilities, then one atomic schema-v3 activation that also deletes old fingerprinting.
- **Proposed canonical owner:** Each retained foundation owner plus one explicit activation/hard-cut owner named during restructuring.
- **Latest resolution point:** Decision is accepted; encode before target DAG approval or #167 readiness.
- **Likely boundary/dependency effect:** Replace T4a/T6 parallelism, move T4b foundation before activation, and reshape T7 into the activation owner.
- **Status:** **resolved by existing accepted decision**.

### C166-004 — Transitional `postingDiscovery`/`postingDetail` and complete-value assumptions

- **Involved:** T3a/T6/T7/T12a/T15; `CONTEXT.md`; current DSL docs; PRD Decisions 1, 9–10, 31–32.
- **Conflicting claims:** Early tickets use current `postingDiscovery`/`postingDetail`, complete title/company/URL Discovery, and description-only Detail, while accepted schema v3 uses `discovery` occurrences/provider values/hints and requested multi-field `detail`.
- **Behavioral consequence:** Transitional tests/types may be mistaken for final target contracts or later tickets may be blamed for changing accepted early behavior.
- **Affected callers/downstream:** Compiler fixtures, built-in profiles, Search Run, UI Detail, docs/tests.
- **Viable options:** Treat early shapes as explicit staged baseline and migrate at assigned owners; or pull final posting semantics into early compiler tickets.
- **Recommendation:** Keep staged delivery and label every pre-T7/T12/T15 path/test as readiness-rebaseline material, not a final dependency contract.
- **Proposed canonical owner:** T7 phase names; T12a provider/hint occurrence values; T15 requested Detail; each updates docs in its own slice.
- **Latest resolution point:** At each affected readiness review.
- **Likely boundary/dependency effect:** No graph change unless restructuring intentionally combines those slices.
- **Status:** **documentation-only cleanup**.

## Primitive and phase-result ownership

### C166-032 — Runtime budgets are safety ceilings, not Bot-Detection avoidance

- **Involved:** T9, T10, T14c, T16; PRD boundedness decisions; shared delivery constraints.
- **Conflicting claims:** Current tickets define deterministic request/page/byte/action/duration/candidate ceilings, but they do not consistently state whether these are resource-safety limits, traffic-pacing behavior, or a mechanism intended to prevent Bot Detection.
- **Behavioral consequence:** Implementers could treat ceilings as target traffic patterns, invent stealth/evasion behavior, or assume that a bounded burst is operationally polite. Conversely, removing ceilings would permit unbounded work without solving Prompt Injection or Bot Detection.
- **Affected callers/downstream:** HTTP and browser runtime, Detection, Discovery, Detail, Candidate Resolution, Source Live Check, Search Run, profile authors and tests.
- **Viable options:** Keep ceilings as termination/resource containment; combine them with separately evidenced generic pacing/rate-limit semantics; or remove bounds. Removing bounds is rejected. Anti-Bot evasion, browser-fingerprint manipulation, and CAPTCHA bypass remain excluded.
- **Recommendation:** Describe every deterministic budget as an immutable or tighten-only **safety ceiling**, never a target or Bot-Detection guarantee. Execute only necessary work. Treat concurrency, minimum spacing, rate limits, and standards-based server guidance such as `Retry-After` as a separate generic capability requiring evidence and its own owner. Do not create retry accounting before executable retry behavior exists.
- **Proposed canonical owner:** PRD/shared delivery for the distinction; each budget-owning ticket for its exact ceiling; a future explicit pacing/rate-limit capability for traffic timing.
- **Latest resolution point:** Apply when restructuring T9/T10/T14c/T16 and before any final lean body is approved.
- **Likely boundary/dependency effect:** No new edge for the clarification. A real pacing/retry capability would need its own contract and dependencies rather than being hidden in existing budget tickets.
- **Status:** **resolved by existing accepted decision**.

### C166-005 — No final owner for global authored Primitive completeness

- **Involved:** T11a/T11b/T11c; PRD Decisions 39–40; T14a and later phases.
- **Conflicting claims:** The PRD requires every authored Primitive to have one canonical implementation and global registry completeness. T11a proves only Parse, T11b only six Select keys, and T11c only thirteen Value keys. No ticket owns remaining fetch/pagination/predicate/transform/capture/output/acceptance families or a final global gate.
- **Behavioral consequence:** The series can claim Primitive convergence while hidden/duplicate dispatch remains, or T11c can expand uncontrollably.
- **Affected callers/downstream:** Compiler, all phase runtimes, Detection convergence, completeness tests.
- **Viable options:** Narrow PRD scope; add explicit family owners and a final gate; or expand T11c to all families.
- **Recommendation:** Apply accepted decision D-005: keep T11 families focused, inventory and assign every remaining authored family, and add one implementation-free global completeness gate. Do not silently make T14a the owner.
- **Proposed canonical owner:** Explicit owner per family plus one final global convergence gate.
- **Latest resolution point:** Decision is accepted; encode family ownership during restructuring and require the gate before #166 completion.
- **Likely boundary/dependency effect:** Add or reshape family tickets and one final gate; consumers depend only on families they actually use.
- **Status:** **resolved by existing accepted decision**.

### C166-006 — One result algebra for budget exhaustion, reducers, and T15 dispositions

- **Involved:** T9, original/lean T12b, T13a–c, T15, T16.
- **Conflicting claims:** Published T12b permits reduction of completed accepted inputs at a budget stop. Lean T9/T12b makes `StrategySetBudgetReport` indivisible and exposes no reduced payload on exhaustion. T15 wants exact phase evidence plus per-field `Unavailable`/`Produced`/`Conflicted`, but no payload means no truthful field evidence. Lean T12b’s shown signature also lacks a distinct report-bearing no-payload terminal.
- **Behavioral consequence:** Values may be released contrary to the chosen terminal, the completion report may be flattened/lost, or fields may be falsely classified as unavailable/conflicted/produced.
- **Affected callers/downstream:** Discovery/Detail APIs, all T13 Policies, Source Detail, UI/Live Check, Candidate Resolution/counts.
- **Viable options:** (A) explicit outcome `Completed { envelope, report } | BudgetExhausted { report, diagnostics, no envelope }`, Cancellation outside; (B) allow a safely committed reduced prefix and redefine provenance/terminal semantics; (C) map exhaustion to `Unavailable`. C is rejected.
- **Recommendation:** Apply accepted decision D-003: T12b owns one report-bearing, no-payload budget terminal and shared phase outcome; T15 propagates it without inventing dispositions. The published prefix-reduction contract is intentionally replaced.
- **Proposed canonical owner:** T9 owns the report; T12b owns shared phase outcome/reducer attachment/commit; T15 owns Source projection.
- **Latest resolution point:** Decision is accepted; encode it before T12b readiness and any T13/T15 implementation.
- **Likely boundary/dependency effect:** T12b becomes the exclusive shared result-foundation owner; all T13 siblings and T15 reuse the exact type directly.
- **Status:** **resolved by existing accepted decision**.

### C166-007 — T13 siblings race to own shared result migration

- **Involved:** T13a/#202, T13b/#203, T13c/#204; T9/T12b; native dependencies.
- **Conflicting claims:** All siblings can land independently after #177/#195 but each may become the first owner of the same Policy enums, `Accepted`/`PolicyUnsatisfied` result, report placement, kernel dispatch, exports, and exhaustive callers.
- **Behavioral consequence:** Parallel workers can create duplicate/divergent wrappers or merge-order-dependent migrations.
- **Affected callers/downstream:** Discovery/Detail, Source Live Check, Search Run, T14a (needs `all_required`), tests and exports.
- **Viable options:** Separate result-foundation owner; serialize tickets; or keep first-lander convention.
- **Recommendation:** Apply D-003: T12b is the exclusive result owner. T13 tickets add only their Policy-specific enum/transition/Diagnostic/tests and remove all first-lander clauses.
- **Proposed canonical owner:** T12b.
- **Latest resolution point:** Decision is accepted; encode before any T13 ticket receives readiness.
- **Likely boundary/dependency effect:** Existing T12b blocker relationship is sufficient; no serial dependency among T13 siblings is needed for result ownership.
- **Status:** **resolved by existing accepted decision**.

### C166-008 — T13a lacks siblings’ explicit pre-envelope Cancellation commit point

- **Involved:** T13a/T13b/T13c; shared runtime/result foundation.
- **Conflicting claims:** T13b/c explicitly discard a pure reduction if Cancellation arrives before envelope commitment. T13a discusses per-attempt Cancellation but does not state the same post-reduction/pre-commit check.
- **Behavioral consequence:** One Policy could release an accepted result where another returns Cancellation at the same boundary.
- **Affected callers/downstream:** Discovery/Detail, Search Run cancellation ordering, Source Live Check.
- **Viable options:** Define one shared commit point; or allow Policy-specific commit semantics.
- **Recommendation:** Apply D-003: reducers are pure; check typed Cancellation before committing any accepted envelope; discard computed reduction if Cancellation wins.
- **Proposed canonical owner:** T12b shared outcome/commit contract; every Policy proves it.
- **Latest resolution point:** Decision is accepted; add the missing T13a acceptance/test case before readiness.
- **Likely boundary/dependency effect:** Acceptance/test clarification, no separate graph edge.
- **Status:** **resolved by existing accepted decision**.

## Detection transitions

### C166-009 — Reconciled pre-browser Source Config is unavailable at the required time

- **Involved:** T14a/T14b/T14c; current Detection/browser flow; PRD Decision 47.
- **Conflicting claims:** T14b derives proposal-preparation contributions after accepted Strategy contributions and validates final config after reduction, but browser templates currently require Source Config before browser work. T14b also says conflicts stop dependent work.
- **Behavioral consequence:** Browser templates lose inputs, consume unvalidated/conflicting values, or retain a second ad-hoc merge path.
- **Affected callers/downstream:** Browser Detection profiles, proposal reducer, compiler/context, T14c/T14d.
- **Viable options:** Add a canonical staged reduction/validation checkpoint before dependent Strategies; compile browser inputs directly from typed captures/input and forbid Source Config references; retain a second builder.
- **Recommendation:** Apply D-006: every contribution is reduced immediately into an immutable reconciled state; dependent Strategies read only that state; available values validate before dependent work and final completeness validates before proposal construction.
- **Proposed canonical owner:** T14b reducer/state/validation contract.
- **Latest resolution point:** Decision is accepted; encode before T14b readiness and T14c compilation.
- **Likely boundary/dependency effect:** T14b becomes a final foundation slice; productive activation waits for native Browser contributions in T14c.
- **Status:** **resolved by existing accepted decision**.

### C166-010 — Current browser aggregation destroys overwrite/conflict provenance

- **Involved:** T14b/T14c; current browser evaluator.
- **Conflicting claims:** T14b translates current browser output once into ordered contributions, but current probes mutate one map and overwrite same-key values before returning.
- **Behavioral consequence:** Earlier values and per-probe origins cannot be reconstructed; conflicts remain hidden last-write-wins.
- **Affected callers/downstream:** Proposal values/provenance/diagnostics, T14c migration, T14d deletion proof.
- **Viable options:** Emit one contribution per probe/output before mutation; move browser execution earlier; defer correctness to T14c.
- **Recommendation:** Apply D-006/D-007: do not translate the lossy aggregate. T14c emits native ordered Browser contributions before mutation; the separate cross-phase Browser/Detection activation owner activates only after every final contribution, acquisition, and phase-adapter foundation is complete.
- **Proposed canonical owner:** T14c for Browser contribution emission; T14b for the reusable contribution/reducer contract; D-007 cross-phase owner for activation.
- **Latest resolution point:** Decision is accepted; remove transitional aggregate and T14c-only activation claims during restructuring.
- **Likely boundary/dependency effect:** T14b and T14c are non-productive final foundations; the separate D-007 cross-phase hard cut owns productive activation and old-path deletion.
- **Status:** **resolved by existing accepted decision**.

### C166-011 — T14c replaces a browser seam used outside Detection

- **Involved:** T14c/T14d; shared `ProfileBrowserClient`; Discovery, Detail, Source Live Check, Search Run, posting/UI services and tests.
- **Conflicting claims:** T14c deletes/replaces `render`/`render_with_context`, but declares several non-Detection callers out of scope.
- **Behavioral consequence:** Deletion breaks callers; retention violates hard-cut/guard claims; silent migration changes accounting, teardown, and error behavior outside acceptance coverage.
- **Affected callers/downstream:** All browser-capable phase and product callers.
- **Viable options:** Make the lifecycle boundary shared and migrate every caller with parity coverage; or introduce a Detection-specific lifecycle boundary while retaining a legitimate responsibility-bearing non-Detection client.
- **Recommendation:** Apply D-007: one phase-neutral Browser Acquisition foundation with managed/scripted adapters, followed by one cross-phase activation that migrates every productive caller and deletes the old seam without wrappers.
- **Proposed canonical owner:** Shared Browser foundation plus cross-phase activation; T14c retains Detection-specific Strategy/contribution semantics.
- **Latest resolution point:** Decision is accepted; encode complete caller migration before activation.
- **Likely boundary/dependency effect:** Extract shared lifecycle/caller migration from Detection-only scope; T14d remains guard-only.
- **Status:** **resolved by existing accepted decision**.

### C166-012 — “Recovered fallback” cannot occur under Detection `all_required`

- **Involved:** T14a/T14c/T14d; PRD Decision 26.
- **Conflicting claims:** Detection mandates `all_required` fail-fast and excludes other Policies, while T14c/T14d test an earlier failed/rejected browser attempt followed by accepted fallback.
- **Behavioral consequence:** Acceptance cannot be implemented without silently adding a Policy or redefining rejection.
- **Affected callers/downstream:** Detection runtime, attempt history, convergence guard/tests.
- **Viable options:** Remove rows; name an inner non-Policy fallback; add a real Policy/dependency.
- **Recommendation:** Apply D-007: remove both impossible recovered-later-Strategy rows. Ordered URL alternatives retain their own first-match tests.
- **Proposed canonical owner:** T14c/T14d contract cleanup.
- **Latest resolution point:** Decision is accepted; remove during restructuring.
- **Likely boundary/dependency effect:** No graph change.
- **Status:** **resolved by existing accepted decision**.

### C166-013 — Detection operation-scope browser ledger has no explicit owner

- **Involved:** T14a/T14c; PRD Decision 48.
- **Conflicting claims:** T14a speaks of one cumulative ledger per profile; T14c requires atomic Strategy + profile + entire Detection-operation ceilings across profiles.
- **Behavioral consequence:** Per-profile reset can violate operation ceilings; one undifferentiated ledger can erase profile/Strategy scopes.
- **Affected callers/downstream:** Detection registry iteration, browser Strategies, Diagnostics/usage.
- **Viable options:** Invocation parent ledger with per-profile child scopes; or blocker-led generic multi-scope reservation contract.
- **Recommendation:** Apply D-007: the Detection operation owns an invocation parent, each profile a child scope, and each Strategy its child; every reservation checks all applicable scopes atomically.
- **Proposed canonical owner:** Detection operation for parent lifecycle; T14c for Browser dimensions.
- **Latest resolution point:** Decision is accepted; encode the consumed interface before the foundation lands.
- **Likely boundary/dependency effect:** Consumed-interface and acceptance clarification, no new graph edge.
- **Status:** **resolved by existing accepted decision**.

### C166-014 — Browser teardown residue has no Cancellation projection

- **Involved:** T14a/T14c/T14d.
- **Conflicting claims:** Detection returns typed `DetectionCancelled` and no low-level cancellation Diagnostic; T14c requires one visible teardown residue summary, but no specified result field/type carries it.
- **Behavioral consequence:** Residue is dropped, logged ad hoc, encoded as a forbidden Diagnostic, or silently widens Cancellation.
- **Affected callers/downstream:** Source setup, diagnostics/operational tooling, T14d invariants.
- **Viable options:** Typed bounded residue on `DetectionCancelled`; private operational telemetry; larger operation envelope.
- **Recommendation:** Apply D-007: bounded teardown completes before typed Cancellation returns; teardown residue remains private/testable and is removed from caller-visible claims. Failure to establish cleanup is typed infrastructure failure, not success.
- **Proposed canonical owner:** Shared Browser Acquisition module.
- **Latest resolution point:** Decision is accepted; encode before Browser foundation/activation readiness.
- **Likely boundary/dependency effect:** No public Cancellation-shape expansion; acceptance/reference cleanup only.
- **Status:** **resolved by existing accepted decision**.

### C166-015 — Final proposal validation occurs after dependent external work

- **Involved:** T14b; PRD Decision 47; C166-009.
- **Conflicting claims:** Complete Detection Source Config validates only after full reduction, while browser acquisition may depend on derived config that can later fail schema validation.
- **Behavioral consequence:** External work executes with invalid values and wastes bounded work; “invalid before dependent work” is not met.
- **Affected callers/downstream:** Browser Detection, reducer/validator, proposal Diagnostics.
- **Viable options:** Validate the reconciled dependency slice before each consumer plus final full validation; or forbid config as Strategy input and use typed captures only.
- **Recommendation:** Apply D-006: the same validator performs incremental validation of available values and final complete validation before proposal construction.
- **Proposed canonical owner:** T14b consuming T3b’s shared validator.
- **Latest resolution point:** Decision is accepted; encode before T14b readiness.
- **Likely boundary/dependency effect:** Explicit validation checkpoints, no second validator or graph change.
- **Status:** **resolved by existing accepted decision**.

### C166-029 — T14d must not absorb deletion deferred by earlier slices

- **Involved:** T14a/T14c/T14d; PRD Decision 37; delivery §3.
- **Conflicting claims:** T14a and T14c already own their replaced-path deletions; T14d owns only residue/convergence. Treating T14d as cleanup permission would violate same-slice hard cuts.
- **Behavioral consequence:** Forbidden compatibility paths survive between tickets.
- **Affected callers/downstream:** Entire Detection call graph and guard.
- **Viable options:** Enforce each ticket’s deletion list; or defer known deletion to T14d.
- **Recommendation:** Enforce same-slice deletion and keep T14d guard/residue-only.
- **Proposed canonical owner:** Each moving ticket; T14d only cross-slice residue.
- **Latest resolution point:** Every T14 completion review.
- **Likely boundary/dependency effect:** None if enforced.
- **Status:** **resolved by existing accepted decision**.

## Detail, Candidate Resolution, and persistence

### C166-016 — T15 has no typed non-cancellation failure needed by T16

- **Involved:** T12b/T15/T16.
- **Conflicting claims:** T15 sketches success or `PhaseCancelled`, mapping policy/absence to dispositions, while T16 needs candidate-scoped Detail execution failure distinct from unresolved/unavailable and from Source abort.
- **Behavioral consequence:** T16 must parse Diagnostics, misclassify failures, or invent a terminal outside T15.
- **Affected callers/downstream:** Candidate `failed`/`unresolved` counts, continued processing, samples, T17 visibility.
- **Viable options:** Reuse a typed T12b failure terminal if it exists; add typed non-cancellation failure to T15; remove T16 candidate failure.
- **Recommendation:** Apply D-008: one closed `SourceDetailOutcome`, payload/dispositions only on `Completed`, typed candidate/source failures, and an exhaustive T16 mapping without Diagnostic inspection.
- **Proposed canonical owner:** T12b owns typed unsatisfied cause; T15 owns Source Detail outcome; T16 owns candidate/Source mapping.
- **Latest resolution point:** Decision is accepted; encode before T15 readiness.
- **Likely boundary/dependency effect:** Existing T15→T16 edge is sufficient; T12b’s shared result contract gains only a closed cause classification, not public Attempt state.
- **Status:** **resolved by existing accepted decision**.

### C166-017 — T16 retries have no executable/accounting owner

- **Involved:** T9/T10/T16; PRD Decision 42.
- **Conflicting claims:** T9/T10 explicitly omit retry accounting until executable retry exists; T16 includes `max_retries`, retry stop/usage, and boundary tests.
- **Behavioral consequence:** Dead always-zero API, invented retry semantics, or false usage reporting.
- **Affected callers/downstream:** Resolution limits/results, tests, fingerprints, T17 serialized visibility.
- **Viable options:** Remove/defer retry dimension; define structurally zero; add an accepted executable retry owner/dependency.
- **Recommendation:** Apply D-002: remove retry fields/dimensions/tests from initial T16. Add them only with a real executable retry capability and truthful charging owner.
- **Proposed canonical owner:** Future executable retry capability, then Candidate Resolution aggregation.
- **Latest resolution point:** Decision is accepted; remove during restructuring before T16 readiness.
- **Likely boundary/dependency effect:** No graph change now; a future capability requires its own ticket/edge.
- **Status:** **resolved by existing accepted decision**.

### C166-018 — SCHOTT smoke expects hint-derived canonical values and unbounded artifacts

- **Involved:** `docs/dev-search-run-smoke.md`; T12a/T16/T17; PRD Decisions 31–32 and 41–46.
- **Conflicting claims:** Smoke expects URL-derived canonical title/location and raw `search-run-candidates.json`; accepted target allows URL-derived hints only for explicit reject-only prefilter, requires provider values for final matches, and bounds/finalizes retained outputs.
- **Behavioral consequence:** Correct target behavior fails the smoke; satisfying the smoke can persist guessed values or retain unbounded candidate payload.
- **Affected callers/downstream:** Manual smoke, SCHOTT profile evidence, Search Run artifacts, persistence.
- **Viable options:** Obtain genuine provider values; narrow smoke to honest unresolved/reference behavior; replace raw artifact with bounded Resolution summary/samples.
- **Recommendation:** Apply D-013: T16 updates bounded Resolution smoke output and removes URL-derived canonical assertions; the relevant profile evidence owner supplies genuine provider extraction; T17 updates durable-row/Match assertions and post-commit artifact behavior.
- **Proposed canonical owner:** T16 for Resolution smoke shape; profile evidence owner for extraction; T17 for persistence assertions/artifacts.
- **Latest resolution point:** Same implementation/document slices, before declaring T16/T17 done.
- **Likely boundary/dependency effect:** Documentation migration targets, no new tracker edge.
- **Status:** **resolved by existing accepted decision**.

### C166-019 — ADR 0008 rejects persistence required by T17

- **Involved:** ADR 0008; T17/#234; `CONTEXT.md` Match.
- **Conflicting claims:** ADR 0008 says no durable Search Run history or Search Request relationship; T17 requires `search_runs`, normalized Match→Run→Request links, reruns, retention, and cascades.
- **Behavioral consequence:** Two active architecture directions guide schema/service work oppositely.
- **Affected callers/downstream:** SQLite migrations, SearchRunService, importer, UI/future history consumers.
- **Viable options:** Supersede conflicting ADR clauses; reject T17; create a new superseding ADR.
- **Recommendation:** Apply D-013: T17 supersedes only no-history/no-link clauses in the same slice, preserving ADR 0008’s durable Job Posting/manual-state/dedupe/update rules unless separately changed.
- **Proposed canonical owner:** T17.
- **Latest resolution point:** Atomically with T17 schema/code.
- **Likely boundary/dependency effect:** Documentation remains part of T17 scope; no extra blocker.
- **Status:** **resolved by existing accepted decision**.

### C166-020 — T16 `remaining` recurrence is ambiguous

- **Involved:** T16 original/lean batch protocol.
- **Conflicting claims:** `remaining` is exact not-yet-emitted count “at that point,” but the contract does not state whether it is measured before or after the current batch.
- **Behavioral consequence:** Adapters disagree by one batch; valid data can trigger inconsistency diagnostics and wrong summaries.
- **Affected callers/downstream:** Production/scripted Discovery adapters, Resolution and Search Run summaries, T17 report assertions.
- **Viable options:** Post-batch remainder with checked recurrence; pre-batch remainder; adapter-owned exactness when provider-side filtering prevents inference.
- **Recommendation:** Apply D-009: `remaining` is post-batch; consecutive exact values use checked subtraction by newly emitted occurrences; adapters unable to guarantee exactness return `None`; contradictions invalidate to `None` with bounded sanitized Diagnostics.
- **Proposed canonical owner:** T16 batch protocol.
- **Latest resolution point:** Decision is accepted; encode before T16 readiness/fake tests.
- **Likely boundary/dependency effect:** Contract precision and tests only.
- **Status:** **resolved by existing accepted decision**.

### C166-031 — Candidate Resolution lacks explicit non-double-counting parent-budget accounting

- **Involved:** T9, T15, T16.
- **Conflicting claims:** T16 owns cumulative Source Resolution limits across repeated Discovery batches and Detail calls, while T9’s ledger is scoped to one phase invocation. T16 also describes child usage/debits without defining whether parent reservation plus child charging double-counts or how a resolution-level one-over is prevented before side effects.
- **Behavioral consequence:** Reserving in both layers double-charges; aggregating only after child execution cannot prevent an over-limit side effect; resetting each child violates cumulative Resolution ceilings.
- **Affected callers/downstream:** Candidate Resolution, Discovery/Detail adapters, usage/completion reports, PartialReason and deterministic tests.
- **Viable options:** Resolution parent allowance that passes remaining limits as caller tightening and commits each child report once; shared mutable public ledger; post-hoc aggregation only.
- **Recommendation:** Apply D-009: T16 owns a private Resolution parent allowance, passes remaining limits as T9 caller tightening, commits each exact child report once, and directly debits only Candidate-owned dimensions. Mid-candidate exhaustion yields `unresolved`; emitted unstarted candidates become `budgetSkipped`.
- **Proposed canonical owner:** T16/#233, consuming T9/T15 reports.
- **Latest resolution point:** Decision is accepted; encode before T16 readiness and fake/protocol freeze.
- **Likely boundary/dependency effect:** Existing transitive blockers suffice; add explicit consumed-interface and acceptance tests, not a new dependency.
- **Status:** **resolved by existing accepted decision**.

### C166-035 — Finalized-only persistence and ADR supersession were absent from the decision log

- **Involved:** T16, T17, ADR 0008, C166-018/C166-019/C166-021/C166-027/C166-028.
- **Conflicting claims:** Candidate decisions ended before persistence while active ADR 0008 rejected durable Search Run history/links and T17 required them; the sole finalized constructor, artifact boundary, and cancellation release rule were not explicitly logged.
- **Behavioral consequence:** Non-final values could bypass Candidate Resolution, persistence authority could remain contradictory, or cancellation/partial results could be committed inconsistently.
- **Affected callers/downstream:** Candidate Resolution, cross-Source merge, importer, SQLite schema/transaction, Search Run result/artifact, ADR consumers.
- **Viable options:** Explicit finalized-only atomic persistence and ADR supersession; retain ADR 0008 no-history model; persist candidate/runtime state. The latter two conflict with the accepted target.
- **Recommendation:** Apply D-013: sole productive finalized constructor, two-stage identity/trust boundary, atomic Search Run/Posting/Source/Match transaction, no runtime-state tables, bounded post-commit artifacts, and same-slice ADR 0008 supersession.
- **Proposed canonical owner:** T16 for finalized handoff; T17 for merge/persistence/deletion/docs.
- **Latest resolution point:** Decision is accepted; encode before target DAG/ticket approval.
- **Likely boundary/dependency effect:** Preserve T16→T17 and make ADR/artifact/call-graph deletion evidence part of T17’s atomic scope.
- **Status:** **resolved by existing accepted decision**.

### C166-021 — Finalized-only boundary can be bypassed by broad constructors

- **Involved:** T17, current posting importer/SearchRun result types.
- **Conflicting claims:** Accepted T17 requires one production path from finalized values, while tests and broad constructors can create normalized postings directly.
- **Behavioral consequence:** Non-final values can bypass T16 despite passing SQL tests.
- **Affected callers/downstream:** Search Run merge/import/persistence and artifacts.
- **Viable options:** Keep importer/internal constructors narrow and trace production call graph; encode finalization in SQL; add duplicate runtime gate.
- **Recommendation:** Apply D-013: keep importer/internal constructors narrow, make `FinalizedCandidate` conversion the sole productive construction path, and allow direct posting fixtures only in tests. Do not duplicate candidate-state validation in SQL.
- **Proposed canonical owner:** T16 finalized handoff and T17 migration/deletion review.
- **Latest resolution point:** T17 completion.
- **Likely boundary/dependency effect:** No new ticket; requires static call-graph evidence.
- **Status:** **resolved by existing accepted decision**.

### C166-027 — Source-local Posting Occurrence identity versus cross-Source Job Posting dedupe

- **Involved:** T12a/T12b/T13c/T16/T17; PRD Decisions 24 and 46; `CONTEXT.md` Job Posting; ADR 0008.
- **Conflicting claims:** Occurrence identity is Source+provider ID or Source+normalized URL and excludes title/company/location; cross-Source Job Posting dedupe uses normalized company/title/location behavior.
- **Behavioral consequence:** Unifying them either prevents legitimate cross-Source merge or incorrectly merges distinct same-Source occurrences.
- **Affected callers/downstream:** Discovery reducer, Detail, Candidate Resolution, merge/import.
- **Viable options:** Keep two-stage identity; unify.
- **Recommendation:** Keep the accepted two-stage model. T12a owns occurrence identity; T17/backend merge owns cross-Source dedupe after final normalization.
- **Proposed canonical owner:** T12a and T17 respectively.
- **Latest resolution point:** Already decided; enforce throughout implementation.
- **Likely boundary/dependency effect:** Preserve existing downstream separation.
- **Status:** **resolved by existing accepted decision**.

### C166-028 — Provider values versus hints

- **Involved:** T12a/T16/T17; PRD Decision 32; Phase-2 constraints.
- **Conflicting claims:** Hints can have canonical-looking keys but remain noncanonical; only explicit `search_prefilter` may reject early; provider values alone can normalize/finalize/persist.
- **Behavioral consequence:** Hint-to-value conversion persists guesses; canonical-key denylist wrongly rejects valid hints.
- **Affected callers/downstream:** Discovery schema/reducer, Candidate prefilter, matching, persistence.
- **Viable options:** Structural separation; generic tagged contribution bag; selected hint promotion.
- **Recommendation:** Preserve structural separation and reject any hint promotion to canonical values.
- **Proposed canonical owner:** T12a representation; T16 prefilter; T17 persistence gate.
- **Latest resolution point:** Already decided; enforce in schema/API/tests.
- **Likely boundary/dependency effect:** None.
- **Status:** **resolved by existing accepted decision**.

### C166-033 — Failure and Cancellation terminals must preserve exact usage

- **Involved:** T9, T12b, T15, T16; D-003, D-008, D-009.
- **Conflicting claims:** T9 requires exact committed usage on every terminal and T16 must commit each child report once, while the initial D-003/D-008 sketches omitted the report from execution failure and Cancellation terminals.
- **Behavioral consequence:** Failed or cancelled external work could be lost from accounting or reconstructed from Diagnostics, breaking truthful parent budgets and observability.
- **Affected callers/downstream:** Phase runtime, Source Detail, Candidate Resolution parent allowance, Cancellation projection and tests.
- **Viable options:** Common report envelope on every started terminal; reconstruct usage; omit failed/cancelled usage. Reconstruction and omission are rejected.
- **Recommendation:** Apply D-010: every started success/failure/budget/Cancellation terminal carries the complete report independently from domain payload; cancellation evidence remains transient and non-persistable.
- **Proposed canonical owner:** T12b common phase envelope; T15 exact projection; T16 one-time parent commit for non-cancelled outcomes.
- **Latest resolution point:** Decision is accepted; encode before T12b/T15/T16 readiness.
- **Likely boundary/dependency effect:** Result-shape and exhaustive-test updates, no new dependency.
- **Status:** **resolved by existing accepted decision**.

## Documentation and tracker-reference cleanup

### C166-022 — Handoff inventory references deleted artifacts

- **Involved:** `handoff/README.md`; Phase-1 matrix; Phase-2 handoff.
- **Conflicting claims:** README says ticket snapshots/archive/worker handoff are retained and directs work through them; Phase-2 authority says they were intentionally deleted and GitHub is the original source. The matrix references the missing old template.
- **Behavioral consequence:** Future reviewers may restore obsolete files or use nonexistent/noncanonical sources.
- **Affected callers/downstream:** Human/agent workflow only.
- **Viable options:** Update navigation; restore files.
- **Recommendation:** Update README/matrix after review approval; never restore deleted snapshots/archive/worker handoff; defer a new lean template until restructuring.
- **Proposed canonical owner:** Handoff metadata cleanup.
- **Latest resolution point:** Before next restructuring session relies on README.
- **Likely boundary/dependency effect:** None.
- **Status:** **documentation-only cleanup**.

### C166-023 — PRD says T16 sample limit is still an open gate

- **Involved:** PRD Decision 49; T16/#233 original/lean.
- **Conflicting claims:** PRD says T16 is not ready until a concrete limit is accepted; T16 records accepted immutable limit 10.
- **Behavioral consequence:** Readiness reviewers may treat a settled decision as open.
- **Affected callers/downstream:** Tracker planning only.
- **Viable options:** Update PRD; reopen decision.
- **Recommendation:** Update Decision 49 to “gate satisfied; limit 10,” without altering T16.
- **Proposed canonical owner:** PRD documentation cleanup.
- **Latest resolution point:** Before restructuring uses PRD gate text.
- **Likely boundary/dependency effect:** None.
- **Status:** **documentation-only cleanup**.

### C166-024 — T3b incorrectly attributes schema composition to T3a

- **Involved:** Lean T3b; T3a; PRD Decision 47.
- **Conflicting claims:** Lean T3b says T3a owns the profile/path Source Config composition rule; T3a explicitly preserves current behavior and T3b finalizes the contract.
- **Behavioral consequence:** Implementers can treat a pre-T3b detail as accepted blocker API.
- **Affected callers/downstream:** T3b compiler/Detection validator planning.
- **Viable options:** Correct ownership; change T3a scope.
- **Recommendation:** Cite T3b itself plus PRD Decision 47.
- **Proposed canonical owner:** T3b lean-body cleanup.
- **Latest resolution point:** Before publication/assignment.
- **Likely boundary/dependency effect:** None.
- **Status:** **documentation-only cleanup**.

### C166-025 — T11b incorrectly attributes shared item representation to T11a

- **Involved:** Lean T11a/T11b.
- **Conflicting claims:** T11b says T11a supplies parsed-document/item representations; T11a defers `RuntimeItem` movement to T11b.
- **Behavioral consequence:** T11b can wait for a predecessor output that will not exist or duplicate item migration.
- **Affected callers/downstream:** Selector runtime/value integration.
- **Viable options:** Correct reference; move item responsibility to T11a.
- **Recommendation:** State T11a supplies `ParsedDocument`; T11b owns selected-item/`RuntimeItem` consolidation.
- **Proposed canonical owner:** T11b lean-body cleanup.
- **Latest resolution point:** Before publication/assignment.
- **Likely boundary/dependency effect:** None.
- **Status:** **documentation-only cleanup**.

### C166-026 — Lean T9 omits current downstream navigation

- **Involved:** Lean T9; native GitHub dependencies.
- **Conflicting claims:** Native graph says #177 blocks #178/#202/#203/#204; lean T9 omits a blocking list.
- **Behavioral consequence:** Local navigation is incomplete, though tracker authority is unaffected.
- **Affected callers/downstream:** Review workflow only.
- **Viable options:** Add compact links; explicitly rely on native GitHub.
- **Recommendation:** Prefer a compact “blocking” line or state that native GitHub is sole relationship source.
- **Proposed canonical owner:** T9 lean-body cleanup.
- **Latest resolution point:** Optional before publication.
- **Likely boundary/dependency effect:** None; do not infer sibling ordering from this cleanup.
- **Status:** **documentation-only cleanup**.

### C166-030 — Lean T7 references deleted published-ticket snapshots

- **Involved:** Lean T7; Phase-2 handoff.
- **Conflicting claims:** T7 says leave published ticket snapshots unchanged; those snapshots were intentionally deleted and must not be restored.
- **Behavioral consequence:** A future worker may recreate or rely on noncanonical snapshots.
- **Affected callers/downstream:** Documentation workflow only.
- **Viable options:** Replace with live GitHub wording; restore snapshots.
- **Recommendation:** Remove the phrase or refer to live GitHub originals/historical handoff records.
- **Proposed canonical owner:** T7 lean-body cleanup.
- **Latest resolution point:** Before publication.
- **Likely boundary/dependency effect:** None.
- **Status:** **documentation-only cleanup**.

## Recommended decision order before restructuring

1. Apply accepted **D-001/D-011 and C166-001/C166-034**: use the serial retained compiler/Policy/naming/provenance/fingerprint foundation, then one atomic schema-v3 activation with no wrapper or dual production path.
2. Apply accepted **D-003/C166-006–008**: T12b owns the closed no-payload budget terminal, shared outcome, and commit point; T13 tickets reuse it without first-lander logic.
4. Apply accepted **D-006/C166-009/C166-010/C166-015**: one incremental reconciled Detection state, native ordered Browser contributions, shared validation checkpoints, and one activation cut.
5. Apply accepted **D-007/C166-011–014**: shared phase-neutral Browser Acquisition, one cross-phase hard cut, scoped parent/child accounting, private teardown residue, and no impossible fallback rows.
6. Apply accepted **D-008/C166-016**, **D-002/C166-017**, and **D-009/C166-020/C166-031** for typed T15→T16 mapping, retry removal, post-batch remainder, and non-double-counting parent/child budgets.
7. Apply accepted **D-004/C166-002** by ordering T5 before T4a, apply **D-005/C166-005** family ownership and final gate, and enforce C166-003 fingerprint partitions.
8. Apply C166-032 consistently: safety ceilings remain boundedness controls; pacing/rate limits are separate, and no Bot-Detection-evasion behavior is implied.
9. Apply accepted **D-010/C166-033** so every started non-domain terminal preserves exact usage without exposing partial domain payload.
10. Apply accepted **D-013/C166-035** to the finalized-only T16→T17 handoff, atomic persistence, artifacts, and ADR 0008 supersession.
11. Apply documentation-only fixes and accepted-decision documentation migrations without changing product semantics.

Only after these review decisions should ticket boundaries and native dependencies be restructured.
