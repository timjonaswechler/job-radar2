# T13c — Add the `collect_all(minAccepted)` Strategy Policy

## Result

A schema-v3 Discovery or Detail Strategy Set authored with `{ "type": "collect_all", "minAccepted": N }` executes every Strategy sequentially in compiled order unless cumulative budget exhaustion or Cancellation stops the invocation. After natural completion it returns one conflict-safely reduced `Accepted` result exactly when at least `N` Strategy attempts were accepted; otherwise it returns the shared metadata-minimal `PolicyUnsatisfied` result.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#177/T9](https://github.com/timjonaswechler/job-radar2/issues/177) and [#195/T12b](https://github.com/timjonaswechler/job-radar2/issues/195).
- Blocking: none.
- Readiness: **Blocked** by #177 and #195.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 2–5, 22–28 and the “Strategy Set Runtime” module decision: policies operate on accepted attempts in one crate-private kernel, while runtime consumes immutable typed plans.
- #177 provides the cumulative Strategy Set ledger/report, denial-before-side-effect behavior, exact-boundary semantics, typed budget exhaustion, typed Cancellation, and private Attempt History/control state.
- #195 provides `PostingOccurrence`, requested-only `DetailPatch`, public typed Discovery/Detail envelopes, and the concrete crate-private conflict-safe phase reducers with ordered Diagnostics and contribution provenance.
- T12a/#193 identity is consumed through #195: Discovery union uses provider-ID-first or normalized-URL Source-local occurrence identity and never correlates mixed identity kinds.
- T13a/#202 and T13b/#203 are independent siblings, not blockers. If neither has landed when T13c becomes ready, T13c directly introduces the already approved shared `Accepted`/`PolicyUnsatisfied` phase-result algebra; otherwise it reuses the exact landed shared shape without a wrapper or conversion.

## Current gap

This ticket is blocked, so the following paths and future names must be re-baselined at readiness review against the code landed by #177 and #195.

The current pre-blocker tree has ordered `strategies` and optional phase-level `accept_when` in `src-tauri/src/profile_dsl/documents/posting_discovery.rs` and `posting_detail.rs`; their Execution Plan counterparts carry no Strategy Policy. `compiler/boundedness.rs` limits non-empty Strategy lists to `MAX_FALLBACK_STRATEGIES = 50` but performs no policy-cardinality validation.

`runtime/posting_discovery.rs::execute_posting_discovery_with_clients_and_context` and `runtime/posting_detail.rs::execute_posting_detail_with_clients_and_context` each own an implicit first-accepted loop. They return immediately after one accepted attempt, infer part of Cancellation through Diagnostics, and emit `fallback_exhausted` after all attempts reject or fail. Discovery returns complete `PostingDiscoveryCandidate` values; Detail returns a description-only result. There is no `collect_all`, shared `PolicyUnsatisfied`, cumulative report, Source-local occurrence reducer, requested Detail patch reducer, or accepted-output collection.

Relevant coverage currently lives in `src-tauri/tests/posting_discovery_runtime/fallback_acceptance.rs`, `src-tauri/tests/posting_detail_runtime.rs`, cancellation coverage, and the Greenhouse, Workday, and SuccessFactors profile fixtures. Production callers are Search Run (`src-tauri/src/search/run/execution.rs`), Source Live Check (`src-tauri/src/checks/source_live/mod.rs`), and lazy Detail (`src-tauri/src/search/posting/service.rs`).

## Target delta

### Authored and compiled policy

The only authored representation added by this ticket is:

```json
{ "type": "collect_all", "minAccepted": 2 }
```

It appears as the mandatory policy beside the ordered Strategies. `minAccepted` is required, integer, positive, and no greater than the final merged Strategy cardinality. Omitted, zero, negative, fractional, string, `null`, scalar, externally tagged, alternate-spelling, alias, and unknown-member forms are invalid and produce no plan. Cardinality is checked after Source specialization against the complete Effective Source Profile; a direct Source fragment may replace the inherited policy, but cannot bypass final validation.

Compilation emits one closed typed `CollectAll { min_accepted: NonZeroUsize }` variant in the mandatory immutable plan policy for reusable, specialized, Source-added, and Source-owned Strategy Sets. A minimum greater than final Strategy cardinality emits a compiler Diagnostic at the final policy/minimum path and produces no plan. Runtime never reads raw authored JSON, compares policy strings, reparses the minimum, or supplies a default. Existing `first_accepted` and independently landed sibling variants remain unchanged.

### Execution and reduction

For `required = minAccepted`:

1. Execute exactly one typed attempt at a time in immutable Strategy order through the landed kernel and T9 ledger.
2. Retain each accepted typed output privately in Strategy order and increment an accepted-attempt count. A Strategy contributes at most one accepted attempt; transport successes, pages, items, occurrences, and fields do not add to the count.
3. Continue after early minimum satisfaction, ordinary rejection/failure, and even early mathematical impossibility. Rejected/failed attempt Diagnostics, exact usage, and private Attempt History remain ordered and observable through the blocker-owned contracts.
4. On budget exhaustion or Cancellation, stop later work, discard retained or computed output, and return the established typed terminal. Neither condition emits `PolicyUnsatisfied`, the collect-all policy Diagnostic, or `fallback_exhausted`.
5. Only after every Strategy completes naturally, return `PolicyUnsatisfied` without reducing when accepted count is below `required`; otherwise invoke the existing phase-owned #195 reducer exactly once over accepted outputs only, in Strategy order.
6. Observe typed Cancellation before committing the accepted envelope. Cancellation after reduction but before commitment discards the computed reduction and remains outside persistable `ResolutionCompletion::Partial`.

A required operation completing with usage exactly equal to an effective T9 limit may complete normally; equality becomes exhaustion only when additional required work is denied. Policy satisfaction depends on accepted attempts, not on how many fields or occurrences survive conflict-safe reduction. Thus a satisfied policy may return an accepted #195 envelope containing quarantined fields, rejected occurrence groups, conflicts, and provenance without fabricated replacement data.

Discovery reduction reuses exact T12a Source-local identity, first-seen ordering, required-provider-URL group rejection, and mixed-kind separation. Detail reuses requested-only patch merging. Both preserve #195 field-local quarantine, exact raw-location comparison, contribution provenance, secret safety, rejection/conflict reporting, and Diagnostic ordering; no phase-local union or last-write-wins path is added.

### Caller-visible outcome and Diagnostic

Reuse the exact shared phase-result algebra approved for T13a/T13b. `Accepted` and `PolicyUnsatisfied` both expose one complete `StrategySetBudgetReport` and ordered `StructuredDiagnostic` values; after natural completion the report's budget completion is exactly `Completed`. `PolicyUnsatisfied` contains no payload, policy/minimum, accepted/attempted/remaining counts, reason, Strategy key/index/outcome, Attempt record/history, occurrence, Detail patch, contribution provenance, conflict, or rejection.

If neither sibling policy has landed, migrate #195's usage-bearing Discovery/Detail envelopes directly to this shared algebra and replace standalone `usage` with the one complete T9 report. If either sibling has landed, reuse that exact shape. Do not add a sibling blocker, policy-specific result, optional-payload discriminator, forwarding wrapper, duplicate report, report reconstruction, or retained standalone usage. Callers discriminate by typed outcome, never Diagnostic text/code.

Natural completion below the minimum appends exactly one terminal Diagnostic after all attempt Diagnostics:

| Field | Exact value |
|---|---|
| category | `runtime` |
| code | `strategy_policy_collect_all_unsatisfied` |
| severity | `error` |
| Discovery path | `/discovery/policy` |
| Detail path | `/detail/policy` |
| `strategy_key` | unset |
| message | `collect_all policy was not satisfied` |
| details | exactly `{ "policy": "collect_all", "requiredAccepted": N }` |

The details contain no runtime progress, Attempt data, Strategy count/key/outcome, provider values, URLs, response data, Source Config, arbitrary paths, or secrets. `requiredAccepted` comes from the immutable compiled policy.

Accepted count, retained outputs, current/remaining attempt, Attempt History, and stopping state remain crate-private. Public callers continue to use the typed Discovery and candidate-scoped Detail operations landed by #195; they do not execute Strategies, count acceptances, select reducers, merge outputs, or inspect Attempt History.

## Dependency and deletion decision

The compiled policy, kernel transitions, accepted-output collection, T9 ledger/report, T12a identity, and T12b reducers are in-process typed data/computation. HTTP/browser execution reuses the landed production implementations and deterministic test adapters; no new external seam is introduced. Cancellation reuses the landed runtime facility.

**Deletion test:** Removing `collect_all` from the one shared kernel would force both Discovery and Detail adapters to duplicate execute-all sequencing, acceptance counting, output retention, budget/Cancellation no-output precedence, natural-completion gating, policy-terminal translation, and reducer invocation. Discovery would additionally risk duplicating Source-local union/order behavior. A forwarding or enum-conversion module fails this test and must be omitted.

## Examples

1. **Early minimum:** with `minAccepted=2`, attempts accept, accept, reject, accept. All four run; the reducer receives attempts one, two, and four in Strategy order.
2. **Natural dissatisfaction:** with `minAccepted=3`, two attempts accept and two reject/fail. Every Strategy runs; no accepted prefix is reduced or exposed; `PolicyUnsatisfied` carries the complete T9 report and exact terminal Diagnostic.
3. **Discovery conflict:** accepted attempts identify one provider-ID occurrence with conflicting required URLs. The policy remains satisfied, but the #195 reducer rejects that occurrence group; no first/last URL wins.
4. **Detail conflict:** accepted patches contain complementary requested fields and conflicting title values. The reducer quarantines title, retains trustworthy requested fields, and reports exact conflicts/provenance without changing policy satisfaction.
5. **Budget/Cancellation:** after the minimum is reached, a later required debit is denied—or Cancellation wins after reduction but before commit. The established terminal wins, all collection output is discarded, and no collect-all policy terminal or later work occurs.

## Scope

- Add strict schema/direct-Serde and compiled typed support for `collect_all(minAccepted)` across complete Discovery/Detail Strategy Sets and direct Source fragments.
- Validate the explicit minimum against final merged Strategy cardinality and reject invalid configuration without a partial plan.
- Extend only the blocker-landed crate-private Strategy Set kernel with sequential execute-all collection and natural-completion evaluation.
- Reuse the exact T9 ledger/report, typed terminals, T12a identity, and T12b reducers; update exhaustive compiler/runtime/export matches.
- Resolve shared phase-result ownership from the landed sibling state, then migrate Search Run, Source Live Check, lazy Detail, and affected tests directly when the shared result shape changes.
- Add external compiler-plus-cross-phase policy coverage, deterministic exact-call-order tests, and affected generic profile/caller regressions.
- Delete superseded phase-local loops, raw policy comparisons/defaults, duplicate collection/reduction/report paths, compatibility aliases/wrappers, and `fallback_exhausted` behavior on `collect_all` paths.
- Update the active canonical policy documentation only where needed for the required minimum, execute-all behavior, terminal precedence, and Diagnostic privacy.

## Adjacent non-goals

- `all_required`, `at_least`, optional/default minimum, percentages, weights, or another policy; T13a/#202 and T13b/#203 own their policies.
- Earliest-success or earliest-impossibility stopping; `collect_all` always attempts all eligible Strategies.
- New T9 budget dimensions, T12a identity/correlation, T12b conflict/provenance semantics, or public generic policy/reducer/ledger/Attempt APIs.
- Detection Strategy Set convergence (T14), Candidate Resolution, cross-Source deduplication, persistence, Partial Completion, resumability, or status changes.
- Parallel/speculative execution, Strategy placement/reordering/deletion/disabling, provider-specific Rust, or network-dependent default-CI tests.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Valid policy and profile/Source forms | Exact object compiles to mandatory typed policy for reusable, specialized, Source-added, and Source-owned forms | External `strategy_policy_collect_all` cases through `compile_source` |
| Invalid representation/cardinality | Invalid syntax rejects with no plan; `minAccepted > final strategies.len()` emits a compiler Diagnostic at the final policy/minimum path and no plan; merged cardinality is authoritative | Schema, semantic-compiler, and direct-fragment cases with exact Diagnostic assertion |
| Discovery/Detail execute all | Every eligible Strategy debits/executes in plan order; accepted outputs reduce once in Strategy order | Public cross-phase tests with exact adapter call order |
| Early minimum or impossibility | Neither condition stops later eligible Strategies | Cross-phase call-order table |
| Recovered rejection/failure | Attempt Diagnostics/report remain ordered; sufficient final acceptances return `Accepted` | Cross-phase result/report assertions |
| Natural dissatisfaction | No reducer/payload; shared `PolicyUnsatisfied`, complete report, exact terminal Diagnostic, no `fallback_exhausted` | Discovery and Detail serialized-result assertions |
| Acceptance counting | Transport-only success does not count; each Strategy counts at most once | Deterministic phase acceptance cases |
| Discovery identity/conflicts | Equal identities union once in first-seen order; mixed kinds remain separate; required-URL conflict rejects group | Public Discovery reducer cases, including reversed contribution order |
| Detail conflicts | Requested-only fields merge; conflicts quarantine without last-write-wins | Public Detail reducer cases |
| Budget before/after minimum | Established budget terminal, exact report, no output/reducer/policy Diagnostic/later work | Real T9 controls plus budget regression |
| Exact boundary | Final work at the limit reaches natural policy evaluation | Cross-phase and T9 boundary cases |
| Cancellation | Typed Cancellation before/during work or before commit discards output and starts no later work | Cross-phase Cancellation plus Search Run regression |
| Result/report privacy | Both natural outcomes expose one complete report; no standalone usage, wrapper, progress, or public Attempt state | Serialization/Public-API/call-graph inventory |
| Sibling independence | T13c compiles without requiring T13a/T13b and reuses any already-landed shared algebra directly | Landed-commit check plus module/call-graph review |
| `first_accepted` and production callers | Existing fallback behavior remains unchanged; Search Run, Source Live Check, and lazy Detail handle typed outcomes exhaustively | Focused policy/runtime and affected caller regressions |
| Generic profiles | Greenhouse, Workday, and SuccessFactors retain provider-neutral behavior | Existing deterministic profile targets |
| Ownership/deletion | One private kernel and existing reducers own behavior; no duplicate loop/union/report or compatibility path remains | Ticket-specific searches and manual API/serialization/call-graph review |

### Focused commands

Inspect blocker-landed target names first and substitute exact landed names where necessary:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_collect_all
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_budget
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
rg -n 'collect_all|CollectAll|minAccepted|min_accepted|requiredAccepted|PolicyUnsatisfied|strategy_policy_collect_all_unsatisfied' <exact-landed-production-and-test-files>
rg -n '\busage\b|StrategySetBudgetReport|fallback_exhausted|AttemptHistory|reduce_discovery|reduce_detail' <exact-landed-kernel-result-reducer-and-caller-files>
```

## Ticket-specific migration items

- [ ] Re-baseline #177/#195 types, paths, tests, terminals, reducers, callers, and all transitive landed contracts before implementation.
- [ ] Add the strict authored/compiled variant and final-cardinality validation for every approved profile/Source form.
- [ ] Extend only the shared private kernel; classify every accepted-count and retained-output hit as kernel-private.
- [ ] Record whether T13a or T13b has landed; directly migrate #195 envelopes only when neither has, otherwise reuse the exact shared landed result.
- [ ] Migrate every affected Search Run, Source Live Check, lazy Detail, export, and test match directly to the final typed outcome.
- [ ] Delete duplicate phase collection/count loops, duplicate occurrence unions/reducers/reports, raw-string policy dispatch, defaults, aliases, wrappers, conversions, and superseded tests.
- [ ] Verify every occurrence union resolves to the one T12b reducer using T12a identity and that no standalone `usage` remains.
- [ ] Verify `fallback_exhausted` is unreachable on `collect_all`, while `first_accepted` retains its established behavior.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
