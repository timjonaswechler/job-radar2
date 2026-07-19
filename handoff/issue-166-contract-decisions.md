# Issue #166 — Phase-1 Contract Decisions

Status: **Phase-1 decision gate complete — local only, no GitHub changes approved**  
Baseline: **2026-07-18**

This document records reviewed product and architecture decisions and accepted boundary constraints that must govern ticket restructuring. Current ticket labels are used only to identify existing responsibilities; the final ticket set and complete dependency graph remain to be designed.

## D-001 — Direct Source Specialization is a clean hard replacement

Status: **accepted**

### Decision

Direct Source Specialization becomes the sole Source-specific behavior-specialization model. The refactor optimizes for the clean final system, not compatibility with the current `sourceOverrides` representation.

The implementation must not introduce or retain:

- a compatibility wrapper around `sourceOverrides`;
- an old-to-new override translator;
- a runtime that interprets both authored models;
- aliases, forwarding functions, migration-only DTOs, or duplicate compiler paths;
- a fallback from Direct Source Specialization to `sourceOverrides`;
- production caller choice between old and new specialization models.

New compiler, merge, schema, provenance, and validation logic is implemented directly in its final responsibility and interface. Preparatory work may establish those final modules and test them through their final interface, but it must not expose a second production authoring or execution route.

The activation slice performs one atomic hard cut:

1. make complete Direct Source Specialization authorable and executable;
2. move every production caller directly to the final compiler interface;
3. rewrite Source Schema, Rust/TypeScript documents, create/edit UI, fixtures, tests, fingerprints, and active documentation;
4. delete `sourceOverrides`, `strategyOverrides`, the override compiler, schemas, UI/editor helpers, exports, tests, and documentation;
5. prove by repository search and caller tests that no old executable or authoring path remains.

Delivery uses focused **final-foundation slices followed immediately by one atomic activation/hard-cut slice**, rather than one oversized rewrite ticket. Before activation, the old path may remain the sole productive authoring/execution path while new modules exist only through their final interfaces and are not externally authorable. This temporary code coexistence is permitted; dual production authoring/execution is not.

Activation waits until the retained foundation supports at least:

- specialization of existing keyed Access Paths and Strategies;
- complete new Strategies and Access Paths;
- deterministic keyed merge and whole-array replacement;
- the Effective Source Config Schema and shared validator;
- final schema-v3 phase names;
- mandatory Strategy Policy shape;
- complete Effective Source Profile and compiler validation.

The activation ticket owns the entire cross-stack cut and deletion. No later cleanup ticket may inherit known `sourceOverrides` residue.

### Rationale

The current `sourceOverrides` model is a real cross-stack production surface. Removing it before the replacement is complete would create a capability regression; keeping it active after Direct Source Specialization is activated would create ambiguous precedence and long-lived architectural debt. A hard replacement preserves a clean final model without wrappers or dual execution.

### Consequences for restructuring

- Current T1 cannot both activate incomplete Direct Source Fragments and defer old-path deletion to T7.
- Early compiler work must produce retained final modules, not temporary adapters.
- Foundation tickets must remain sequentially focused and feed directly into activation; they may not expose a second productive authored model.
- One explicit activation/hard-cut owner covers backend, frontend, schemas, callers, fingerprints, tests, docs, and complete old-path deletion.
- Known deletion work cannot be deferred to a later cleanup/convergence ticket.
- Existing Source documents require no compatibility migration because this is an approved pre-production hard cut.

## D-002 — Runtime budgets are safety ceilings, not Bot-Detection behavior

Status: **accepted**

### Decision

Deterministic request, page, byte, action, duration, candidate, and enrichment limits are termination and resource-containment safety ceilings. They are not target traffic patterns and do not claim to prevent Bot Detection or Prompt Injection.

Pacing, concurrency policy, rate limiting, and standards-based handling such as `Retry-After` require a separate generic, evidence-backed capability and owner. Retry fields or accounting must not exist before executable retry behavior exists. Anti-Bot evasion, browser-fingerprint manipulation, CAPTCHA bypass, and provider-specific stealth behavior remain excluded.

The final Detection HTTP route reuses the existing immutable **67,108,864-byte (64 MiB)** HTTP `response_bytes` value as one cumulative backend safety ceiling per public Detection operation. One allowance spans every HTTP check in that operation; D02 passes only the remaining allowance through P09 to H01. H01 remains the sole collector/counter and D02 owns typed Detection projection/reporting. Exact-boundary work may complete. Known-cost work above the remaining allowance does not start; an already-started stream may prove one excess byte, charges only its admitted prefix, commits no response or domain payload, and starts no later work. This adds no authored Detection byte field, provider/Profile/host override, Discovery/Detail O03 outcome transfer, or new capability, and HTTP `response_bytes` remains distinct from Browser `browser_rendered_bytes`.

### Consequences for restructuring

- T9/T10/T14c/T16 must describe ceilings consistently as safety controls.
- T16’s unowned retry dimension is removed or deferred to a real retry capability.
- No budget ticket silently acquires pacing, stealth, or Bot-Detection responsibilities.
- H01 owns the one HTTP collector/counter; D02 supplies and projects the cumulative Detection-operation allowance without creating a second ledger or authored limit.

## D-003 — One closed phase outcome and commit boundary

Status: **accepted**

### Decision

Discovery and Detail use one shared typed phase-outcome algebra after T9:

```text
PhaseOutcome
├── Completed
│   ├── Accepted { reducedPayload }
│   └── PolicyUnsatisfied
├── BudgetExhausted { completeBudgetReport, diagnostics }
└── ExecutionFailed { typedFailure, diagnostics }
```

Typed Cancellation remains outside this outcome as control flow.

Only `Completed::Accepted` may contain a reduced Posting Occurrence or requested Detail patch and its contribution provenance/conflicts/rejections. `PolicyUnsatisfied`, `BudgetExhausted`, `ExecutionFailed`, and Cancellation expose no phase payload.

Budget Exhaustion always preserves the complete indivisible Strategy Set budget report and ordered safe Diagnostics. It never becomes a per-field `Unavailable`, `Produced`, or `Conflicted` disposition because exhaustion supplies no field evidence.

Every Policy uses the same commit point: phase reducers are pure; after reduction, typed Cancellation is checked before the `Completed::Accepted` envelope is committed. If Cancellation wins, the computed reduction is discarded.

At the Source Detail interface, Budget Exhaustion and genuine Execution Failure remain typed operation-level outcomes. T16 maps those outcomes without inspecting Diagnostic text. Already finalized candidates from earlier completed candidate operations may remain usable for non-cancellation Resolution Partial Completion; Cancellation does not automatically release them.

### Ownership

- T9 owns the complete budget report and exhaustion facts.
- T12b owns the shared phase-outcome/result migration, reducer attachment, commit point, exports, and exhaustive caller migration.
- T13 Policy tickets add only their authored/compiled Policy variant, kernel transition, terminal Diagnostic, and Policy tests. They do not use a first-lander rule and do not redefine the result algebra.
- T15 owns Source-level projection of the typed phase outcome.
- T16 owns candidate-state and Source-resolution mapping.

### Replaced contract

This explicitly replaces the published T12b rule that budget stopping may expose a reduced prefix of completed accepted contributions. The replacement is intentional: a budget-terminal phase exposes no reduced payload. The former Phase-2 `contract loss` is therefore resolved by this accepted semantic decision rather than restored.

### Consequences for restructuring

- T12b becomes the exclusive shared result-foundation owner after T9/T12a.
- Remove all T13 “first sibling to land” clauses; T13 siblings may remain independent after the shared T12b outcome lands.
- Add the common post-reduction/pre-commit Cancellation case to T13a.
- T15 must not fabricate field dispositions on budget exhaustion.
- D-008 resolves the exact typed `ExecutionFailed` content and T15→T16 mapping; restructuring must apply both decisions together.

## D-004 — Authored Policy lands before complete Effective Profile provenance

Status: **accepted**

### Decision

The final authored and compiled Strategy Policy contract lands before T4a freezes complete Effective Source Profile provenance. T5 is therefore a direct blocker of T4a:

```text
T3b → T5 → T4a
          ├→ T6 may continue on its independent naming branch
          └→ T4a feeds later provenance/fingerprint consumers
```

T5 owns the authored and compiled mandatory `first_accepted` Policy shape. T4a subsequently includes Policy terminals in the one complete final provenance representation.

T4a must not:

- invent provenance for the current implicit fallback behavior;
- omit Policy provenance and require a later format extension;
- introduce a temporary Policy-origin variant;
- replay or diff compiler output to retrofit Policy provenance.

D-011 refines the ordering further: T6 lands before T4a so provenance is created once with final internal phase names. D-001 activation waits for the complete serial compiler/Policy/naming/provenance/fingerprint foundation.

### Consequences for restructuring

- Add T5 as a direct blocker of T4a in the proposed target graph.
- Remove current prose that treats T4a and authored Policy as independent.
- T4a serializes the complete final provenance format once.
- T4b remains downstream of complete T4a provenance and directly precedes schema-v3 activation under D-011.

## D-005 — Every authored Primitive family has one owner and one global completeness gate

Status: **accepted**

### Decision

The global single-ownership invariant from PRD Decisions 39–40 remains strict. Every authored DSL Primitive variant has exactly one canonical Rust implementation owner. Parse, Select, and Value remain focused families; T11c is not expanded into a catch-all migration.

Every remaining authored family—such as acquisition/fetch, pagination, predicates, acceptance, capture, transforms, or other actually admitted variants—must receive an explicit implementation owner during restructuring. A family ticket:

1. implements or moves each owned Primitive directly into its final canonical file;
2. migrates real compiler/runtime callers;
3. deletes duplicate behavior, dispatch logic, helpers, fakes, aliases, and superseded tests in the same slice;
4. extends schema/document/compiled registration parity for only its real variants.

Placeholder registrations, rejection stubs for unimplemented future variants, and hidden Primitive behavior in registry/dispatch modules are prohibited.

After all admitted families have owners, one implementation-free global convergence gate verifies:

- parity among authored Schema/Serde variants and compiled registrations;
- missing and duplicate registrations;
- exactly one canonical implementation owner per admitted Primitive;
- absence of Primitive-specific behavior in registry/dispatch modules.

The gate adds no runtime Capability and does not become a generic plugin interface.

### Dependency rule

A capability ticket depends only on the Primitive families it actually consumes, not automatically on global convergence. The global gate blocks completion of the #166 series: the series cannot claim complete Primitive consolidation while any admitted family lacks an owner.

Future Primitive additions extend the gate only in the same slice that introduces a real executable Capability and its canonical owner.

### Consequences for restructuring

- Keep T11a/T11b/T11c focused on Parse/Select/Value.
- Inventory every remaining authored Primitive variant and map it to an explicit owner.
- Add one final global registry-completeness/convergence gate after those owners.
- Do not silently assign omitted families to T14a or another consumer.

## D-006 — Detection uses one incremental reconciled state and one activation cut

Status: **accepted**

### Decision

Detection uses one ordered contribution pipeline and one conflict-safe reducer:

```text
Strategy output
→ ordered DetectionContribution
→ conflict-safe reducer
→ immutable ReconciledDetectionState
→ next Strategy
```

Every URL, HTTP, and Browser Strategy emits its evidence, captures, Source Config contributions, Access Path recommendation, and complete origin as individual ordered contributions before any same-key mutation can discard information.

The reducer is the sole owner of equal-value reconciliation, conflict detection, origin aggregation, retained values, and deterministic ordering. Each successful reduction yields an immutable state snapshot. Subsequent Strategy templates may read only that reconciled state; no second mutable capture/config map or proposal builder exists.

The shared Effective Source Config validator exposes two operations over the same constrained contract and implementation:

- incremental value validation checks every currently available value and applicable property constraint while allowing not-yet-produced required fields;
- final complete validation checks the complete composed contract and all required fields before Source Proposal construction.

A conflict or invalid available dependency stops the profile before dependent external work.

### Browser transition and activation

The current aggregated browser map is not translated into contributions because overwritten values and origins cannot be reconstructed safely. T14a/T14b establish only retained final Strategy, contribution, reducer, and state modules. T14c makes Browser Strategies emit native contributions directly.

The new Detection route becomes productive only after URL, HTTP, and Browser Strategies all use the final contribution/state model. One atomic activation/hard-cut slice then migrates the productive Detection callers and deletes:

- mutable browser capture/config aggregation;
- old URL/HTTP/browser evaluators;
- duplicate proposal builders and merge paths;
- compatibility operations, wrappers, aliases, and old fakes.

T14d remains a convergence/guard proof and may not inherit known migration or deletion work.

### Ownership

- T14a owns final URL/HTTP compiled Strategy execution inputs and outputs.
- T14b owns `DetectionContribution`, incremental reducer, `ReconciledDetectionState`, shared validation checkpoints, and sole final proposal construction.
- T14c owns native Browser Strategy contributions and Detection-specific ceilings, but not the cross-phase activation by itself.
- The D-007 Browser/Detection cross-phase activation owner migrates all productive browser and Detection callers after every final foundation is complete.
- T14d owns residue verification/guard only.

### Consequences for restructuring

- Remove T14b’s transitional aggregate-browser translation contract.
- Do not promise complete productive browser provenance before T14c.
- Add explicit incremental-state consumption to every dependent Strategy interface.
- Combined D-007 activation requires complete URL/HTTP/Browser contribution support, the final shared Browser Acquisition module, every phase adapter, and same-slice old-path deletion.

## D-007 — One shared phase-neutral Browser Acquisition module and one cross-phase hard cut

Status: **accepted**

### Decision

Browser process/acquisition behavior lives in one phase-neutral module behind one proven seam with two real adapters:

```text
Detection adapter ─┐
Discovery adapter ─┼→ Browser Acquisition module
Detail adapter ────┘      ├→ managed production adapter
                            └→ scripted deterministic adapter
```

The shared module owns browser process lifecycle, navigation/actions, cancellation-aware waits, content acquisition, bounded teardown, and safe infrastructure failure classification. It does not own phase acceptance, reducers, phase Diagnostics, or phase output projection.

Detection, Discovery, and Detail retain their typed adapters, inputs/outputs, acceptance/reducer semantics, and phase-specific budget scopes. Detection browser ceilings do not become Discovery/Detail limits merely because acquisition is shared.

### Clean activation

The final Browser Acquisition module and both real adapters may be established through their final interface before activation. One cross-phase activation/hard-cut slice then migrates every productive browser caller directly—including Detection, Discovery, Detail, Source Live Check, Search Run, posting/UI paths, commands, and deterministic tests—and deletes:

- `ProfileBrowserClient` and `render*` operations;
- old browser acquisition implementations;
- forwarding helpers, wrappers, aliases, exports, and duplicate fakes;
- superseded caller and implementation-detail tests.

No wrapper forwards the old seam to the new module, and old/new productive browser seams do not coexist after activation.

### Budget scopes

Each owning phase supplies a typed parent allowance. Detection uses an invocation parent with profile and Strategy child scopes. Every Browser reservation atomically checks every applicable Strategy, profile, and operation scope. Discovery and Detail supply their own phase-appropriate scopes.

### Teardown and Cancellation

Teardown is part of the bounded operation. The module does not return until graceful close or bounded forced termination/reap has established the cleanup invariant. Failure to establish that invariant is typed `BrowserInfrastructureFailure`, never success.

On Cancellation, bounded teardown completes first and the caller receives only typed Cancellation. Teardown residue remains private, bounded, and testable; it is not added to `DetectionCancelled`, public phase outcomes, or Diagnostics. Safe terminal infrastructure classification is exposed only when operationally necessary.

### Detection Policy correction

The T14c/T14d recovered-later-Strategy fallback rows are removed. Mandatory Detection `all_required` is fail-fast and has no such recovery route. Ordered URL alternatives retain their separate first-match semantics.

### Consequences for restructuring

- Extract shared Browser foundation and cross-phase activation from the Detection-only scope.
- T14c retains Detection-specific Browser Strategy compilation, contributions, and Detection ceilings.
- T14d remains a convergence guard and owns no known migration.
- Every phase adapter proves parity through the shared module and appropriate production/scripted adapter.

## D-008 — Source Detail exposes one typed outcome mapped explicitly into Candidate Resolution

Status: **accepted**

### Decision

T15 exposes one closed Source Detail outcome:

```text
SourceDetailOutcome
├── Completed { fields, dispositions, phaseEvidence }
├── BudgetExhausted { completeBudgetReport, diagnostics }
├── CandidateExecutionFailed { typedFailure, diagnostics }
├── SourceExecutionFailed { typedFailure, diagnostics }
└── SourceMismatch
```

Typed Cancellation remains outside:

```text
Result<SourceDetailOutcome, DetailCancelled>
```

Only `Completed` contains requested field values and dispositions. `Unavailable` means the complete ordinary Detail execution ended without an accepted value for a supported field. Budget Exhaustion, Cancellation, and execution failure never become `Unavailable` and expose no Source Detail field result.

The shared phase outcome’s payload-free `PolicyUnsatisfied` carries one closed non-Diagnostic classification:

```text
PolicyUnsatisfiedCause
├── RejectedOnly
└── IncludesExecutionFailure
```

This does not expose Attempt History, transport state, or runtime progress. It only permits typed projection:

- `RejectedOnly` → ordinary `Completed` with applicable `Unavailable` dispositions;
- `IncludesExecutionFailure` → `CandidateExecutionFailed`.

Candidate-scoped HTTP, Browser, Parse, or Selection execution failures may produce `CandidateExecutionFailed`. Broken Source identity, violated adapter/infrastructure invariants, or an untrustworthy Source execution state produce `SourceMismatch` or `SourceExecutionFailed`. Compiler failures never reach T15.

### T16 mapping

| T15 outcome | Candidate Resolution behavior |
|---|---|
| `Completed`, required fields available | Re-evaluate candidate |
| `Completed`, required field `Unavailable`/`Conflicted` | Candidate `unresolved` |
| `CandidateExecutionFailed` | Candidate `failed`; continue later candidates |
| `BudgetExhausted` | End Source Resolution with bounded Partial Completion |
| `SourceExecutionFailed` | Abort Source execution without Resolution |
| `SourceMismatch` | Abort as Source protocol/caller failure without Resolution |
| `DetailCancelled` | Existing Search Run cancellation path; release no Resolution |

No mapping inspects Diagnostic code or text.

### Ownership

- T12b owns `PolicyUnsatisfiedCause` as part of the shared phase outcome.
- T15 owns `SourceDetailOutcome`, dispositions, and phase-to-Source projection.
- T16 owns Source Detail outcome to candidate/Source-resolution mapping.

### Consequences for restructuring

- Extend D-003’s T12b result contract with the closed unsatisfied cause, not public Attempt state.
- Make T15’s typed terminal contract complete before T16.
- Preserve candidate failure continuation and Source-abort/cancellation non-release behavior in external tests.

## D-009 — Candidate Resolution owns one non-double-counting parent allowance and post-batch `remaining`

Status: **accepted**

### Decision

T16 owns one private Source Resolution parent allowance across repeated Discovery batches and Detail invocations:

```text
Source Resolution parent allowance
├── Discovery invocation child allowance
├── Detail invocation child allowance
├── Candidate-Resolution-only counters
└── monotonic parent deadline
```

Before each Discovery or Detail invocation, T16 computes the remaining applicable Resolution limits and passes them as caller tightening to the T9 child ledger. The child ledger prevents work above the remaining allowance before side effects. On return, T16 validates and commits the complete child usage report exactly once.

T16 does not separately reserve or debit child-owned requests, response bytes, pages, Browser actions, or equivalent phase work before the call. Those dimensions enter the Resolution parent only through the exact child report, preventing double charging. A child report exceeding the supplied allowance is a typed Source invariant failure, not ordinary Budget Exhaustion.

T16 directly debits only Candidate-Resolution-owned dimensions such as Discovery batches, discovered/emitted candidates, Detail candidates, and enrichment rounds. Resolution duration uses one monotonic parent deadline; each child receives only the remaining time, and elapsed duration is not double-summed.

### Budget stop and counts

On non-cancellation Budget Exhaustion:

- finalized candidates committed by earlier complete candidate operations remain usable;
- the currently started but unfinalized candidate becomes `unresolved`;
- emitted candidates not yet started become `budgetSkipped`;
- provider occurrences not yet emitted contribute only to `remaining` when exactly known.

This preserves:

```text
processed = finalized + rejected + unresolved + failed
discovered = processed + budgetSkipped
```

Cancellation remains distinct: it returns no Resolution and does not automatically release earlier finalized candidates.

### `remaining`

`remaining` is the exact number of provider occurrences not yet emitted **after** the returned batch. When consecutive exact values are available:

```text
current.remaining
  = previous.remaining - current.occurrences.len()
```

using checked arithmetic. An adapter that cannot guarantee a stable exact count—for example because the provider view changes or applies opaque filtering—returns `None`. Contradictory exact values are invalidated to `None` with one bounded sanitized Diagnostic.

### Consequences for restructuring

- T16 owns the private parent allowance; no second public mutable ledger is introduced.
- T9 remains the sole child phase-ledger/report owner.
- Remove retry dimensions under D-002 before finalizing the parent shape.
- Add exact child allowance/report, mid-candidate exhaustion, count-invariant, and `remaining` recurrence tests.

## D-010 — Every started phase terminal carries one complete usage report

Status: **accepted**

### Decision

Every terminal reached after phase work begins carries the complete indivisible Strategy Set budget report and ordered safe Diagnostics independently from domain payload:

```text
PhaseOutcome
├── Completed {
│     policyOutcome,
│     completeBudgetReport,
│     diagnostics
│   }
├── BudgetExhausted {
│     completeBudgetReport,
│     diagnostics
│   }
└── ExecutionFailed {
      typedFailure,
      completeBudgetReport,
      diagnostics
    }

PhaseCancelled {
  completeBudgetReport,
  diagnostics
}
```

Only `Completed::Accepted` contains a reduced domain payload. Reports contain usage/completion facts only and never Posting Occurrences, Detail patches, field dispositions, or other domain partial output.

Cancellation remains typed control flow outside normal phase outcomes. Its report is transient operational evidence: it is not Resolution Partial Completion, is not persisted as a Resolution, and does not release domain output. `SourceMismatch` detected before phase work starts carries no report.

T15 preserves the exact child report on `Completed`, `BudgetExhausted`, `CandidateExecutionFailed`, `SourceExecutionFailed`, and `DetailCancelled` projections. T16 validates and commits each non-cancelled child report exactly once into the Resolution parent. On Cancellation it may propagate transient usage for observability but creates no Source Resolution.

No usage or completion fact is reconstructed from Diagnostic text or attempt history.

### Consequences for restructuring

- Extend D-003 and D-008 result sketches with the common complete report placement.
- T12b owns the common phase report envelope; T15 projects without flattening.
- Add exhaustive success, policy-unsatisfied, budget, candidate failure, Source failure, and Cancellation accounting tests.

## D-011 — Compiler foundation is serial and complete before schema-v3 activation

Status: **accepted**

### Decision

The clean pre-activation compiler foundation lands in this responsibility order:

```text
final compiler interface
→ complete existing-entry merge
→ complete new Strategies/Access Paths
→ Effective Source Config Schema
→ mandatory first_accepted Policy
→ final internal phase names
→ complete Effective Profile provenance
→ canonical schema-v3 fingerprints
→ atomic schema-v3 activation hard cut
```

Mapped to current responsibility labels for restructuring input:

```text
T1 → T2 → T3a → T3b → T5 → T6 → T4a → T4b → Activation
```

These labels are navigation to current responsibilities, not approval of final ticket boundaries.

T6 precedes T4a so provenance is created once with final `detection`/`discovery`/`detail` internal paths. T4b follows complete T4a provenance and builds only the canonical schema-v3 fingerprint implementation.

Before activation, the old productive Source model continues to use its old fingerprint path; the new canonical fingerprint module is final foundation but not productively wired. The activation hard cut atomically:

1. makes complete schema-v3 Direct Source Specialization authorable/executable;
2. migrates compiler, frontend, Source validation, Source Live Check, and every productive caller;
3. activates T4b as the sole fingerprint implementation;
4. deletes the old `source_overrides` fingerprint component and old freshness path together with the complete old specialization stack.

There is no merged state with schema-v3 specialization active but canonical fingerprinting absent.

### Consequences for restructuring

- Replace the current T4a/T6 parallel assumption with T6→T4a.
- Move T4b’s final foundation responsibility before activation rather than creating fingerprints after the hard cut.
- Treat current T7 only as input to a renamed/restructured schema-v3 activation owner.
- Keep every pre-activation slice non-authorable/non-productive for Direct Source Specialization.

## D-012 — Retry boundedness is conditional on a real executable retry capability

Status: **accepted**

### Decision

D-002 explicitly refines the Strategy Algebra PRD’s broad retry wording. The invariant is:

> Every retry that actually executes must be bounded and truthfully accounted, but no authored/compiled/runtime/result retry field exists before a real executable retry capability is separately accepted and implemented.

The current #166 delivery contains no executable retry capability. Therefore initial schema-v3 plans, Strategy Set reports, Candidate Resolution limits/usage, tests, and fingerprints contain no retry dimension.

PRD user-story/Decision wording that lists retries as an unconditional current Candidate Resolution budget is updated to conditional future behavior. References in T7/T9/T10/T15/T16 that imply existing retry accounting are removed or narrowed to the conditional invariant.

A future retry capability requires its own evidence, exact execution semantics, attempt interaction, pacing/rate-limit relationship, safety ceiling, accounting owner, Diagnostics, fingerprint/version effect, and direct dependencies. It cannot enter as an always-zero field or metadata placeholder.

### Consequences for restructuring

- Remove retry fields, counters, limits, usage rows, and tests from the initial target tickets and schemas.
- Update the canonical PRD in the approved documentation/activation slice before final ticket bodies rely on it.
- Do not treat retry removal as weakening boundedness: executable retries remain prohibited until bounded behavior exists.

## D-013 — Finalized-only merge and atomic durable Search Run/Match persistence

Status: **accepted**

### Decision

Only committed `SourceResolution.finalized` values cross into cross-Source Job Posting deduplication and persistence:

```text
SourceResolution.finalized
→ cross-Source Job Posting deduplication
→ atomic SQLite persistence
```

`FinalizedCandidate` conversion is the sole productive constructor for the post-finalization merge/import payload. Direct normalized-posting construction remains test-fixture-only. Importers and constructors remain narrow enough that production callers cannot bypass Candidate Resolution.

Source-local Posting Occurrence identity remains distinct from cross-Source Job Posting deduplication. Hints never become canonical or persisted values. Cross-Source merge runs only after provider values are loaded, normalized, and finalized.

### Completion and release

On non-cancellation bounded Partial Completion, candidates finalized by earlier complete candidate operations remain usable. The currently budget-stopped candidate is `unresolved`; emitted unstarted candidates are `budgetSkipped`; neither is persisted.

Cancellation or Source execution abort releases no Source Resolution and does not automatically persist internally finalized candidates.

### Atomic transaction

One committed Search Run transaction writes:

1. `search_runs`;
2. deduplicated `job_postings`;
3. `job_posting_sources`;
4. `matches`;
5. Search Request last-run metadata.

Any failure rolls back the complete transaction. Completed and completed-with-errors runs receive Matches for finalized post-merge results. Failed and Cancelled terminal runs receive a durable run row and updated last-run metadata but no Matches and no candidate-derived Job Posting/Source rows.

No durable Candidate State, Resolution usage, Diagnostic, checkpoint, Source Resolution history, or provider payload tables are introduced. No new Source/Search Run status variant is invented.

### ADR 0008 supersession

The persistence slice atomically updates or supersedes ADR 0008’s conflicting claims that Search Runs are not historized and have no persisted Search Request relationship. ADR 0008’s durable Job Posting work-item model, manual workflow state, and unaffected posting/source dedupe/update rules remain in force unless separately changed.

### Artifacts and smoke

Raw Candidate artifacts are removed. Debug/manual smoke output is bounded to Resolution summaries, exact counts, and sanitized samples. SCHOTT smoke expectations may not promote URL-derived hints into canonical title/location values. Post-commit artifacts remain non-authoritative and cannot roll back committed SQLite state.

### Ownership and consequences

- T16 owns finalized construction and the sole pre-persistence handoff.
- T17 owns cross-Source merge, constructor/caller narrowing, schema migration, transaction, Match linkage, artifact boundary, deletion evidence, and ADR 0008 supersession.
- Persistence tests use the real migrated temporary SQLite database.
- Static call-graph evidence proves that no productive non-final constructor bypass remains.

## Decision enforcement table

This table constrains restructuring without pre-approving final ticket names or sizes.

| Decision | Canonical contract owner | Implementing responsibility | Activation | Same-slice deletion/evidence | Direct prerequisites / latest gate |
|---|---|---|---|---|---|
| D-001 | Schema-v3 activation owner | Final compiler foundations plus Source specialization cross-stack cut | Schema-v3 Source activation | Complete `sourceOverrides` backend/frontend/schema/UI/test/docs stack | D-011 chain; before #167 readiness/target DAG approval |
| D-002 | Strategy Algebra PRD + shared delivery | Every budget-owning ticket | N/A | Remove Bot-Detection claims; no hidden pacing/retry fields | Before T9/T10/T14c/T16 target bodies freeze |
| D-003 | T12b shared phase outcome | T12b result/caller migration; T13/T15 reuse | N/A | Old standalone usage/result wrappers and first-lander clauses | T9 + T12a; before T13/T15 readiness |
| D-004 | T5 Policy + T4a provenance | Policy shape then complete provenance | N/A | No implicit/temporary Policy provenance | D-011 serial order; before T4a serialization |
| D-005 | Explicit owner per Primitive family + final gate | Family migrations and implementation-free global gate | #166 convergence gate | Duplicate family behavior/dispatch/helpers/tests per slice | Variant inventory before target graph; gate before #166 completion |
| D-006 | T14b Detection reducer/state | T14a/T14b foundations; native Browser contribution completion | Combined Detection/Browser activation under D-007 | Mutable aggregate maps, duplicate proposal builders/evaluators | Detection contribution/validator interfaces before activation |
| D-007 | Shared Browser Acquisition owner | Browser foundation plus cross-phase activation | Browser/Detection cross-phase cut | `ProfileBrowserClient`, `render*`, old implementations/fakes/exports | Complete caller/deletion inventory before sizing/assignment |
| D-008 | T15 Source Detail outcome | T12b cause + T15 projection + T16 mapping | N/A | Diagnostic-derived control and incomplete terminal mappings | D-003; before T15/T16 readiness |
| D-009 | T16 Candidate Resolution | Private parent allowance and batch protocol | N/A | Resettable/duplicate parent counters and vector executor residue | T9 + T15; before T16 protocol freeze |
| D-010 | T12b common report envelope | T12b/T15/T16 exact report propagation | N/A | Flattened/reconstructed usage and report wrappers | T9; before result shapes freeze |
| D-011 | Serial compiler-foundation/activation owner | Current T1→T2→T3a→T3b→T5→T6→T4a→T4b responsibilities | Schema-v3 Source activation | Old fingerprint and specialization stack in activation | Before target DAG approval |
| D-012 | Strategy Algebra PRD owner | Canonical docs plus T7/T9/T10/T15/T16 schemas/results/tests | N/A | Initial retry fields/counters/limits/tests/fingerprint material | Before final lean bodies use canonical PRD |
| D-013 | T16 finalized handoff + T17 persistence | Finalized conversion, merge, schema/transaction/artifact/ADR cut | T17 persistence migration | Broad productive constructors, raw candidate artifacts, conflicting ADR clauses | T16; before T17 readiness/completion |

Required restructuring inputs still to produce:

1. variant-level Primitive owner/deletion inventory for D-005;
2. complete `sourceOverrides` caller/schema/UI/fingerprint/deletion inventory for D-001/D-011;
3. complete `ProfileBrowserClient` caller/deletion inventory for D-007;
4. target ticket sizing and dependency DAG using this table;
5. canonical PRD/ADR/document amendment list for D-012/D-013 and existing documentation cleanup.
