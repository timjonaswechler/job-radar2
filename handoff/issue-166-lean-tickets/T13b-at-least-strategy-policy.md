# T13b — Add the `at_least(count)` Strategy Policy

## Result

A schema-v3 Discovery or Detail Strategy Set authored with `{ "type": "at_least", "count": N }` executes Strategies sequentially in compiled order, succeeds immediately when the `N`th attempt is accepted, and returns the shared payload-free `PolicyUnsatisfied` outcome immediately when the remaining Strategies can no longer reach `N`. A successful set reduces only its accepted attempts, in Strategy order, while preserving the landed cumulative budget, Cancellation, Diagnostic, provenance, and conflict-reduction contracts.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#177/T9](https://github.com/timjonaswechler/job-radar2/issues/177) and [#195/T12b](https://github.com/timjonaswechler/job-radar2/issues/195).
- Blocking: none.
- Readiness: **Blocked** by #177 and #195.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 2–10 and the “Strategy Set Runtime” module decision: policies operate on accepted attempts through one crate-private kernel; phase operations, inputs, outputs, acceptance, and reducers remain typed.
- #166 / PRD Decisions 12–22 and 27–28: the fully merged Strategy Set is validated, runtime consumes one mandatory immutable compiled policy, and Source specialization cannot weaken cumulative or immutable ceilings.
- #177/T9 supplies one cumulative ledger and exact `StrategySetBudgetReport`, debit-before-side-effect behavior, budget terminal precedence, typed Cancellation, and private Attempt control/history.
- #195/T12b supplies the typed Discovery occurrence and requested-only Detail patch envelopes, one complete T9 `StrategySetBudgetReport`, ordered contribution provenance and Diagnostics, and concrete conflict-safe Discovery and Detail reducers.
- [#202/T13a](https://github.com/timjonaswechler/job-radar2/issues/202) and [#204/T13c](https://github.com/timjonaswechler/job-radar2/issues/204) are independent siblings, not blockers. If neither sibling has landed when T13b becomes ready, T13b owns the one shared migration from T12b's report-bearing accepted phase result to the approved `Accepted`/`PolicyUnsatisfied` algebra. Otherwise it reuses the exact sibling-landed shape without implementing that sibling's policy, wrapper, conversion, or duplicate report.

## Current gap

This section is provisional while #177 and #195 remain open and must be re-baselined against their landed code before implementation.

The current tree is still pre-schema-v3 for this behavior. `profile_dsl/documents/posting_discovery.rs::PostingDiscoveryStep` and `posting_detail.rs::PostingDetailStep` contain ordered Strategies and optional phase acceptance but no authored Strategy Policy or count. Their `execution_plan/` counterparts contain no compiled policy. `compiler/boundedness.rs` limits a non-empty fallback list to `MAX_FALLBACK_STRATEGIES = 50` but performs no positive-count or final-cardinality validation.

`runtime/posting_discovery.rs::execute_posting_discovery_with_clients_and_context` and `runtime/posting_detail.rs::execute_posting_detail_with_clients_and_context` each own an implicit first-accepted loop and append `fallback_exhausted` when all attempts fail or reject. Current result types expose candidates or description-oriented Detail data plus Diagnostics; there is no shared policy kernel, exact cumulative report, `PolicyUnsatisfied` discriminator, accepted-output gate, requested Detail patch, or conflict-safe phase reducer yet.

Relevant current coverage is in `src-tauri/tests/posting_discovery_runtime/fallback_acceptance.rs`, `posting_discovery_runtime/cancellation.rs`, and `posting_detail_runtime.rs`. Production callers are Search Run (`search/run/execution.rs`), Source Live Check (`checks/source_live/mod.rs`), and lazy Detail (`search/posting/service.rs`); the generic acceptance fixtures are Greenhouse, Workday, and SuccessFactors. T13b closes only the parameterized `at_least` policy gap after the blocker-owned infrastructure lands.

## Target delta

Extend the landed strict authored and compiled closed policy representations with one parameterized variant; exact names and placement follow the landed code:

```rust
pub enum AuthoredStrategyPolicy {
    FirstAccepted,
    AtLeast { count: NonZeroUsize },
    // independently landed sibling variants
}

pub enum CompiledStrategyPolicy {
    FirstAccepted,
    AtLeast { required_accepted: NonZeroUsize },
    // independently landed sibling variants
}
```

The authored representation is exactly `{ "type": "at_least", "count": N }` beside the ordered Strategy list. `N` must be an integer greater than zero and no greater than the final merged Strategy cardinality. Schema/direct Serde reject malformed shapes: zero, negative, fractional, string, `null`, omitted `count`, unknown members, aliases, scalars, and alternate tags. The compiler emits the landed schema-v3 Structured Diagnostic at the policy/count path when a semantic final-cardinality error reaches it, and produces no partial plan. Reusable, specialized existing, Source-added, and Source-owned Strategy Sets use the same validation. Runtime neither reparses authored JSON nor infers a default.

For private `accepted` and `remaining` attempt counts, execution obeys these invariants:

1. Strategies execute one at a time in immutable compiled order. One typed accepted attempt increments `accepted` by one; transport responses, output items, fields, and Diagnostics do not.
2. After the `N`th accepted attempt, stop immediately. No later Strategy receives an attempt debit, request, browser action, parse, acceptance evaluation, or Strategy-specific reducer work; the one phase reduction required by the successful policy still follows.
3. After each completed rejected or failed attempt, return `PolicyUnsatisfied` immediately when `accepted + remaining < N`. Continue when equality still makes success reachable; preserve completed-attempt Diagnostics, exact usage, and private Attempt History.
4. Retain accepted outputs privately. On success, invoke the phase-owned T12b reducer exactly once over accepted outputs only, in Strategy order. Rejected, failed, and unattempted outputs never reach it. T12b conflict quarantine, required-provider-URL rejection, exact raw-location comparison, provenance, rejection, and Diagnostic ordering remain unchanged; policy success does not promise every field survives reduction.
5. On impossibility, expose no accepted-prefix payload, Detail patch, contribution provenance, conflict, or rejection data. Do not invoke the reducer.
6. Typed Cancellation observed before phase-envelope commitment wins over success, impossibility, and budget completion. If it becomes observable after threshold acceptance or after reduction was computed, discard the reduction and return the landed typed Cancellation terminal while preserving committed observability. Reducers remain pure and do not poll Cancellation. Emit neither an `at_least` terminal nor persistable `ResolutionCompletion::Partial`.
7. A denied T9 debit or active duration exhaustion wins over policy impossibility and uses the landed budget terminal/report with no reducer, later work, or policy terminal. If the `N`th accepted attempt completes with usage exactly equal to a limit, success remains valid; equality is not exhaustion.
8. Landed local bounds retain their attempt classifications. A rejected/failed classification participates in reachability; T13b does not reclassify it as cumulative exhaustion.
9. `at_least(count == strategies.len())` has the same acceptance truth condition as `all_required`, but remains a distinct authored variant and Diagnostic path; it does not dispatch to, alias, wrap, or depend on T13a. `first_accepted` and independently landed sibling behavior remain unchanged.

Use the blocker-landed report-bearing phase results directly. If T13b is the first T13 sibling to land, migrate them once to the approved shared algebra; otherwise reuse that exact sibling-landed shape. Both `Accepted` and `PolicyUnsatisfied` expose exactly one complete T9 `StrategySetBudgetReport` and ordered Diagnostics; ordinary success and impossibility report completion `Completed`, while actual T9 budget exhaustion remains distinct. `PolicyUnsatisfied` remains one stable shared discriminator with no phase payload, policy enum, reason, accepted/remaining/attempt counts, Strategy key, stopping outcome, Attempt record, or Attempt History. Callers discriminate through the typed result, never Diagnostic text. Preserve the landed placement of Cancellation and budget exhaustion; do not add a second result envelope, optional-payload discriminator, duplicate/flattened report, or compatibility conversion.

At earliest impossibility append exactly one Diagnostic after all completed-attempt Diagnostics:

| Field | Exact value |
|---|---|
| category | `runtime` |
| code | `strategy_policy_at_least_unsatisfied` |
| severity | `error` |
| path | `/discovery/policy` or `/detail/policy` |
| `strategy_key` | unset |
| message | `at_least policy was not satisfied` |
| details | exactly `{ "policy": "at_least", "requiredAccepted": N }` |

The details contain no `accepted`, `remaining`, current/attempted/rejected/failed/Strategy count, attempt identity, stopping key/outcome, or other runtime-progress value. The message contains no interpolated count, provider value, URL, response material, Source Config, authored JSON, secret, or Strategy identifier. Existing safe attempt Diagnostics remain unchanged and ordered. Cancellation and budget exhaustion emit neither this Diagnostic nor `PolicyUnsatisfied`; `fallback_exhausted` is absent on every `at_least` path.

Public Discovery and candidate-scoped Detail callers continue to invoke the blocker-landed typed phase operations. Accepted/remaining arithmetic, accepted-output retention, runtime-attempt provenance, Attempt History, cumulative ledger/scopes, and terminal selection remain inside the one crate-private kernel. Phase adapters continue to own typed execution, acceptance, concrete reduction, and result translation. Add no public policy executor/trait, reducer trait/callback, accepted-attempt list, mutable ledger, test-only policy hook, or erased output map.

## Dependency and deletion decision

Policy/count validation, transition arithmetic, retention, and phase reducers are in-process and are tested through the real compiler and typed phase operations. HTTP/provider endpoints are true external dependencies; browser execution is a local-substitutable external runtime. Reuse their landed production and deterministic test implementations, asserting exact call suppression after success or impossibility. Reuse the landed typed Cancellation/deadline mechanism and T9 ledger; neither receives a T13b-specific seam.

**Deletion test:** removing `at_least` handling from the one shared kernel would force both Discovery and Detail adapters to duplicate threshold progress, earliest-success/impossibility arithmetic, accepted-output retention and release, Cancellation/budget precedence, observability preservation, and reducer gating. A forwarding T13b module that can be removed without spreading this complexity fails the test and must not be introduced. The Strategy Set Runtime remains a Deepening Candidate unless implementation evidence separately satisfies the shared acceptance criteria.

## Examples

1. **Earliest success:** with `N=2`, attempts are accepted, rejected, accepted, unattempted. The first and third accepted outputs reach the reducer once in Strategy order; the fourth Strategy never starts. The recovered rejection remains in ordered Diagnostics and usage.
2. **Earliest impossibility:** with `N=3` across four Strategies, attempts are accepted, rejected, rejected. After the third completed attempt, one acceptance plus one unattempted Strategy cannot reach three. Return payload-free `PolicyUnsatisfied`, do not run the fourth Strategy or reducer, and append exact details `{ "policy":"at_least", "requiredAccepted":3 }`.
3. **Immediate impossibility/equality:** when `N` equals total cardinality, the first rejection or failure ends the set immediately. By contrast, whenever `accepted + remaining == N`, execution continues.
4. **Conflict after success:** two accepted outputs conflict on title. The policy remains accepted, while T12b quarantines the field and preserves its trustworthy output/provenance rules; no last-write-wins path appears.
5. **Budget and Cancellation:** debit denial before the threshold returns the budget terminal. Cancellation after a computed reduction but before envelope commitment discards that reduction. Neither case emits the policy Diagnostic or runs later work.
6. **Specialization:** a direct Source fragment replaces inherited `first_accepted` with `{ "type":"at_least", "count":2 }`; validation uses the final merged cardinality and runtime sees no fragment distinction.

## Scope

- Add exact authored schema/direct-Serde and mandatory compiled-plan support for `at_least(count)` across complete reusable, specialized existing, Source-added, and Source-owned Discovery/Detail Strategy Sets.
- Validate positive count against final merged Strategy cardinality and reject invalid configurations without a plan.
- Extend only the shared crate-private Strategy Set kernel with sequential counting, private accepted-output retention, earliest success, and earliest impossibility.
- Gate the exact T12b Discovery/Detail reducer once on success and reuse the shared `PolicyUnsatisfied` result/report shape on impossibility.
- Add the exact policy-terminal Diagnostic and preserve existing attempt Diagnostics, T9 usage/report, provenance, local-bound classification, Cancellation, and budget precedence.
- Update exhaustive compiler/runtime/export/caller matches directly where the closed enum/result requires it; preserve Search Run, Source Live Check, and lazy Detail without status or persistence expansion.
- Add external cross-phase policy tests and affected compiler, caller, and generic-profile regressions. Update canonical policy documentation only where needed to state the landed earliest-stop and Diagnostic-privacy behavior.
- Delete any duplicate phase count loop, raw-string policy comparison, optional/default policy handling, compatibility alias/conversion/wrapper, duplicate report/reducer path, and superseded implementation-detail test replaced by public coverage. Remove `fallback_exhausted` from `at_least` paths without changing `first_accepted` behavior.

## Adjacent non-goals

- `all_required` (#202/T13a), `collect_all` (#204/T13c), `minAccepted`, percentages, weights, exactly-one/dynamic thresholds, or authored configurable stop modes.
- Detection convergence (T14), Source execution/Candidate Resolution (T15/T16), persistence, resumability, parallel/speculative execution, or any Source/Source Run/Search Run/Check Report status expansion.
- New occurrence identity, requested fields, reducer conflict rules, provenance rules, cumulative budget dimensions/ceilings, or Cancellation semantics owned by #177/#195.
- Public generic Strategy execution, Attempt History, reducer/ledger interfaces, callback engines, plugins, or provider-/profile-specific Rust.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Exact authored policy | Strict object compiles to mandatory typed `AtLeast`; runtime uses only the plan | Schema/Serde parity and external compiler test |
| Invalid representation/count | Malformed numeric/object shapes, zero, and `count > final cardinality` reject with no plan | Schema/Serde fixtures plus compiler semantic test |
| Final merged forms | Reusable, specialized, Source-added, and Source-owned sets validate final cardinality under direct-Source authority and each execute through the same public typed phase policy/reducer path while preserving Effective Profile/Source-owned distinction | Compiler-plus-Discovery/Detail integration cases |
| Earliest Discovery/Detail success | Stop at the `N`th accepted attempt; reduce accepted outputs once in Strategy order; no later calls | Cross-phase tests with deterministic HTTP/browser adapters and exact call order |
| Recovery and equality | Rejection/failure remains observable while reachable; equality continues | Cross-phase result/report/Diagnostic assertions |
| Earliest/immediate impossibility | Stop once unreachable; no later work, reducer, or accepted-prefix payload | Exact call order and typed `PolicyUnsatisfied` assertions |
| Attempt counting | Transport-only success rejected by phase acceptance does not increment count | Public phase test |
| Result/report privacy | Accepted and unsatisfied outcomes each carry one exact complete report; unsatisfied carries no payload/progress/history | API/serialization review and external assertions |
| Diagnostic contract | Exactly one terminal with exact fields/two-member details after attempt Diagnostics; forbidden runtime state absent | Serialized value/order assertions |
| Reducer conflict/input | Only accepted outputs enter the T12b reducer; conflict behavior remains field-safe | Public payload/provenance/conflict test |
| Exact budget boundary | `N`th acceptance at the limit is Accepted with `Completed` report | Real T9 ledger boundary test |
| Budget denial | Landed budget terminal wins; no reducer, later call, or policy/fallback terminal | Real T9 debit-denial test |
| Cancellation before commitment | Typed Cancellation wins, including after computed reduction; no released payload/policy terminal/Partial Completion | Landed Cancellation mechanism with caller-visible assertion |
| Local bound | Existing rejected/failed classification participates in reachability without false budget classification | Focused phase regression |
| Sibling/regression | `first_accepted` unchanged; T13b compiles without T13a and adds no `collect_all` behavior | Existing policy tests plus compile/call-graph review |
| Production callers/profiles | Search Run, Source Live Check, lazy Detail, Greenhouse, Workday, and SuccessFactors remain generic and exhaustive | Caller/profile regressions and static review |
| Ownership/deletion | One private kernel and existing phase reducers; no duplicate/public policy machinery or raw authored runtime input | Call-graph/import/search review |

Tests cross `compile_source` followed by the public typed Discovery or Detail operation. They use real in-process policy/reducer code and deterministic implementations only for genuine HTTP/browser variation, and assert typed outcomes, exact reports/Diagnostics, payload/provenance, and exact external call order.

### Focused commands

Exact blocker-landed target names must be re-baselined at readiness review. Expected focused coverage is:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_at_least
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
```

If the current names remain after blockers land, also run `posting_discovery_runtime` and `posting_detail_runtime`. Shared full-suite/build obligations remain in the delivery contract.

## Ticket-specific migration items

- [ ] Re-baseline exact #177/#195 authored-policy, compiled-plan, kernel, ledger/report, phase-result, reducer, caller, and test paths before implementation.
- [ ] Add strict `at_least(count)` schema/Serde/compiled support and final-cardinality validation for every approved profile/Source form.
- [ ] If first among T13 siblings, introduce the one shared `Accepted`/`PolicyUnsatisfied` algebra directly on T12b's report-bearing result; otherwise reuse the exact landed sibling shape. Extend the one private kernel and migrate exhaustive compiler/runtime/export/caller matches directly.
- [ ] Delete duplicate count loops, raw-string/default/optional policy handling, aliases, wrappers, conversions, duplicate reports/reducers, and superseded tests.
- [ ] Prove no `fallback_exhausted` occurs on `at_least`, while `first_accepted` remains unchanged.
- [ ] Classify every production/test hit from:

```bash
rg -n 'at_least|AtLeast|required_accepted|requiredAccepted|PolicyUnsatisfied|strategy_policy_at_least_unsatisfied' src-tauri/src src-tauri/tests
rg -n 'fallback_exhausted|accepted.*remaining|remaining.*accepted|AttemptHistory|StrategyAttemptHistory' src-tauri/src src-tauri/tests
```

Every `accepted`/`remaining` progress hit must remain kernel-private; serialized result/Diagnostic code must match the exact approved shape.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
