# T8 — Route Discovery and Detail through one typed Strategy Set runtime

## Result

Discovery and Detail execute their mandatory compiled `FirstAccepted` Strategy Sets through one crate-private, closed, typed orchestration kernel. Their public operations remain separate and phase-typed, while duplicated fallback loops and Diagnostic-code-based Cancellation control flow are deleted without changing caller-visible outputs, ordering, bounds, Diagnostics, or Search Run Cancellation behavior.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#174](https://github.com/timjonaswechler/job-radar2/issues/174) — T7, schema-v3 authored hard cut.
- Blocking: [#177](https://github.com/timjonaswechler/job-radar2/issues/177) — T9, cumulative Strategy Set budgets.
- Readiness: **Blocked**. #174 is open, and #176 has no `ready-for-agent` label. Re-baseline all provisional paths, symbols, bounds, tests, and callers against the landed #174 tree before implementation.
- Open decision: none. Stop for review rather than widening T8 if the landed design would require a public generic executor, erase typed phase contracts, or pull cumulative accounting or Detection convergence forward.

## Consumed contracts

- #166 / PRD Decisions 2–10, 22, 26–28, and the “Strategy Set Runtime” module decision: mandatory typed phase policies, acceptance-driven execution, ordered attempts, immutable plans, typed Cancellation, and phase-owned reducers.
- #174 must provide schema-v3 `discovery`/`detail` vocabulary, mandatory authored and compiled `FirstAccepted`, final typed phase operations, and removal of schema-v2 compatibility paths.
- The compiler remains `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)` with an authoritative direct Source and immutable typed Execution Plan output. Runtime does not inspect authored documents.
- T9/#177 owns cumulative Strategy Set budgets, parent/child ledgers, aggregate usage, effective cumulative ceilings, and new cumulative terminal semantics. T8 preserves only bounds already observable after #174.
- T4a compiler merge provenance and T4b fingerprint material are not runtime-attempt provenance and do not block T8. If T4b has landed, follow only its existing runtime behavior-version maintenance rule.

## Current gap

The current pre-#174 repository still has separate implementations in `src-tauri/src/profile_dsl/runtime/posting_discovery.rs` and `posting_detail.rs`. Each exposes several convenience operations, performs its own pre-cancellation/cardinality checks, iterates Strategies in plan order, evaluates `accepted`, concatenates Diagnostics, and emits `fallback_exhausted`.

The duplicated attempt shapes are `PostingDiscoveryStrategyAttempt` and `PostingDetailStrategyAttempt` in their respective `strategy.rs` files. `runtime/cancellation.rs::contains_runtime_execution_cancelled` makes Diagnostic code inspection part of control flow; Discovery pagination repeats that inspection. `RuntimeExecutionContext` also carries the current caller-owned per-Strategy Discovery request budget.

Current production callers include Search Run Discovery in `search/run/execution.rs`, Source Live Check in `checks/source_live/mod.rs`, and lazy Detail in `search/posting/service.rs`. Coverage exists in `posting_discovery_runtime`, `posting_detail_runtime`, `source_live_check`, Search Run/posting tests, and the Greenhouse, Workday, and SuccessFactors profile regressions. These names describe the drafting baseline only; #174 must first replace phase vocabulary and establish the mandatory compiled policy.

## Target delta

Keep exactly one canonical context-aware public operation per phase, using the exact types landed by #174:

```rust
pub async fn execute_discovery(/* typed plan, clients, context */) -> DiscoveryExecutionResult;
pub async fn execute_detail(/* typed plan, input, clients, context */) -> DetailExecutionResult;
```

Public callers continue to see distinct Discovery and Detail inputs/results. They do not iterate Strategies, inspect attempt history, infer control state from Diagnostics, choose reducers, or call a generic Strategy executor. Delete convenience functions that only select default clients, inject an uncancellable context, or forward to another phase operation; callers construct the landed production or deterministic clients directly.

Add one crate-private closed kernel, provisionally `profile_dsl/runtime/strategy_set.rs`, with responsibility-equivalent typed state:

```rust
enum AttemptAcceptance {
    Accepted,
    Rejected,
}

enum AttemptCompletion {
    Completed { acceptance: AttemptAcceptance },
    Failed,
    Cancelled(TypedCancellation),
}

enum StrategySetTerminal {
    Accepted { accepted_attempt: usize },
    Exhausted { bound_stop_observed: bool },
    Cancelled(TypedCancellation),
}
```

Each record retains immutable Strategy index/key, completion, ordered attempt Diagnostics, and only an already-observable per-operation/per-Strategy bound stop. A phase-typed accepted output travels with its attempt and is returned through a closed Discovery/Detail representation; it is never reconstructed from Diagnostics or erased into `serde_json::Value`.

The kernel owns only mandatory compiled `FirstAccepted`: ordered invocation, attempt history, accepted/rejected/failed/cancelled transitions, deterministic stopping, terminal selection, and concatenation of completed-attempt Diagnostics in Strategy order. Accepted stops immediately; rejected and ordinary failed attempts permit fallback; Cancellation stops immediately; ordinary exhaustion occurs only after all attempts.

Discovery and Detail adapters retain their typed plan/input/output, missing-plan and Strategy-cardinality validation/error projection, one-Strategy execution, phase and Strategy acceptance composition, output validation, reducers, ordinary-failure classification, and final public-result projection. They alone translate an exhausted terminal into exactly one phase-level `fallback_exhausted`, or typed Cancellation into exactly one phase-level `runtime_execution_cancelled`. Attempt Diagnostics remain payload, never policy state.

Cancellation must propagate as typed control flow from HTTP/browser/pagination work through the phase adapter and kernel. `TypedCancellation` retains enough phase/Strategy/sub-operation origin metadata to reproduce the landed Diagnostic path, Strategy key, and details without carrying a Diagnostic as control state. Low-level work emits no cancellation Diagnostic; the matching public phase adapter is its sole translator. Remove `contains_runtime_execution_cancelled` and equivalent production code searches. Preserve prior completed-attempt Diagnostics; cancelled/accepted execution emits no `fallback_exhausted`; rejected, failed, cancelled, or exhausted output is never exposed as accepted output.

Search Run behavior remains exact: cancellation after a phase result preserves one phase-level `runtime_execution_cancelled` and then appends one distinct `source_execution_cancelled`; pre-phase cancellation bypasses phase invocation and emits only `source_execution_cancelled`. Neither case creates Partial Completion, releases finalized candidates, or adds a status.

Inventory the #174-landed runtime bounds. Preserve each enforced value, trigger, Diagnostic, and fallback effect. T8 adds no cumulative arithmetic, ledger, aggregate usage, immutable ceiling, or new public completion type. Runtime-attempt Strategy identity/order/outcome is private provenance, not Effective Profile provenance or fingerprint input.

## Dependency and deletion decision

Policy transitions, attempt state, reducers, Diagnostic ordering, and Cancellation translation remain in-process. Existing HTTP and browser boundaries retain their production and deterministic implementations; no new external seam or speculative trait is introduced. Registry snapshots and compiled plans remain immutable input data.

**Deletion test:** removing the kernel must force both phase modules to re-own `FirstAccepted` transitions, ordered attempt history, typed Cancellation precedence, Strategy key/index provenance, bound-stop propagation, Diagnostic concatenation, and deterministic accepted/cancelled/exhausted stopping. A forwarding wrapper or type rename fails this test.

## Examples

1. **Rejected then accepted:** Discovery `primary` completes but fails acceptance; `fallback` accepts. The fallback candidates are returned, Diagnostics remain in attempt order, no later Strategy runs, and no exhaustion Diagnostic appears.
2. **Failed then accepted:** Detail `api_detail` fails ordinarily; `html_detail` accepts. Only the typed HTML Detail output is returned, with the first attempt’s failure Diagnostics retained.
3. **Exhaustion:** all Strategies reject or fail. The public output is empty and exactly one final `fallback_exhausted` follows all attempt Diagnostics.
4. **Existing bound stop:** the landed caller-owned Discovery request limit stops one Strategy exactly as before; its acceptance/fallback behavior is unchanged and no cross-Strategy usage is calculated.
5. **Cancellation after recovery:** attempt 0 rejects; attempt 1 is cancelled; attempt 2 is not called. Prior Diagnostics precede exactly one phase cancellation Diagnostic, with no exhaustion or accepted output. A narrow private test proves that a non-cancelled attempt whose payload happens to contain the cancellation code text does not cancel execution.

## Scope

- Re-inventory the #174-landed compiler, plans, policy, phase APIs, attempts, bounds, Cancellation plumbing, exports, tests, and production callers.
- Add the one crate-private closed kernel and adapt Discovery and Detail to produce typed attempts and consume typed terminals.
- Propagate typed Cancellation through existing cancellable HTTP/browser/pagination paths and remove code-string control flow.
- Migrate Search Run, Source Live Check/activation, lazy Detail, app/export, deterministic clients, and tests directly to the two canonical context-aware operations.
- Delete phase-local fallback loops, duplicate attempt models, convenience/forwarding operations, aliases, compatibility exports, and superseded implementation-detail tests after equivalent public coverage exists.
- Preserve generic Greenhouse, Workday, and SuccessFactors behavior. Apply a landed T4b runtime behavior-version bump only if its established ownership rule requires one.

## Adjacent non-goals

- T9/#177 cumulative budgets, ledgers, usage, or new limits.
- `all_required`, `at_least`, `collect_all`, parallel execution, or another policy.
- Detection convergence, shared Primitive extraction, transport redesign, or Candidate Resolution.
- Requested Detail fields, provider-value/hint semantics, matching, deduplication, persistence, fingerprints, or new statuses.
- A public Strategy Set/attempt API, generic callback executor, erased output map, policy/reducer trait, plugin registry, or Diagnostic-injection seam.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Compiled policy | Both typed phases execute mandatory compiled `FirstAccepted` through the kernel | Compiler-plus-`strategy_set_runtime` integration test |
| First accepted | One call, first typed output, no later attempt or exhaustion | Discovery and Detail runtime tests |
| Transport-only success | Acceptance rejects output and fallback runs | Both phase tests |
| Rejected/failed recovery | Later accepted output; earlier Diagnostics retained in exact order | Cross-phase deterministic tests |
| Exhaustion | Empty phase output and exactly one final `fallback_exhausted` | Both phase tests |
| Pre-attempt Cancellation | Zero HTTP/browser calls, no attempt output/failure/exhaustion, and exactly one phase cancellation Diagnostic | Both context-aware phase tests with deterministic call counters |
| Mid-HTTP Cancellation | Started work returns typed Cancellation rather than ordinary fetch failure; no later Strategy/output/exhaustion | Both phase tests with deterministic hanging HTTP clients |
| Mid-browser Cancellation | Typed Cancellation stops browser work and later Strategies without code inspection | Applicable Discovery/Detail browser tests |
| Pagination Cancellation | Typed Cancellation stops pagination and later Strategies without ordinary failure/exhaustion | Discovery pagination test |
| Cancellation after recovery | Prior completed-attempt Diagnostics remain ordered; no later Strategy, accepted output, or exhaustion | Cross-phase deterministic tests |
| Search Run cancellation | Post-phase order is runtime then source Diagnostic; pre-phase has source Diagnostic only | Search Run regressions |
| Fake cancellation code | Typed completion wins over Diagnostic text; no public injection seam | Narrow private kernel test plus static review |
| Missing plan/Strategy cardinality | Existing phase adapter error projection remains unchanged and is not converted into ordinary policy exhaustion | Both phase regression tests |
| Existing bounds | Same requests/items, stop point, Diagnostic, acceptance, and fallback; no cumulative usage | Landed boundedness and Source Live Check tests |
| Typed contracts | Discovery and Detail outputs remain distinct; no erased/public generic result | External phase tests and API search |
| Production callers | Search Run, Source Live Check, and lazy Detail behavior remains unchanged | Caller tests; temporary SQLite where already used |
| Source-owned access | Compiled Source-owned Access Path executes through the same typed phase boundary | Compiler-plus-runtime regression |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors outputs/order/Diagnostics remain generic | Existing profile targets |
| Deletion | One transition owner; no code-based cancellation, duplicate loop/model, old wrapper, or raw authored runtime input | Reviewed repository searches |

### Focused commands

Re-baseline target names after #174; expected focused coverage is:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
npm run build

rg -n 'runtime_execution_cancelled|contains_.*cancel|diagnostic\.code.*cancel' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'for .*strateg|strategies\.iter\(|fallback_exhausted' src-tauri/src/profile_dsl/runtime --glob '*.rs'
rg -n '\bpub\s+(trait|struct|enum|type|fn)\s+[A-Za-z0-9_]*(StrategySet|StrategyExecutor|Attempt|Reducer|Policy)|\bdyn\s+[A-Za-z0-9_]*(Reducer|Policy)|\bexecute_strategy_set' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs src-tauri/tests --glob '*.rs'
rg -n -U '(?s)\bpub\s+use\s+[^;]*(strategy_set|StrategySet|StrategyExecutor|Attempt|Reducer|Policy)[^;]*;' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs src-tauri/tests --glob '*.rs'
rg -n '\b(inject_diagnostic|with_injected_diagnostic|diagnostic_injection|DiagnosticInjector|InjectDiagnostic)\b' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs src-tauri/tests --glob '*.rs'
rg -n '\b(PostingDiscovery|PostingDetail)[A-Za-z0-9_]*\b|execute_posting_(discovery|detail)|\b(Legacy|Compat|Compatibility)(Discovery|Detail|StrategySet)|\b(legacy|compat)_(discovery|detail|strategy_set)\b' src-tauri/src src-tauri/tests --glob '*.rs'
find src-tauri/src src-tauri/tests \( -name '*posting_discovery*.rs' -o -name '*posting_detail*.rs' -o -type d \( -name '*posting_discovery*' -o -name '*posting_detail*' \) \) -print
rg -n '^pub async fn execute_(discovery|detail)[A-Za-z0-9_]*|pub use [^;]*execute_(discovery|detail)' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs --glob '*.rs'
rg -n -U '^pub async fn execute_(discovery|detail)[A-Za-z0-9_]*\([^)]*(SourceDocument|SourceProfileDocument|serde_json::Value)' src-tauri/src/profile_dsl/runtime --glob '*.rs'
rg -n 'cumulative|BudgetLedger|StrategySetBudget|parent.*budget|child.*budget|remaining_budget' src-tauri/src/profile_dsl/runtime src-tauri/tests --glob '*.rs'
```

## Ticket-specific migration items

- [ ] Replace both phase-local fallback loops and `PostingDiscoveryStrategyAttempt`/`PostingDetailStrategyAttempt` equivalents with the closed typed kernel record/terminal model.
- [ ] Delete `contains_runtime_execution_cancelled` and classify every cancellation-code hit as output construction/assertion rather than production control flow.
- [ ] Retain exactly one canonical context-aware operation per phase; migrate all callers and delete default-client, uncancellable, forwarding, alias, and compatibility surfaces.
- [ ] Verify exactly one private `FirstAccepted` transition owner and classify other Strategy loops as compiler validation or phase-internal item/pagination work.
- [ ] Verify no public generic runtime abstraction, Diagnostic-injection seam, raw authored runtime input, cumulative budget work, compiler provenance, or fingerprint material crossed the boundary.
- [ ] Replace superseded orchestration tests with public compiler-plus-phase coverage; keep private coverage only for the fake-code invariant or another explicitly unobservable kernel invariant.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
