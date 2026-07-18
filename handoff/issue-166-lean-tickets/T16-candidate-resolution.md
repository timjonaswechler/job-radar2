# T16 — Resolve Source candidates in deterministic bounded batches

## Result

A Search Run resolves each compiled Source through one deterministic, Source-scoped Candidate Resolution operation. The operation consumes bounded Discovery batches, requests only missing Detail fields, evaluates normalized provider values against one Search Request, and returns only finalized candidates together with typed complete/partial completion, exact counts, cumulative usage, and bounded sanitized Diagnostics.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#219/T15](https://github.com/timjonaswechler/job-radar2/issues/219).
- Blocking: [#234/T17](https://github.com/timjonaswechler/job-radar2/issues/234).
- Readiness: **Blocked**; re-baseline this ticket against #219's landed occurrence, Detail, capability, usage, Diagnostic, and Cancellation contracts before assignment.
- Open decision: none. The immutable production Candidate Diagnostic Sample Limit is 10, and internally tagged `PartialReason` option 1 with an empty initial `BoundedSourceStopCode` is approved.

## Consumed contracts

- #166 / PRD Decisions 24, 31–32, and 41–46: Source-local occurrence identity and reduction, provider-value/hint trust, Candidate Resolution, typed completion/counts, finalized-only downstream handoff, and cross-Source deduplication after resolution.
- `handoff/issue-166-delivery.md`: shared readiness, hard-cut, seam, testing, migration, Definition-of-Done, and PR-evidence rules.
- #219 supplies field-requested, candidate-scoped Detail over the landed T12 occurrence/patch/reducer model. T16 reuses its trustworthy values, conflict quarantine, requested-field, provenance, usage, Diagnostic, and Cancellation semantics rather than wrapping or redesigning them.
- When a T13 policy is present, its compiled ordering, cardinality, stopping, budget, usage, and Diagnostic behavior remains authoritative below Source execution.
- #57 is related but not blocking: T16 retains the current `Vec<String>` Location model and existing `GeoResolver` behavior.

## Current gap

This section describes the repository while #219 remains open; exact future names and paths are provisional until readiness review.

- `src-tauri/src/search/run/execution.rs` exposes `SourceExecutor::execute(SourceExecutionInput)` and accumulates one `SourceExecutionOutput { candidates: Vec<SourceCandidate>, diagnostics }`; there is no batch continuation or candidate-scoped Detail protocol.
- `src-tauri/src/search/run/types.rs` requires complete title, company, URL, locations, and posting metadata in every `SourceCandidate`. It has no occurrence identity, provider-value/hint distinction, Candidate states, requested fields, Resolution completion/counts, or usage.
- `src-tauri/src/search/run/service/runner.rs` collects all candidates, then normalizes and applies Include, Exclusion, and Location rules before `service/merging.rs` performs cross-Source Job Posting deduplication and `search/posting/mod.rs` imports postings/Matches.
- `service/rules.rs` evaluates complete title strings only. `service/source_runs.rs` stores an independently mutable `candidate_count`, matched count, status, Diagnostics, and error, but no Resolution visibility. `service/persistence.rs` owns Search Request last-run metadata and the optional development result artifact, not posting/Match import.
- Location ambiguity reporting uses fixed `take(5)` samples, but no generic Candidate Diagnostic sampler maintains exact unsampled per-code totals.
- Deterministic executors and current Search Run coverage live in `src-tauri/src/search/run/tests/support.rs` and `tests/{source_execution,matching,failures,deduping,lifecycle}.rs`.

The missing owner is one Source-scoped operation for bounded Discovery continuation, Source-local uniqueness, Candidate transitions, minimal Detail rounds, normalization and Search Request evaluation, cumulative bounds, Cancellation, exact counts, bounded Diagnostics, and completion visibility.

## Target delta

The exact Rust future/lifetime spelling may follow landed conventions, but the caller-visible responsibilities must remain:

```rust
pub async fn resolve_source_candidates(
    request: SourceResolutionRequest<'_>,
    execution: &dyn SourceCandidateExecution,
) -> Result<SourceResolution, SourceResolutionAbort>;

pub struct SourceResolutionRequest<'a> {
    pub source: &'a CompiledSource,
    pub requirements: &'a CandidateMatchRequirements,
    pub limits: ResolutionLimits,
    pub geo_resolver: Option<&'a dyn GeoResolver>,
    pub cancellation: RuntimeExecutionContext<'a>,
}

pub trait SourceCandidateExecution: Send + Sync {
    fn next_discovery_batch<'a>(
        &'a self,
        request: DiscoveryBatchRequest<'a>,
    ) -> BoxedFuture<'a, Result<DiscoveryBatchResult, SourceExecutionAbort>>;

    fn load_detail<'a>(
        &'a self,
        request: SourceDetailRequest<'a>,
    ) -> BoxedFuture<'a, Result<SourceDetailResult, SourceExecutionAbort>>;
}

pub struct DiscoveryBatchRequest<'a> {
    pub source: &'a CompiledSource,
    pub continuation: Option<&'a DiscoveryContinuation>,
    pub max_items: NonZeroU64,
    pub cancellation: RuntimeExecutionContext<'a>,
}

pub struct DiscoveryBatchResult {
    pub occurrences: Vec<PostingOccurrence>,
    pub continuation: Option<DiscoveryContinuation>,
    pub exhausted: bool,
    pub remaining: Option<u64>,
    pub diagnostics: Diagnostics,
    pub usage: DiscoveryBudgetUsage,
}

pub enum SourceExecutionAbort {
    Cancelled(PhaseCancelled),
    BoundedStop(BoundedSourceStop),
    Failed(SourceExecutionFailure),
}

pub struct SourceResolution {
    pub completion: ResolutionCompletion,
    pub counts: ResolutionCounts,
    pub finalized: Vec<FinalizedCandidate>,
    pub diagnostics: ResolutionDiagnostics,
    pub usage: ResolutionBudgetUsage,
}

pub enum SourceResolutionAbort {
    Cancelled(ResolutionCancelled),
    ExecutionFailed(ResolutionExecutionFailed),
}
```

`CandidateMatchRequirements` contains compiled backend-owned Include/Exclusion Rules and current Search Request Location/radius requirements, never raw Profile DSL criteria. The request addresses one authoritative compiled Source and never performs a second Source lookup. The execution seam has a production implementation delegating to compiled bounded Discovery and #219 Detail, plus one deterministic scripted implementation.

### Batch protocol and execution outcomes

- The first Discovery call has no continuation; each later call uses exactly the preceding result's opaque process-local continuation. It is not serialized, persisted, resumed, reused, or reordered. Stale, foreign-Source, reused, or out-of-order tokens fail execution.
- Each batch returns deterministic provider-ordered, T12-reduced `PostingOccurrence`s, optional continuation, `exhausted`, optional exact provider `remaining`, Diagnostics, and incremental usage. The implementation emits each Source-local identity at most once; Candidate Resolution verifies that guarantee. A duplicate is a protocol failure, not another discovered Candidate or cross-Source deduplication.
- A non-exhausted batch must contain at least one occurrence and a new continuation. More than requested items, an unchanged continuation, an empty non-exhausted batch, `exhausted` with continuation, or non-exhausted without continuation aborts deterministically without a busy loop. Only `exhausted=true` proves exhaustion.
- `remaining` is accepted only as an exact provider count. Successive values cannot increase and must agree with emitted items. An inconsistency invalidates the aggregate to `None`, emits a sanitized Diagnostic, and otherwise permits processing to continue.
- Batch/Detail Diagnostics and usage are debited exactly once. Source-level Cancellation maps only to `SourceResolutionAbort::Cancelled`; protocol, arithmetic, or ordinary Source failure maps to `ExecutionFailed`. Neither returns Resolution data nor automatically releases accumulated finalized values. `ExecutionFailed` still carries sanitized terminal Diagnostics and cumulative usage as abort evidence.
- Candidate-scoped Detail execution failure terminates only that Candidate as `failed`, emits a sanitized sampled Candidate Diagnostic, and allows later Candidates to continue. The initial `BoundedSourceStopCode` is empty, so no production or deterministic path can instantiate `BoundedStop`. A first code requires a separate accepted, explicitly bounded backend contract.

### Candidate transitions and finalization

Process unique occurrences sequentially in provider order through private `pending`, `rejected`, `finalizable`, `needs_fields`, `unresolved`, `failed`, and `budget_skipped` states:

1. Only a hint explicitly authorized as `hintUse: search_prefilter` may reject early under Include/Exclusion rules. No hint can populate a final value, satisfy a required provider field, or create a Match; other hints cannot reject.
2. Centrally normalize provider title, company, and raw Location strings. Apply final Include, Exclusion, and Location rules only to normalized provider values.
3. If required values are absent, request the smallest non-empty typed Detail field set. Reuse trustworthy values; one response may satisfy several fields. Re-evaluate after every non-empty trustworthy patch.
4. A repeated request set with no new trustworthy value terminates as `unresolved`; enrichment rounds are bounded. A quarantined conflict in any required field is also `unresolved`, while non-conflicting fields remain available only for provenance/Diagnostics. Missing/unsupported values and no progress are not `failed`.
5. `FinalizedCandidate` contains exactly one normalized provider-valued title, company, absolute URL, `Vec<String>` locations, posting metadata, and Source-local `PostingOccurrenceIdentity`; it contains no hint or raw description.

### Completion, bounds, counts, and serialization

`ResolutionLimits` covers discovered items, batch size, Detail candidates, requests, bytes, retries, pages, duration, fan-out, and enrichment rounds per Candidate. Effective limits are the strictest of immutable production ceilings, caller tightening, and compiled Source/Profile tightening. Authored values above immutable ceilings are rejected rather than clamped. `max_batch_size` limits each call but alone never causes Partial Completion.

```rust
enum ResolutionCompletion { Complete, Partial { reason: PartialReason } }
enum PartialReason {
    LimitReached { dimension: ResolutionBudgetDimension },
    BoundedSourceStop { code: BoundedSourceStopCode },
}
enum BoundedSourceStopCode {} // intentionally empty initially
```

- `Complete` requires observed exhaustion and a terminal processed outcome for every emitted occurrence; unresolved or failed counts may be non-zero.
- `LimitReached` identifies the first cumulative dimension preventing the next deterministic operation. Exact-boundary work runs; one-over work never starts. Already finalized values remain usable. Emitted occurrences that cannot start are `budget_skipped`; un-emitted items appear only through exact provider `remaining`.
- The reserved bounded-stop variant has the same future finalized-usability and count semantics, but has no valid initial runtime or serialized instance. `PartialReason` is internally tagged: `{ "type": "limit_reached", "dimension": "requests" }`. Schema/Serde reject every attempted `bounded_source_stop` code; no placeholder/free-form/provider code exists.
- Every successful non-cancelled result uses checked `u64` arithmetic and satisfies `processed = finalized + rejected + unresolved + failed`, `discovered = processed + budget_skipped`, and `finalized.len() = finalized`. Overflow aborts as sanitized execution failure.
- `ResolutionBudgetUsage` exposes cumulative `discovery_batches`, `discovered_items`, `detail_candidates`, `requests`, `bytes`, `retries`, `pages`, `elapsed_ms`, `fan_out`, and `enrichment_rounds` across every consumed batch and Detail round, including failed/rejected attempts. Reservation occurs before work and one debit commits under the landed T9 contract. `remaining` is the last exact provider remainder adjusted only by later emissions; it excludes discovered `budget_skipped` items and is never estimated.
- Cancellation before/during/between batches, before/during Detail, between Candidates, or after finalization but before commit stops active and later work and returns only `Cancelled`, never Partial Completion, counts, or releasable finalized values.

### Diagnostics and Search Run visibility

`ResolutionDiagnostics` separates finite-shape terminal Diagnostics from Candidate samples and contains exact `candidate_counts_by_code`, immutable production `candidate_sample_limit = 10`, and exact omitted count. Candidate Diagnostics of every severity increment per-code totals in provider/Candidate order. Retain at most the first 10 sanitized entries and immediately discard later entries; discarded payloads/messages must not survive in another vector, error chain, trace buffer, or deferred serializer. Tests may inject a lower limit only to prove boundaries; Source/Profile data cannot author or raise it.

Codes come from a finite internal set; messages are fixed templates; details are bounded allowlisted scalars such as fields, dimensions, counts, safe ordinals, or non-reversible bounded fingerprints. No secret, credential, authorization/header/cookie value, raw request/response body, description, arbitrary provider text/markup, posting metadata, or unbounded collection may cross the execution seam into stored, logged, returned, or serialized Diagnostics. When samples are omitted, one terminal summary reports limit, sampled, and omitted counts without retaining discarded entries.

Add an optional, all-or-none `resolution_completion`/`resolution_counts` pair to each `SourceRunResult`. Skipped, pre-resolution-failed, aborted, and cancelled Source Runs expose neither. Keep existing status semantics. Delete or derive `candidate_count` from `counts.discovered`.

Derive one `SearchRunResolutionSummary` in selected-Source order: complete/partial Source counts plus checked sums of participating Resolution counts. Aggregated `remaining` is `Some(sum)` only when at least one Source participates and every participant has an exact value; otherwise `None`. Candidate samples remain on Source Runs. Only `SourceResolution.finalized` proceeds to `service/merging.rs`; posting/Match import remains downstream in `search/posting/mod.rs`.

After this change, `SearchRunService` supplies one compiled Source, compiled requirements, limits, `GeoResolver`, and Cancellation context, then consumes one Resolution or abort. It no longer knows paging, Source-local deduplication, Candidate states, Detail loops, normalization/final re-evaluation, cumulative accounting, sampling, or count arithmetic.

## Dependency and deletion decision

Candidate transitions, normalization, rule evaluation, arithmetic, sampling, and aggregation are concrete in-process logic. Compiled Sources, occurrences, patches, and continuations are immutable typed data. `SourceCandidateExecution` is the single true-external seam, backed by compiled production Discovery/Detail and a deterministic scripted implementation. Reuse the existing local-substitutable `GeoResolver` and browser adapters; add no Candidate repository, browser port, or persistence mock. SQLite posting/Match import stays downstream for T17.

**Deletion test:** Without Candidate Resolution, `runner.rs`, execution adapters, Source Run projection, and tests would each need to repeat continuation validation, batching, Source-local uniqueness, field-request rounds, final rules, cumulative bounds, Cancellation, exact counts, bounded sanitization, and Partial Completion. A module that merely forwards to the old vector `SourceExecutor` fails this test.

## Examples

1. **Complete without Detail:** one exhausted batch contains complete trustworthy provider values; final rules pass, Detail calls are zero, and counts are `discovered=processed=finalized=1` with `Complete`.
2. **Minimal enrichment/conflict:** an occurrence with title and URL requests exactly company and locations. One patch may satisfy both; a quarantined required-location conflict instead yields `unresolved=1`, `failed=0`, and later Candidates continue.
3. **Partial limit:** seven emitted occurrences yield three finalized, one rejected, one Detail failure, and two unable to start; three exact provider items remain. The result is `Partial(LimitReached(DetailCandidates))`, counts are `(discovered=7, processed=5, finalized=3, rejected=1, failed=1, budget_skipped=2, remaining=Some(3))`, and only the three finalized values continue.
4. **Cancellation after finalization:** Cancellation observed before Resolution commit returns `Cancelled`; no completion/counts are stored and no accumulated finalized value is released.
5. **Bounded samples:** 100 Candidate Diagnostics with one code retain the first 10, report omitted 90 and per-code total 100, and retain none of the discarded payloads.

## Scope

- Add `src-tauri/src/search/run/service/candidate_resolution.rs` as the Source-scoped orchestration owner, with private helper files only for substantive hidden complexity.
- Add the request/result/abort, execution/batch/continuation, final Candidate, completion/reason/count/usage, and Diagnostic-summary contracts using landed T12/T15 types.
- Implement production and deterministic `SourceCandidateExecution`; validate protocol, identity, remainder, bounds, and Cancellation.
- Move hint prefiltering, normalization, final rules, bounded Detail rounds, state/count/usage logic, and Diagnostic sampling from Search Run orchestration into Candidate Resolution.
- Add Source Run Resolution visibility and checked Search Run aggregation; move all callers/tests directly.
- Delete the old unbounded vector executor flow, forwarding adapters, duplicate fakes/counters, Candidate loop, and superseded tests. Update the active Search Run smoke documentation only for behavior changed by this slice.

## Adjacent non-goals

- T17/#234 finalized-only SQLite enforcement, durable Search Run/Match persistence, and its transaction regressions.
- Changing or relocating cross-Source Job Posting deduplication tolerance.
- Structured Location semantics or new `GeoResolver` policy (#57).
- UI-only lazy Description loading or Description persistence.
- Parallel Candidate/Strategy execution, resumable/persisted cursors, checkpoints, hidden resume, or unbounded prefetch.
- Adding the first bounded-stop code, new status variants, or treating Cancellation/ordinary failure as Partial Completion.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Complete provider values | No Detail; `Complete`; exact finalized counts | `complete_provider_values_finalize_without_detail` |
| Authorized prefilter | Authorized title hint rejects with no Detail, `rejected=1`, and no hint in final values | `authorized_prefilter_hint_can_only_reject` |
| Unauthorized hint | Cannot reject/finalize and requests provider fields | `unauthorized_hint_cannot_reject_or_finalize` |
| Minimal multi-field Detail | Exact company+locations request; one patch supplies both | `requests_minimal_fields_and_one_patch_supplies_many` |
| Reuse | Existing trustworthy title is not requested again | `reuses_provider_values_and_requests_only_missing_fields` |
| No progress | Repeated request set with no new value becomes bounded `unresolved` | `no_progress_becomes_unresolved` |
| Required conflict | Quarantined required field yields `unresolved=1`, `failed=0` | `required_detail_conflict_is_unresolved_not_failed` |
| Candidate Detail failure | `failed=1`; sanitized sampled Diagnostic; later Candidate continues | `detail_execution_failure_is_candidate_scoped` |
| Complete with failure | Proven exhaustion may be `Complete` with non-zero failed | `exhausted_source_can_complete_with_failed_candidates` |
| Duplicate identity | Execution abort; duplicate is neither counted nor finalized | `duplicate_identity_is_protocol_failure` |
| Invalid batch protocol | Empty non-exhausted, bad continuation, or oversized batch aborts without loop | `rejects_invalid_discovery_batch_protocol` |
| Known remainder | Consistent provider remainder is adjusted exactly | `preserves_consistent_provider_known_remainder` |
| Unknown remainder | Remains `None`, never estimated | `unknown_provider_remainder_stays_none` |
| Inconsistent remainder | Processing may continue; sanitized Diagnostic; final `None` | `invalid_provider_remainder_is_not_exposed_as_exact` |
| Exact limit boundary | Boundary work completes; no one-over operation | `exact_limit_boundary_is_allowed` |
| One-over each dimension | First preventing dimension yields bounded Partial and both invariants | table test `one_over_each_limit_is_partial_and_bounded` |
| Empty bounded-stop enum | Schema/Serde reject every code; no fake/runtime instance | `initial_bounded_source_stop_code_set_is_empty` plus parity check |
| Ordinary execution failure | Protocol/generic/free-form stop aborts with no Partial/counts/finalized release | `ordinary_execution_failure_is_not_partial` |
| Budget-skipped versus remaining | Emitted unstarted and un-emitted provider items stay distinct | `separates_budget_skipped_from_provider_remaining` |
| Cancellation before start | Zero execution calls; no Resolution data | `cancellation_before_start_commits_nothing` |
| Cancellation during Discovery | Active work stops; no later batch/Detail or Resolution | `cancellation_during_discovery_commits_nothing` |
| Cancellation between batches | No next batch or Resolution | `cancellation_between_batches_commits_nothing` |
| Cancellation before Detail | No Detail call or Resolution | `cancellation_before_detail_commits_nothing` |
| Cancellation during Detail | No later round/Candidate or Resolution | `cancellation_during_detail_commits_nothing` |
| Cancellation after finalization | No finalized release, Partial, or counts | `cancellation_after_finalization_releases_nothing` |
| Diagnostic 9/10/11+ | First 10 retained; exact omitted/per-code totals | Three `diagnostic_sample*` boundary tests |
| Diagnostic sanitization | Headers/body/description/provider strings are absent; shape is fixed/bounded | `diagnostics_are_sanitized_before_sampling` plus forbidden-field review |
| Discard memory bound | Storage stays 10 plus finite aggregates under a large stream | `discarded_diagnostics_do_not_accumulate` |
| Deterministic rerun | Calls, order, usage, counts, samples, and terminal Diagnostics are byte-identical | `scripted_rerun_is_byte_for_byte_deterministic` |
| Source Run visibility | Only resolved Sources have the completion/count pair; statuses unchanged | `source_execution::resolution_visibility_matches_source_outcome` |
| Search Run aggregation | Resolved-only checked sums; statuses unchanged | `source_execution::resolution_summary_aggregates_resolved_sources_only` |
| Unknown aggregate remainder | Any participating `None` produces summary `None` | `source_execution::resolution_summary_requires_all_remainders_known` |
| Aggregation overflow | Sanitized construction failure; no saturation | `source_execution::resolution_summary_rejects_count_overflow` |
| Downstream boundary | Only finalized values reach merging; SQLite remains downstream | `matching::only_finalized_candidates_reach_merging` plus call-graph review |
| Current Location model | Existing normalization/`GeoResolver`; no structured Location type | Matching regressions plus static import/type check |
| Migration/deletion | No old vector executor, forwarding fake, mutable duplicate count, or runner Candidate loop | Repository searches and public-API/call-graph review |

Candidate Resolution tests cross `resolve_source_candidates` with real in-process policy/state logic and the deterministic execution implementation. Production-adapter tests compile a real Source but use deterministic HTTP/browser adapters. Search Run tests use the same Source-scoped seam and real temporary SQLite where existing setup requires it; no live network or persistence mock is added.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test candidate_resolution
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::source_execution
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::matching
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::failures
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::deduping
cargo test --manifest-path src-tauri/Cargo.toml search::run::tests::lifecycle

rg -n '\b(SourceExecutor|SourceExecutionOutput|SourceCandidateExecution|DiscoveryBatchResult|SourceResolution|ResolutionCompletion|ResolutionCounts|FinalizedCandidate)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\b(candidate_count|candidateDiagnosticsOmitted|candidate_diagnostics_omitted|candidate_sample_limit)\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '(authorization|cookie|raw[_A-Za-z]*(body|response)|description|posting_meta|serde_json::Value)' src-tauri/src/search/run --glob '*.rs'
rg -n '\b(SourceStatus|SourceRunStatus|SearchRunStatus)\b' src-tauri/src/search/run src-tauri/tests --glob '*.rs'
rg -n 'import_search_run_result_in_transaction|update_search_request_last_run|write_search_run_result' src-tauri/src/search --glob '*.rs'
```

At readiness review, add the landed #219 Detail targets if needed. The full Rust suite follows the shared delivery contract.

## Ticket-specific migration items

- [ ] Re-baseline exact T12/T15 occurrence, reducer, Detail, field, usage, Diagnostic, and Cancellation names after #219 completes.
- [ ] Move every production Search Run caller to Source-scoped Candidate Resolution and the compiled requirements/limits input.
- [ ] Add the production and deterministic execution implementations without a discovery-only forwarding adapter.
- [ ] Derive Source Run completion/counts and Search Run summary; delete or derive legacy `candidate_count`.
- [ ] Keep only finalized complete/limit-partial values on the path to `service/merging.rs`; keep posting import in `search/posting/mod.rs` and metadata/artifact writes in `service/persistence.rs`.
- [ ] Delete `SourceExecutor::execute -> SourceExecutionOutput { candidates: Vec<_> }`, `DefaultSourceExecutor`, old vector fakes, wrappers, aliases, duplicate counters, runner Candidate loop, and superseded tests after callers move.
- [ ] Verify the production sample limit is exactly 10 and cannot be Source/Profile-authored; verify empty bounded-stop schema/Serde parity and no fabricated test code.
- [ ] Classify every hit from the focused searches, including public API, serialization, Diagnostic construction/logging, memory retention, status, and persistence call graphs.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
