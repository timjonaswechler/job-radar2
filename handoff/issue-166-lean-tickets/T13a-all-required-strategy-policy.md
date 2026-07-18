# T13a — Add the `all_required` Strategy Policy

## Result

A schema-v3 Discovery or Detail Strategy Set authored with `{ "type": "all_required" }` executes Strategies sequentially in compiled order and returns an accepted phase payload only after every Strategy is accepted. The first rejected or failed required attempt stops the set without reducing or exposing an accepted prefix; callers receive typed `PolicyUnsatisfied` with the complete cumulative budget report and ordered Diagnostics.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#177/T9](https://github.com/timjonaswechler/job-radar2/issues/177) and [#195/T12b](https://github.com/timjonaswechler/job-radar2/issues/195).
- Blocking: [#205/T14a](https://github.com/timjonaswechler/job-radar2/issues/205).
- Readiness: **Blocked** by #177 and #195. Re-baseline all provisional paths and type names against their landed code before implementation.
- Open decision: none. Fail-fast execution and the metadata-minimal caller-visible `PolicyUnsatisfied` outcome are selected requirements.

## Consumed contracts

- #166 / PRD Implementation Decisions 2–5, 22–28 and the “Strategy Set Runtime” module decision: policies operate on accepted attempts; runtime consumes an immutable typed plan; one crate-private kernel owns policy transitions, budgets, Cancellation, Attempt History, and deterministic stopping while phase adapters own typed acceptance and reducers.
- #166 / PRD Implementation Decisions 12–21, 35 and 37–38: the strict schema-v3 policy object is mandatory on complete Strategy Sets, direct Source fragments may specialize it through the same typed vocabulary, and Source-owned Access Paths use the same contract without compatibility syntax or runtime defaults.
- #177/T9 provides one cumulative ledger and one exact `StrategySetBudgetReport` per typed Discovery or Detail invocation, denial before side effects, exact attempted-work usage, immutable ceilings, typed budget completion, and Cancellation/budget precedence.
- #195/T12b provides typed Discovery occurrence and requested-only Detail patch envelopes, one complete T9 `StrategySetBudgetReport`, bounded `ContributionOrigin`, conflicts/rejections, ordered Structured Diagnostics, and one deterministic crate-private reducer per phase. Attempt History and runtime-attempt provenance remain private and distinct from contribution provenance.
- T13a/#202, T13b/#203, and T13c/#204 are independent siblings. If no sibling has landed when T13a becomes ready, T13a owns the one shared migration from T12b's report-bearing accepted phase result to the approved `Accepted`/`PolicyUnsatisfied` algebra. If a sibling already landed it, T13a reuses that exact shape without wrapper, conversion, or duplicate report.
- Shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules follow `handoff/issue-166-delivery.md`.

## Current gap

This section is provisional while #177 and #195 remain open. In the current tree:

- `profile_dsl/documents/posting_discovery.rs` and `posting_detail.rs` contain ordered `strategies` and optional phase-level `accept_when`, but no authored Strategy Policy;
- the corresponding `execution_plan/posting_discovery.rs` and `posting_detail.rs` types mirror that shape without a compiled policy, cumulative limits, or phase reducer contract;
- `compiler/boundedness.rs` enforces a non-empty list and `MAX_FALLBACK_STRATEGIES = 50`, but has no policy-specific validation;
- `runtime/posting_discovery.rs::execute_posting_discovery_with_clients_and_context` and `runtime/posting_detail.rs::execute_posting_detail_with_clients_and_context` each implement their own implicit first-accepted loop, continue after rejected/failed attempts, and append `fallback_exhausted` only when all attempts fail;
- Cancellation is partly inferred through `runtime/cancellation.rs::contains_runtime_execution_cancelled`, and Discovery currently returns `PostingDiscoveryExecutionResult { candidates, diagnostics }` while Detail returns a description-only `PostingDetailExecutionResult` using `PostingDetailPostingOccurrence`;
- `tests/posting_discovery_runtime/fallback_acceptance.rs`, `tests/posting_discovery_runtime/cancellation.rs`, and `tests/posting_detail_runtime.rs` cover existing fallback order, recovery, exhaustion, and Cancellation. No test authors, compiles, or executes `all_required`.

The production callers are Search Run (`search/run/execution.rs`), Source Live Check (`checks/source_live/mod.rs`), and lazy Detail (`search/posting/service.rs`). The gap after the blockers land is one additional closed authored/compiled policy variant, its transition arm in the shared kernel, reducer gating, and its typed public terminal—not a new kernel, ledger, phase envelope, reducer, or posting model.

## Target delta

### Authored and compiled policy

Extend the blocker-landed closed authored and compiled policy representations with one non-parameterized `AllRequired` variant. The exact serialized authored value is:

```json
{
  "policy": { "type": "all_required" },
  "strategies": [
    { "key": "catalog", "...": "..." },
    { "key": "supplement", "...": "..." }
  ]
}
```

A scalar, external tag, `null`, unknown member, alternate spelling, alias, count, threshold, reducer choice, mode, or `continueOnFailure` member is invalid. Complete reusable-profile, Source-added, and Source-owned Strategy Sets may author it. A typed direct Source fragment may inherit the base policy or replace it with `all_required` through the landed recursive specialization rules.

The compiler emits one mandatory typed immutable plan policy and no partial plan for an invalid, empty, over-limit, incomplete, or otherwise non-executable set. Every listed Strategy is required. Policy specialization cannot widen compiled or backend limits. Runtime never reads authored JSON, compares raw policy strings, or supplies a missing-policy default. Existing `first_accepted` behavior remains unchanged.

### Execution and reduction

For each Strategy in immutable compiled order, the shared kernel executes exactly one typed attempt through the cumulative ledger:

1. Typed Cancellation returns the established Cancellation terminal immediately.
2. Cumulative debit denial or active duration exhaustion returns the established budget terminal immediately.
3. A rejected or ordinary failed attempt returns `PolicyUnsatisfied` immediately. No later debit, request, browser action, parse, extraction, acceptance evaluation, or reducer work occurs.
4. An accepted output is retained privately until universal acceptance is known.
5. Only after every Strategy is accepted does the phase adapter invoke its T12b reducer exactly once over all accepted outputs in Strategy order and return `Accepted`.

A successful transport is insufficient unless the attempt reaches the landed accepted state. Previously accepted prefix output is neither reduced nor exposed after rejection, failure, budget exhaustion, or Cancellation. Completed/started attempt Diagnostics, exact cumulative usage, and private runtime-attempt provenance/history remain in established order. An unsuccessful set fabricates no `ContributionOrigin`; successful contribution provenance remains owned by the phase reducer.

Reducer behavior is unchanged: equal/complementary values, field-local quarantine, required-provider-URL rejection, exact raw-location comparison, conflicts/rejections, and retained-responsibility provenance follow T12b. Reducer conflicts do not retroactively make accepted attempts policy failures, and `all_required` never performs last-write-wins or ad hoc concatenation. If universal acceptance reduces to no retained value for a conflicted responsibility, callers receive the landed typed envelope rather than invented data. Any post-reduction phase validity rule remains phase-owned.

Cancellation and cumulative budget exhaustion take precedence over ordinary policy dissatisfaction and emit neither the policy terminal nor `fallback_exhausted`. A local operation/Strategy bound retains its landed attempt classification; when that classification is rejected or failed, normal fail-fast behavior applies. A final accepted attempt may succeed with usage exactly equal to an effective limit because exhaustion begins only when required work is denied.

### Caller-visible outcome

Extend the existing accepted-versus-terminal decision point in both blocker-landed phase result algebras directly. This ticket performs that shared migration only when it is the first T13 sibling to land; otherwise it reuses the exact sibling-landed algebra:

```rust
PolicyUnsatisfied {
    budget_report: StrategySetBudgetReport,
    diagnostics: Vec<StructuredDiagnostic>,
}
```

Adapt field placement only if the landed common envelope already exposes the same report and Diagnostics unconditionally. Do not add a second outer wrapper, optional-payload discriminator, standalone `usage`, flattened/reconstructed report, or T13a-specific report type.

Both `Accepted` and `PolicyUnsatisfied` expose exactly one complete T9 `StrategySetBudgetReport`—typed completion plus exact cumulative usage—and ordered Diagnostics. Ordinary accepted and policy-unsatisfied runs have budget completion `Completed`; actual budget exhaustion retains T9's distinct typed outcome/report semantics. `PolicyUnsatisfied` contains no Discovery occurrence, Detail patch, contribution provenance, conflicts, rejections, policy field, reason, attempt index/key/outcome, Attempt record, or Attempt History. Existing safe attempt Diagnostics may retain their landed path, message, and `strategy_key`; T13a adds no new derived stopping metadata. Callers discriminate acceptance only by the typed result variant, never Diagnostic contents.

For ordinary policy dissatisfaction, append exactly one terminal Structured Diagnostic after the stopping attempt's Diagnostics:

| Field | Exact value |
|---|---|
| category | `runtime` |
| code | `strategy_policy_all_required_unsatisfied` |
| severity | `error` |
| Discovery path | `/discovery/policy` |
| Detail path | `/detail/policy` |
| `strategy_key` | unset |
| message | `all_required policy was not satisfied` |
| details | exactly `{ "policy": "all_required" }` |

The terminal Diagnostic contains no stopping reason or identifier, provider value, URL, response material, Source Config, authored JSON, arbitrary path, or secret. `fallback_exhausted` is absent. If Cancellation is represented as `Result<_, PhaseCancelled>` or budget completion sits elsewhere in the landed algebra, preserve those established placements.

The policy transition arm, accepted-prefix storage, Attempt History, runtime-attempt provenance, ledger, and terminal selection remain crate-private. Discovery and Detail keep typed one-attempt execution, acceptance, reducer, and Diagnostic translation. No public generic executor, policy/reducer trait, callback, accepted-attempt list, mutable ledger, erased value map, or test-only public hook is added.

## Dependency and deletion decision

Authored/compiled policies, kernel state, reducers, reports, phase envelopes, and Diagnostics are in-process typed data or computation. Reuse the one T9 ledger/report and the two concrete T12b phase reducers. HTTP remains the existing true-external seam and browser execution the existing local-substitutable runtime seam; their landed production and deterministic test implementations prove that fail-fast suppresses later external work. T13a introduces no new seam.

**Deletion test:** removing the `all_required` arm from the shared Strategy Set kernel would force both Discovery and Detail adapters to duplicate universal-acceptance tracking, fail-fast stopping, accepted-prefix withholding, Cancellation/budget precedence, observability preservation, and reducer gating. A separate forwarding module or enum-conversion layer whose removal does not spread that complexity must not be added.

## Examples

1. **Universal acceptance:** `catalog` and `supplement` accept. Their outputs reach the phase reducer once in that order. `Accepted` contains the typed reduced envelope, ordered Diagnostics, and one complete T9 report with `Completed` and both attempts' exact usage.
2. **Accepted prefix then failure:** attempt 0 accepts, attempt 1 fails, and attempt 2 is never debited or invoked. The prefix is not reduced or exposed. `PolicyUnsatisfied` contains only the complete `Completed` budget report and Diagnostics ending with the generic policy Diagnostic.
3. **Reducer conflict:** two accepted title contributions conflict. Policy acceptance succeeds; T12b quarantines title and returns its typed conflict data and any trustworthy retained fields without last-write-wins.
4. **Budget boundary and denial:** a final accepted attempt at the exact limit returns `Accepted`; denial of required work after an accepted prefix returns only the established budget outcome and performs no reduction or policy-terminal emission.
5. **Cancellation:** Cancellation during a required attempt stops later work and reduction, preserves committed usage/Diagnostics through the established path, and never becomes `PolicyUnsatisfied` or persistable Resolution Partial Completion.
6. **Specialization:** a direct Source fragment replaces inherited `{ "type": "first_accepted" }` with `{ "type": "all_required" }`; the Effective Source Profile compiles one typed policy and runtime cannot observe the fragment origin.

## Scope

- Add strict schema/Serde support for `{ "type": "all_required" }` to complete Discovery/Detail Strategy Sets and typed direct Source fragments.
- Add and compile the matching mandatory policy variant for reusable-profile, inherited/specialized, Source-added, and Source-owned cases while preserving all existing validation and immutable ceilings.
- Extend only the shared crate-private Strategy Set kernel with sequential fail-fast universal-acceptance transitions and private accepted-prefix retention.
- Gate each landed Discovery/Detail reducer once after universal acceptance and preserve its typed payload, provenance, conflict/rejection, and Diagnostic contracts.
- Add `PolicyUnsatisfied` and the exact terminal Diagnostic to the existing phase result/Diagnostic locations; update exhaustive production caller matches and exports directly.
- Migrate Search Run, Source Live Check/activation, lazy Detail, deterministic adapters, fixtures, and tests wherever the closed policy/result enums require exhaustive handling, without status or persistence expansion.
- Delete duplicate policy dispatch, raw-string/default/compatibility branches, wrappers, aliases, duplicate reports/reducers, and superseded implementation-detail tests after equivalent public compiler-plus-phase coverage exists.
- Update the canonical policy documentation only as needed to state the selected fail-fast behavior and remove ambiguity.

## Adjacent non-goals

- `at_least(count)` (#203/T13b) or `collect_all(minAccepted)` (#204/T13c), including their cardinality, early-success/impossibility, execute-all, or occurrence-union semantics.
- Detection Strategy Set migration (#205/T14a) or Detection contribution reduction; Detection does not execute `all_required` in this slice.
- Candidate Resolution, Source-scoped execution, Detail capability/routing, persistence, counts, statuses, Partial Completion, or resumability.
- Execute-all-after-failure, configurable stop behavior, parallel/speculative execution, Strategy placement/reordering, deletion/disabling, or `null` semantics.
- New budget dimensions/ceilings, attempt classifications, occurrence identity, requested-field algebra, conflict policy, or provenance representations owned by the blockers.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Exact authored policy | Strict object is accepted and compiles to mandatory `AllRequired` | Schema/Serde parity and compiler integration test |
| Invalid shape or set | Scalar/null/parameter/unknown member/alias and empty, over-limit, or incomplete set reject with no plan | Schema/Serde/compiler tests |
| Profile/Source forms | Reusable, specialized, Source-added, and Source-owned sets compile under direct-Source authority and each execute through the same public typed phase transition/reducer path while preserving Effective Profile/Source-owned distinction | Compiler-plus-Discovery/Detail integration cases |
| Discovery/Detail success | All attempts accept; ordered calls; reducer runs once; `Accepted` exposes reduced payload, one complete report, ordered Diagnostics | Public cross-phase policy test with deterministic adapters |
| Rejection or failure | First such attempt stops later work and reducer; no prefix/payload escapes; report is complete with `Completed` | Cross-phase fail-fast call/debit/result assertions |
| Transport-only success | Acceptance rejects; result is `PolicyUnsatisfied`, not `Accepted` | Deterministic phase test |
| Policy Diagnostic | Exactly one specified terminal follows attempt Diagnostics; no `fallback_exhausted` or stopping metadata | Exact value/order assertions and serialization review |
| Typed discrimination/privacy | Diagnostic injection cannot change acceptance; unsuccessful result has no payload/history/reason/index/key/outcome | Exhaustive caller test plus public API/serialization review |
| Report parity | Accepted and policy-unsatisfied outcomes each expose the same single complete report shape and exact executed-work usage | Cross-phase known-usage assertions |
| Conflict-safe reduction | Equal/complementary/conflicting fields and required URL/location rules remain T12b behavior | Public phase reducer regressions |
| Reducer gating | Accepted prefix followed by failure invokes no reducer | Observable reducer-output/call assertions through public phase seam |
| Exact boundary/budget denial | Equality succeeds; denied work uses only T9's outcome and suppresses later work/policy terminal | T9 integration regressions |
| Local bound | Its landed rejected/failed classification leads to `PolicyUnsatisfied`, not false cumulative exhaustion | Public phase bound regression |
| Cancellation | Existing typed Cancellation wins with committed usage/Diagnostic order and no later work/output | Deterministic Cancellation tests |
| `first_accepted` regression | Existing reject/fail-then-accept order and output are unchanged | Existing public policy/runtime and profile regressions |
| Production callers | Search Run, Source Live Check, and lazy Detail handle typed outcomes exhaustively without status/persistence changes | Caller tests plus call-graph review |
| Ownership/deletion | One private policy loop, one report, phase-owned reducers; no wrapper/raw policy/public Attempt state | Added-line search plus manual API/serialization/call-graph review |
| Sibling scope | T13a-added lines contain no `at_least`, `collect_all`, configurable mode, parallelism, or Detection execution | Added-line diff search; independently landed sibling code is allowed |

Tests compile complete Sources and call the public typed Discovery/Detail operations with the real compiler, kernel, ledger, acceptance, and reducers plus deterministic existing HTTP/browser implementations. Re-baseline target names after #177/#195 land.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_all_required
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_budget
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
```

If the landed tree retains the current target names, also run `posting_discovery_runtime` and `posting_detail_runtime`. Shared full-suite/build requirements follow the delivery contract.

Run and classify the ticket-specific ownership/deletion inventory:

```bash
rg -n 'all_required|AllRequired|PolicyUnsatisfied|strategy_policy_all_required_unsatisfied|StrategySetBudgetReport|fallback_exhausted' src-tauri/src src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'raw.*policy|policy.*default|compat.*policy|optional.*payload|standalone.*usage|AttemptHistory|StrategyAttemptHistory|accepted.*prefix' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\bpub\s+(trait|struct|enum|type|fn)\s+[A-Za-z0-9_]*(Policy|Reducer|Attempt|StrategySet)' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs --glob '*.rs'
```

Every hit must be classified as the one authored/compiled variant, the one private kernel/result owner, a concrete phase reducer/caller, an exact test fixture/assertion, or residue to delete. Independently landed sibling variants are allowed; T13a-added code must not implement them.

## Ticket-specific migration items

- [ ] Re-baseline the exact landed authored/fragment/compiled policy, kernel, phase result, report, reducer, Diagnostic, export, caller, and test paths after #177/#195 complete.
- [ ] Add the strict authored and mandatory compiled variant in every supported reusable/direct/Source-owned form; reject every parameter/default/alias shape.
- [ ] Extend only the shared kernel; prove no later debit/call/reducer and no accepted-prefix escape after rejection/failure.
- [ ] If first among the T13 siblings, introduce the one shared `Accepted`/`PolicyUnsatisfied` algebra directly on T12b's report-bearing result; otherwise reuse the exact landed sibling shape. Add the same exact complete report to both outcomes without a wrapper, duplicate `usage`, flattening, narrowing, or reconstruction.
- [ ] Add the exact policy-terminal Diagnostic and preserve existing attempt Diagnostics byte/value/order-wise; add no stopping metadata and no `fallback_exhausted` on `all_required` paths.
- [ ] Move exhaustive Search Run, Source Live Check, and lazy Detail matches directly to the landed result algebra without changing persistence or status models.
- [ ] Delete any duplicate policy loop/dispatch, compatibility/default/raw-string branch, forwarding conversion, optional-payload discriminator, duplicate report/reducer, and superseded implementation test introduced or replaced by this slice.
- [ ] Classify every production/test hit for policy ownership, `PolicyUnsatisfied`, report fields, `fallback_exhausted`, raw `all_required` comparisons, and sibling policies; manually distinguish executable code from comments/fixtures.
- [ ] Review the public API, serialization shape, and call graph to confirm private Attempt ownership, one policy loop, reducer gating, typed discrimination, and no new public generic executor/trait/ledger/history.
- [ ] Record caller knowledge, hidden complexity, error/test surface, deletion-test evidence, and whether the Strategy Set Runtime remains a Deepening Candidate or now satisfies the accepted deep-module criteria; do not claim acceptance without that evidence.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
