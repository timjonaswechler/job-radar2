# T15 — Add field-requested candidate-scoped Detail to Source execution

## Result

For one Source-local `PostingOccurrence`, UI, Source Live Check, and future Search Run callers can request a non-empty typed set of canonical Detail fields. Source execution reuses trustworthy available values, routes only supported missing fields to one bounded candidate-scoped Detail phase invocation, lets one response provide several requested fields, and returns only requested trustworthy values with typed per-field outcomes.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#195/T12b](https://github.com/timjonaswechler/job-radar2/issues/195).
- Blocking: [#233/T16](https://github.com/timjonaswechler/job-radar2/issues/233).
- Readiness: **Blocked**; #195 is open, and #219 has no `ready-for-agent` label. Re-baseline the paths and provisional identifiers below after #195 and its transitive blockers land.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 10 and 41–42: Detail is candidate-scoped and lazy, accepts an explicit typed requested-field set, may satisfy several fields per bounded response, and participates in bounded Candidate Resolution without owning it.
- #166 / PRD Strategy Set Runtime module decision: callers use a typed Detail phase operation while the private kernel retains Policy execution, cumulative budgets, attempt history, Cancellation, provenance, and deterministic stopping.
- #195 must provide exactly four canonical Detail fields—title, company, raw provider locations, and description text—plus `PostingOccurrence`, a non-empty requested-field set, requested-only `DetailPatch`, explicit `DetailPhaseResult`, field-local conflict quarantine, usage, Structured Diagnostics, and typed Cancellation. T15 reuses these contracts without wrapping or redefining them.
- Landed compiler/runtime contracts provide an authoritative `CompiledSource`, immutable typed output plans, Strategy Policy behavior, and bounded HTTP/browser execution. T15 neither uses the Search-Run-specific `SourceExecutionSource`, copies the Execution Plan, accepts a Source key for a second lookup, nor changes the compiled Strategy list or Policy cardinality. Source-owned access remains distinct from profile-based Effective Source Profile compilation.
- `handoff/issue-166-delivery.md` owns shared readiness, hard-cut, test, migration, deletion, and PR-evidence requirements.

## Current gap

The repository still has a description-only Detail path:

- `profile_dsl/documents/posting_detail.rs` and `execution_plan/posting_detail.rs` define only `description_text: FieldExpression`; no immutable finite Strategy capability set supports request routing.
- `profile_dsl/runtime/posting_detail.rs` exposes the duplicate `PostingDetailPostingOccurrence`, `PostingDetailExecutionResult { description_text, diagnostics }`, and `execute_posting_detail*` functions. They accept no requested fields and execute fallback Strategies for description text only; `runtime/mod.rs` and `lib.rs` re-export this family.
- `search/posting/service.rs::get_posting_detail_with_clients` skips loading when a persisted description exists, but otherwise compiles by Source key, converts to the duplicate occurrence, calls the transport-shaped Detail operation, and persists a returned description.
- `checks/source_live/mod.rs` performs a similar conversion and decides Detail success by reading `description_text` directly.
- `search/run/execution.rs` remains discovery-only through `SourceExecutor`; T16, not T15, will replace that Search Run flow.
- Relevant behavior currently lives in `src-tauri/tests/posting_detail_runtime.rs`, `src-tauri/src/search/posting/tests/detail_loading/`, `src-tauri/tests/source_live_check.rs`, and the Greenhouse, Workday, and SuccessFactors profile fixtures.

Because #195 is still open, this is drafting-time evidence. Exact landed types, exports, tests, and callers must be re-inspected at readiness review; the responsibility and observable behavior below do not change.

## Target delta

Use #195's landed names where they differ, without creating a parallel field or patch model:

```rust
pub struct SourceDetailRequest<'a> {
    source: &'a CompiledSource,
    occurrence: &'a PostingOccurrence,
    required_fields: NonEmptyRequestedFields,
    cancellation: RuntimeExecutionContext<'a>,
}

pub enum SourceDetailRequestError {
    SourceMismatch {
        compiled_source_key: SourceKey,
        occurrence_source_key: SourceKey,
    },
}

pub enum RequestedFieldDisposition {
    Reused,
    Produced,
    Unsupported,
    Unavailable,
    Conflicted,
}

pub struct RequestedFieldOutcome {
    pub field: CanonicalPostingField,
    pub disposition: RequestedFieldDisposition,
}

pub struct SourceDetailResult {
    pub values: DetailPatchValues,
    pub field_outcomes: Vec<RequestedFieldOutcome>,
    pub phase_result: Option<DetailPhaseResult>,
}

pub trait SourceDetailExecution: Send + Sync {
    fn execute_detail<'a>(
        &'a self,
        request: SourceDetailRequest<'a>,
    ) -> BoxedSourceDetailFuture<'a>;
}
```

`ProfileDslSourceDetailExecution` is the production implementation. A deterministic scripted implementation supports T15 caller and seam tests; it records immutable `(Source key, occurrence identity, canonical required-field set)` snapshots, returns scripted typed results or Cancellation, and rejects unexpected calls. It does not interpret plans, execute Policies, merge patches, model persistence, or predefine T16's combined execution fake.

### Request, routing, and result invariants

1. `SourceDetailRequest` has private fields and a checked constructor. Production and deterministic implementations expose no second unchecked constructor or pre-execution path. The compiled Source key must equal the Source key in `PostingOccurrenceIdentity`. A mismatch returns only typed `SourceMismatch` before reuse, routing, phase execution, I/O, or persistence; matching URL, provider ID, profile, or Access Path cannot substitute for Source identity. Source Detail adds no serialized mismatch representation or runtime Diagnostic, while callers may translate the error into their existing source-scoped result/Diagnostic behavior at their boundary.
2. `required_fields` is a non-empty finite typed set. Duplicates and input order do not affect behavior. Outcomes contain exactly one entry per requested field in canonical order: title, company, locations, description text.
3. The compiler derives each complete Detail Strategy's immutable finite capabilities from its executable typed output expressions. Capabilities are not authored promises or dynamic strings; invalid, unsupported, or empty executable capability shapes reject compilation.
4. Before I/O, production copies requested, trustworthy, non-conflicted provider values already present on the occurrence into `values` and marks them `Reused`. Reused values retain their existing provider meaning; T15 invents no Detail provenance for them.
5. Missing requested fields absent from the union of all compiled Detail Strategy capabilities are `Unsupported`. The remaining supported missing fields become the one requested set passed to the typed Detail phase.
6. The phase receives the original complete compiled Strategy list. T15 never filters, reorders, or skips individual Strategies by field capability and never changes `first_accepted`, `all_required`, `at_least(count)`, or `collect_all(minAccepted)` semantics. Capability routing occurs only at the field boundary.
7. The complete Strategy Set is invoked at most once per Source Detail request, not once per field. One bounded response may produce several requested fields.
8. `Produced` means the requested field is present in #195's trustworthy requested-only patch. `Conflicted` means #195 quarantined it. `Unavailable` means it was supported but bounded non-cancelled phase execution produced neither a trustworthy value nor a conflict. `Unsupported` means no compiled Strategy can produce it. Callers never infer these states from Diagnostic strings.
9. `values` contains each requested trustworthy reused or produced field at most once and no unrequested field. Production never asks Detail to produce a reused field, so no reuse-versus-Detail overwrite rule exists.
10. When Detail runs, `phase_result` is the exact #195 result, including patch, provenance, conflicts, rejections, Policy/budget completion, usage, and Diagnostics. T15 adds no second phase or Diagnostics envelope. It is `None` when all requested fields are reused or unsupported.
11. Mixed support does not fail the request: unsupported fields are reported independently while the full Strategy Set runs once for supported missing fields. Policy-unsatisfied or budget-terminal phase results remain unchanged and absent supported fields are `Unavailable` unless #195 reports a conflict.
12. Provider and transport errors remain translated below this interface by landed adapters. Existing immutable Strategy, request, byte, retry, page, duration, browser, and cumulative Strategy Set limits apply. T15 adds no limit beyond the four-field enum and no parallel field/Strategy execution.
13. Cancellation is checked before reuse/routing and propagated through active work. If observed before commit, it stops later Strategies and returns `Err(PhaseCancelled)` with no `SourceDetailResult`, reused values, or phase patch. It is never a field disposition or persistable Resolution Partial Completion.

Callers know only the compiled Source, occurrence, typed required fields, Cancellation context, requested values/outcomes, and exact phase result when execution occurred. Capability indexing, available-field subtraction, aggregate support classification, result assembly, and outcome classification remain private concrete in-process logic.

## Dependency and deletion decision

- Field sets, capability intersection, reuse, classification, and patch assembly are concrete in-process logic with no trait.
- `PostingOccurrence`, `DetailPatch`, `DetailPhaseResult`, provenance, conflicts, and rejections are #195 domain data and are reused directly.
- Compiled plans and capability sets are immutable input data, not a registry or plan port.
- `SourceDetailExecution` is the one justified variation point, with `ProfileDslSourceDetailExecution` in production and a scripted deterministic test implementation.
- Existing HTTP/browser interfaces remain below the phase operation. UI description persistence remains in `JobPostingService` and uses real temporary SQLite in tests.

**Deletion test:** Removing this module would force UI lazy Detail, Source Live Check, T16's future production adapter, and their tests to repeat capability inspection, available-field subtraction, minimal request routing, reuse, field-outcome classification, phase invocation, and error/Cancellation translation. A forwarding-only wrapper fails this test.

## Examples

1. **One response, several fields:** an occurrence has only its provider URL; `[title, locations]` is requested; one capable Strategy response yields both. Both outcomes are `Produced`, both values are returned, and the exact phase result is present.
2. **Reuse plus Detail:** the occurrence already has a trustworthy title and lacks description text. Title is `Reused`; Detail receives only `[descriptionText]`; a produced description is returned without changing title provenance.
3. **Unsupported plus unavailable:** company is supported, locations is not, and bounded execution produces no company. Company is `Unavailable`, locations is `Unsupported`, values are empty, and the unchanged Policy execution remains visible in `phase_result`.
4. **Identity mismatch:** compiled `source-a` plus an occurrence from `source-b` fails construction with `SourceMismatch`; no reused value, phase call, I/O, Diagnostic, or SQLite write occurs.
5. **UI Source fallback:** Source 1 yields unavailable/conflicted/non-cancellation failure with contextual Diagnostics; Source 2 produces description text. Existing Source order is preserved and exactly one successful description update occurs. If all Sources fail, existing failed/unsupported behavior and all contextual Diagnostics remain and no description update occurs.
6. **Cancellation:** Cancellation during active Detail stops later Strategies and returns only `PhaseCancelled`; no values, phase result, persistence, or Resolution Partial Completion are released.

## Scope

- Reuse the exact #195 occurrence, field-set, patch, phase-result, capability/value-plan, usage, Diagnostic, and Cancellation contracts.
- Derive and validate one immutable finite capability set per compiled Detail Strategy from executable typed output fields.
- Add checked `SourceDetailRequest`, typed dispositions/results, the `SourceDetailExecution` seam, its production Profile DSL implementation, and its deterministic scripted implementation.
- Implement canonical reuse, supported-missing routing, full-Policy delegation, requested-only result assembly, one-response/many-field behavior, and Cancellation semantics.
- Migrate `JobPostingService` lazy description loading and Source Live Check directly while preserving caller-owned Source fallback, UI state, Check Report behavior, contextual Diagnostics, and SQLite writes.
- Move caller-visible Detail tests to the highest practical Source Detail seam while retaining narrow private tests only for hidden finite-set/intersection/classification edges.
- Delete the replaced description-era occurrence/result/public operation exports, caller conversions, transport-shaped wrappers, aliases, duplicate fakes, and superseded implementation-detail tests.
- Update canonical domain/behavior documentation to reflect typed requested fields, capabilities, reuse, and ownership boundaries.

## Adjacent non-goals

- Candidate Resolution, Search Request evaluation, normalization, batching, enrichment rounds, resolution counts/completion/sampling, or persistence-facing finalization: #233/T16.
- Finalized-only Search Run persistence: T17; UI description persistence remains caller-owned.
- Redesigning #195 identity, field equality, reducers, provenance, conflict quarantine, Diagnostics, or secret-safety behavior.
- Cross-Source Job Posting deduplication or structured Location semantics (#57).
- New canonical fields, dynamic capability registries, URL/hint/postingMeta mutation, per-field fetch loops, eager bulk Detail, parallelism, or resumability.
- Changes to Source Run/Search Run result/status types or any Resolution completion/count type.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| One response, multiple fields | One phase/provider response produces requested title and locations; both `Produced` | External Source Detail integration test with recording adapter |
| Same Source identity | Checked request constructs and execution proceeds | Constructor and public seam test |
| Mixed Source identity | Typed `SourceMismatch`; no result, reuse, phase/I/O call, Diagnostic, or persistence | Constructor test plus zero-call/zero-write assertions |
| All four fields; duplicate/order independence | Finite set is canonical; only requested trustworthy values and canonical outcomes return | Typed-set and external seam tests |
| Reuse / all reused | Existing values are `Reused`; only missing fields reach Detail; all reused means no phase call and `phase_result=None` | Recording seam tests |
| Unsupported / mixed support | All unsupported means no phase call; mixed support invokes the full Strategy Set once only for supported missing fields | External seam tests |
| Policy composition | Original Strategy list, order, thresholds, stop behavior, usage, and Diagnostics remain unchanged for every landed Policy | Full-plan call-graph review plus landed-policy integration regressions |
| Unavailable / conflict | Supported absence becomes `Unavailable`; quarantined field becomes `Conflicted`; other trustworthy fields survive | External seam tests using exact #195 phase results |
| Rejection/failure/budget | Exact landed phase completion, usage, and Diagnostics survive; outcomes remain typed | External seam and budget regression |
| Capability validation | Unsupported, dynamic, or empty executable capability shape rejects compilation | External compiler integration test |
| Cancellation before/during work | `PhaseCancelled`; no committed result or later work | External seam cancellation tests |
| Scripted implementation | Exact Source/identity/field snapshot recorded; scripted success, unavailable/conflict, and Cancellation results return; unexpected call fails | Fake contract tests |
| UI persisted description | Existing UI result; zero Source Detail calls and no new write | Real temporary SQLite service test |
| UI missing/fallback/all fail | Exact description-only requests in persisted-Source order; contextual Diagnostics preserved; one successful write or none when all fail | Table-driven real temporary SQLite service tests |
| Source Live Check | Exactly description text is requested; Check Report details/Diagnostics derive from typed outcomes/result | `source_live_check` integration test |
| Acceptance profiles | Generic behavior holds for Greenhouse, Workday, and SuccessFactors fixtures without provider-specific Rust | Existing offline profile regressions |
| Boundary/deletion | No Candidate Resolution behavior and no old description-era public family, conversion, alias, wrapper, or duplicate fake remains | Repository searches plus manual export/call-graph review |

Production integration tests cross `SourceDetailExecution` through `ProfileDslSourceDetailExecution`: the real compiler, capability routing, Strategy Policy/runtime, reducers, and result assembly execute behind that seam, while deterministic HTTP/browser adapters supply provider facts. Tests assert caller-visible requests, values, outcomes, exact phase evidence, limits, and Cancellation so private routing changes do not invalidate them.

### Focused commands

Adapt target names only to the #195-landed tree and record exact substitutions:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::posting
```

If #195 retains the current target name, also run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test posting_detail_runtime
```

## Ticket-specific migration items

- [ ] Re-baseline #195's landed fields, occurrence, phase result, complete Strategy Set budget report, capabilities, exports, callers, and tests before implementation; remove deletion targets already completed by #195 and retain only landed Source-facing wrappers/conversions that T15 actually supersedes.
- [ ] Derive finite compiled Strategy capabilities and add the checked request, production implementation, and deterministic scripted implementation.
- [ ] Migrate `JobPostingService` and Source Live Check directly; preserve description-only request behavior, ordered Source fallback, contextual Diagnostics, and SQLite write semantics.
- [ ] Delete `PostingDetailPostingOccurrence`, `PostingDetailExecutionResult`, `execute_posting_detail_with_*`, caller conversion helpers, forwarding aliases/wrappers, and duplicate fakes after equivalent public-seam coverage exists.
- [ ] Verify raw `FieldExpression`, raw authored JSON, registry lookup, persistence/UI DTOs, and transport types do not cross the Source Detail boundary.
- [ ] Confirm T15 adds no Candidate Resolution, completion/count/sampling/status behavior.
- [ ] Classify every hit and manually trace exports/callers:

```bash
rg -n '\b(SourceDetailRequest|SourceDetailRequestError|SourceDetailExecution|RequestedFieldDisposition|CanonicalPostingField|NonEmptyRequestedFields|DetailPhaseResult)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\b(PostingDetailPostingOccurrence|PostingDetailExecutionResult|execute_posting_detail_with_)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'FieldExpression|serde_json::Value' src-tauri/src/profile_dsl/runtime src-tauri/src/search/run --glob '*.rs'
rg -n 'CandidateResolution|ResolutionCompletion|ResolutionCounts|candidateDiagnosticsOmitted' src-tauri/src src-tauri/tests --glob '*.rs'
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
