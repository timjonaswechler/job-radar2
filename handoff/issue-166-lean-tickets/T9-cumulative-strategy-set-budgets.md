# T9 — Enforce cumulative Strategy Set budgets

## Result

Every compiled Discovery and Detail `FirstAccepted` invocation enforces one deterministic cumulative budget across all attempted Strategies and nested work. The typed phase result reports exact committed usage and distinguishes normal policy completion, cumulative budget exhaustion, and typed Cancellation; work that would exceed an effective ceiling is denied before its side effect.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#176 — T8 — Route Discovery and Detail through one typed Strategy Set runtime](https://github.com/timjonaswechler/job-radar2/issues/176).
- Readiness: **Blocked**. Re-baseline all provisional paths and names below against the landed T8 kernel before implementation.
- Open decision: none. The immutable ceilings and the T9/T10 byte/retry boundary are accepted.

## Consumed contracts

- #166 / PRD Decisions 27–28 and the “Strategy Set Runtime” module decision: budgets are cumulative, immutable backend ceilings cannot be weakened, public phase operations stay typed, and one private kernel owns deterministic policy execution.
- `handoff/issue-166-delivery.md`: shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.
- #176 supplies mandatory compiled `FirstAccepted`, separate typed Discovery/Detail operations, one crate-private Strategy Set kernel, ordered attempt history, runtime-attempt provenance, typed Cancellation, and typed policy/attempt terminals. T9 extends those landed types rather than adding a second loop, result wrapper, or Diagnostic-based control path.
- The compiler continues to receive the directly supplied Source as authoritative, compile an immutable Execution Plan, expose an Effective Source Profile for profile-based access, and keep Source-owned Access Paths distinct.
- #171 compiler merge-origin provenance and T4b fingerprint work are not prerequisites. If T4b has landed, apply its existing ownership rules to every affected partition: compiler validation/plan-material changes require the compiler partition, execution changes require the runtime partition, and the changed Source Live Check global-inventory interpretation/material requires the existing immutable-globals partition without adding a fourth inventory row.

## Current gap

The repository is still pre-T8. Current names are therefore evidence of the gap, not implementation targets:

- `profile_dsl/compiler/mod.rs::compile_source_execution_plan` and `execution_plan/{posting_discovery,posting_detail,capabilities}.rs` produce old phase plans with no Strategy Policy, cumulative limits, usage, completion, or ledger.
- `compiler/boundedness.rs` caps fallback lists at 50 and validates positive local fetch, pagination, and browser bounds. JSON Schemas cap pagination at 1,000 requests, 100,000 items, and depth 20; HTTP/browser fetch timeouts at 60,000/120,000 ms; and one interaction at 50. These limits reset at their local owner.
- `runtime/posting_discovery.rs::execute_posting_discovery_with_clients_and_context` and `runtime/posting_detail.rs::execute_posting_detail_with_clients_and_context` each own a fallback loop and infer Cancellation partly from Diagnostics. T8 must replace this structure.
- `runtime/cancellation.rs::PostingDiscoveryExecutionBudget` provides only `max_requests_per_strategy`; Source Live Check supplies one request per Discovery Strategy. Pagination separately counts requests/items/depth, while non-paginated acquisitions, Detail, browser actions, fan-out, elapsed time, and failed fallback usage have no invocation-wide accounting.
- Responses are decoded `String` values and retry metadata has no executable retry loop. Therefore truthful pre-read byte enforcement and retry usage do not exist.
- Search Run, Source Live Check, and lazy Detail are the production callers. Relevant current coverage includes `posting_discovery_runtime`, `posting_detail_runtime`, `compiler_security_boundedness`, `compiler_resolution`, `schema_validation`, `source_live_check`, and the Greenhouse, Workday, and SuccessFactors profile regressions.

At readiness review, replace this section with the exact #176-landed plans, kernel, result/Cancellation algebra, browser/HTTP boundaries, callers, and test targets.

## Target delta

### Immutable and effective limits

One canonical backend owner defines these immutable ceilings per public Discovery or Detail invocation:

| Dimension | Ceiling |
|---|---:|
| Strategy attempts | 50 |
| Logical external requests/browser navigations | 1,000 |
| Retained produced items | 100,000 |
| Elapsed duration | 120,000 ms |
| Paginated pages/sitemap documents | 1,000 |
| Browser actions attempted | 50 |
| Admitted fan-out work items | 100,000 |

Schema-v3 Discovery and Detail Strategy Sets gain one optional typed `limits` object beside mandatory `policy` and ordered `strategies`, with `maxAttempts`, `maxRequests`, `maxProducedItems`, `maxDurationMs`, `maxPages`, `maxBrowserActions`, and `maxFanOut` only. Every value is a positive integer no greater than its backend ceiling. Omission means no authored tightening, so the backend ceiling applies; `null`, zero, “unlimited,” unknown members, and above-ceiling values are invalid.

A Base Source Profile may author limits. A direct Source fragment may inherit or tighten an existing Strategy Set limit but may not raise or remove it. A complete Source-added Strategy Set or Source-owned Access Path may author values no greater than backend ceilings. Attempted weakening is rejected with a deterministic compiler Diagnostic at the authored schema-v3 limit path; it is never silently clamped. Successful compilation places mandatory fully resolved typed limits in the immutable Discovery/Detail plan.

Both phase operations accept one typed optional caller-tightening value for these seven dimensions. For each dimension:

```text
effective = min(backend ceiling, compiled limit, optional caller limit)
```

Caller values cannot widen compiled/backend capacity and invalid caller values are unconstructable or rejected before execution. Caller limits are runtime control data, not authored Profile/Source data or persistence. Source Live Check Discovery intentionally changes from one request per Strategy to cumulative `maxRequests: 1`; its Detail call keeps the T8-landed control behavior. Search Run and lazy Detail gain no invented product budget.

### Typed report and private ledger

Extend the landed Discovery and Detail result/terminal algebra directly with a read-only budget report equivalent to:

```rust
StrategySetBudgetReport {
    completion: Completed | BudgetExhausted {
        dimension, used, requested, effective_limit
    },
    usage: StrategySetUsage {
        attempts, requests, produced_items, elapsed_ms,
        pages, browser_actions, fan_out
    },
}
```

Exact public names and placement follow #176. `Completed` means no cumulative debit was denied; accepted versus ordinary `FirstAccepted` exhaustion remains the policy outcome. `BudgetExhausted` means required work was denied before its side effect and is distinct from ordinary policy exhaustion, local bounds, failure, and typed Cancellation. Usage includes committed work from accepted, rejected, failed, and cancelled attempts, excludes denied work, and remains observable for every terminal. It contains no compiler provenance, payload/body, credentials, Candidate Resolution counts, or persistence state.

The T8 kernel owns one crate-private root ledger per phase invocation. Strategy and nested child scopes are accounting views over that same root: they neither reset/mint capacity nor reserve independent limits. No mutable ledger, scope, reservation, generic Strategy executor, attempt model, reducer, or policy callback becomes public.

Ledger invariants:

1. Known-cost multi-dimensional debits check all affected counters with checked arithmetic, then commit all exactly once or none. Overflow is a deterministic internal runtime failure, not exhaustion.
2. `used + requested <= limit` is admitted. Equality alone is not exhaustion; exhaustion occurs only when further required work is denied.
3. Failed/rejected work and Cancellation refund nothing. No debit or work follows Cancellation or exhaustion.
4. Cancellation is checked before debit and before side effect. If Cancellation and duration expiry are simultaneously ready, Cancellation wins. Diagnostics never determine control flow.
5. Existing per-operation/per-Strategy bounds remain independently owned and retain their landed local-stop behavior.

### Charging semantics

- **Attempts:** debit one immediately before Strategy execution; a denied attempt performs no acquisition, interpretation, acceptance, or reducer work.
- **Requests:** debit before each logical DSL acquisition, including non-paginated Discovery/Detail fetch and browser navigation. Redirect hops internal to the existing transport are not separately charged. Started failed/cancelled calls remain counted.
- **Pages:** debit atomically with the request for every paginated page or sitemap-document fetch. Non-paginated acquisition consumes no page.
- **Produced items:** charge typed phase items after extraction/output validation, in deterministic provider/Strategy order. Discovery candidates consume one each; a present valid Detail output consumes one. If a batch exceeds remaining capacity, commit only the fitting prefix, deny the next item, and terminate exhausted. Under `FirstAccepted`, expose no partial phase output; the prefix remains visible only in usage and Diagnostics.
- **Elapsed duration:** start one monotonic deadline at public invocation. Cap active async work by remaining duration and deny continuation when no positive time remains. Race active work with priority Cancellation, operation completion, deadline, so exact-boundary completion may succeed. Duration exhaustion reports actual elapsed milliseconds (not clamped), `requested = 1`, and the effective duration; already admitted active operations remain counted. Use paused/advanced Tokio time or the landed deterministic facility, not a clock trait introduced only for tests.
- **Browser actions:** debit immediately before each actual `click_if_visible` or `click_until_gone` side effect. Navigation consumes a request; waits and `waitAfterMs` consume duration only. Extend the existing production/deterministic browser boundary so capacity is checked before action and typed exhaustion/usage propagates; do not add another browser adapter.
- **Fan-out:** charge each non-duplicate child sitemap/follow-up item immediately before enqueue. A denied item is not queued, and deterministic queue order is preserved.
- **Depth:** remains a local structural pagination bound.
- **Bytes/retries:** absent. T10 owns byte-preserving responses, bounded acquisition/decoding, and later activation of response-byte charging. T9 must not use decoded `String::len()`. Retry accounting remains absent until an accepted executable retry capability exists.

### Terminal precedence and Diagnostics

Preserve the T8 attempt states and apply deterministic precedence: typed Cancellation already observed; accepted output already completed; denied cumulative debit; ordinary `FirstAccepted` exhaustion; then landed local-bound behavior. Accepted output exactly at a limit is accepted with `Completed`. A rejected/failed exact-boundary attempt followed by required fallback work exhausts when the next attempt debit is denied.

Budget exhaustion returns no `fallback_exhausted`; Cancellation returns neither budget exhaustion nor `fallback_exhausted`. Prior attempt/local Diagnostics remain in Strategy and within-attempt order. Exhaustion appends exactly one phase-level runtime Diagnostic with the canonical landed phase path and Strategy/operation identity. Details include dimension, used, requested, effective limit, and deterministic winning sources. Numeric source ties list `backend`, `compiled`, `caller`; an atomic debit denied on several dimensions selects the primary in order `attempts`, `requests`, `produced_items`, `duration`, `pages`, `browser_actions`, `fan_out`, while details may list all in that order.

Cancellation continues through the existing Search Run path and creates no `ResolutionCompletion`, partial persistence behavior, or new Source/Source Run/Search Run status.

## Dependency and deletion decision

Authored/compiled limits, immutable ceilings, effective-limit selection, checked arithmetic, and parent/child usage are in-process data/computation. HTTP remains the existing true-external boundary; request admission occurs before it. Browser execution remains the existing local-substitutable boundary and gains pre-action admission in both production and deterministic implementations. Monotonic Tokio time is the existing runtime facility. SQLite is unchanged and used only by existing caller regressions.

**Deletion test:** Removing the ledger must force strictest-limit selection, atomic checked charging, usage/deadline state, and terminal precedence into the Strategy Set kernel, Discovery pagination/fan-out, Detail output handling, HTTP/browser paths, Source Live Check controls, and their tests. A forwarding budget type fails this test.

## Examples

1. **Shared request ceiling:** with compiled `maxRequests: 3`, a rejected Strategy commits two requests and a fallback commits one. Its next request is denied with `used=3`, `requested=1`; no later Strategy runs and no `fallback_exhausted` is emitted.
2. **Exact boundary:** one allowed attempt and request produce accepted Detail output. The policy result is accepted, completion is `Completed`, and usage is one attempt, one request, and one produced item.
3. **Atomic page debit:** with one request but zero pages remaining, the next paginated fetch is denied on pages; neither counter changes and no external call occurs.
4. **Item prefix:** with capacity two, `[A, B, C]` charges A and B, denies C, preserves order, and exposes no accepted Discovery output.
5. **Duration/Cancellation:** a request started at 900 ms under a 1,000 ms limit remains counted if the deadline interrupts it. If Cancellation is simultaneously observable, typed Cancellation wins and no budget terminal is produced.
6. **Local bound:** a Strategy-local pagination maximum of two can stop before a cumulative request limit of twenty; its existing local Diagnostic remains and completion is `Completed` unless later cumulative work is denied.

## Scope

- Add the seven-field Strategy Set limits shape to schema-v3 Profile, direct Source fragment, Source-added, and Source-owned documents with schema/Serde parity and tighten-only compilation.
- Add one canonical ceiling owner and mandatory resolved limits to immutable Discovery/Detail plans.
- Extend the landed T8 kernel with one private root/child ledger and route every attempt, request/page, produced item, browser action, fan-out admission, and deadline through it.
- Extend typed phase results with exact usage/completion while preserving distinct Discovery/Detail outputs, acceptance, attempt provenance, Cancellation, and local bounds.
- Replace the Discovery-only per-Strategy caller budget with cumulative caller tightening for both phases; migrate Source Live Check Discovery to one cumulative request.
- Migrate Search Run, Source Live Check/activation, lazy Detail, exports, HTTP/browser deterministic clients, and tests directly.
- Delete old per-Strategy arithmetic, resettable counters, duplicate usage/completion models, wrappers, aliases, and superseded implementation-detail tests.
- Update active schema/domain/runtime documentation only as required by the landed serialized contract. If T4b has landed, apply its compiler, runtime, and immutable-global interpretation/material version rules to the affected T9 behavior without changing fingerprint shape or adding an inventory row.

## Adjacent non-goals

- Byte-preserving transport, bounded decoding, and response-byte accounting: T10/#178.
- `all_required`, `at_least`, or `collect_all`: T13a/T13b/T13c.
- Detection convergence, shared Primitive extraction, transport redesign beyond browser admission, or executable retries.
- Candidate Resolution batches/counts/completion/sampling, matching, normalization, deduplication, persistence, or new statuses.
- Parallel/speculative Strategy execution, reservations, rate-limit scheduling, resumability, or checkpoints.
- A public generic executor, mutable budget API, budget service/trait, policy/reducer adapter, plugin interface, or speculative Candidate Resolution port.
- Compiler merge-origin provenance, fingerprint design, Source Config Schema expansion, or Search Request criteria in Source/Profile configuration.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Omitted/authored limits | Omission compiles backend ceilings; valid tightening compiles exact resolved limits | External compiler tests |
| Direct Source tightening | A valid fragment lowers an inherited existing-set limit in the effective profile and plan, and the typed phase runtime enforces that lower value | Compiler-plus-Discovery/Detail tests |
| Source-owned Access Path | Same compiled and runtime limit behavior without a fake Effective Source Profile | Compiler-plus-runtime test |
| Source/backend weakening | Raise/remove, zero/null/unlimited/unknown/above-ceiling values reject at the authored path; no plan | Schema/Serde/compiler tests |
| Caller tightening/widening/tie | Strictest value wins; widening grants nothing; tie sources are ordered deterministically | Both phase tests |
| Invalid caller limit | Zero or otherwise invalid tightening is unconstructable or rejected before execution; no debit or call | External API test |
| Attempts exact/one-over | Last allowed acceptance completes; denied next attempt performs no work | Discovery and Detail tests |
| Requests cumulative | Failed/rejected fallback usage carries forward; denied call has no side effect | Discovery and Detail tests |
| Non-paginated request failure/Cancellation | Started failed or cancelled Discovery/Detail requests remain charged exactly once; denied requests are never started | Both phase deterministic-client tests |
| Page/request atomicity | Denial changes neither counter and performs no fetch | Discovery pagination test |
| Produced items exact/one-over | Exact batch may accept; one-over charges ordered prefix but exposes no partial output | Discovery test |
| Detail produced item | Present valid Detail output charges one without changing typed output | Detail test |
| Duration exact/one-over | Completion-at-boundary may succeed; winning deadline interrupts and reports actual elapsed | Both phases with paused time |
| Browser actions | Actual clicks share one allowance; navigation is request; waits are duration; denied click has no side effect | Deterministic browser tests |
| Fan-out | Exact ordered prefix queues; denied child is absent | Sitemap Discovery test |
| Local bound first | Existing local behavior/Diagnostic remains, with no false cumulative exhaustion | Landed boundedness regressions |
| Failure/rejection/mid-operation Cancellation | Committed usage remains; Cancellation is typed and wins without budget/fallback terminal | Both phases and Search Run |
| Pre-Cancellation | No debit or external call; typed Cancellation; no budget/fallback terminal; zero counters except elapsed projection | Both phase tests |
| Accepted/budget stopping | No later debit/Strategy; prior attempt/local Diagnostics retain exact order, followed by one budget terminal Diagnostic and no `fallback_exhausted` | Cross-phase Strategy Set test |
| Multi-dimension/overflow | Stable primary ordering; all-or-none mutation; overflow is internal failure | External test plus narrow private ledger test |
| Source Live Check | At most one cumulative Discovery request; Detail control and Check Report semantics otherwise remain | `source_live_check` |
| Search Run/lazy Detail | Report plumbing adds no status or persistence change; lazy behavior remains | Search Run and temporary-SQLite posting-service regressions |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors retain generic deterministic behavior | Existing profile tests |
| Bytes/retries absent | No fake fields, decoded-body counting, retry execution, or retry usage | Static search/review |
| Immutable boundary/deletion | Runtime receives typed plans; one ledger owner; no resettable/compatibility path or public generic budget API | Call-graph and repository searches |

Tests cross the real compiler and landed typed phase operations, using existing deterministic HTTP/browser implementations. Assertions cover call/action/enqueue order, typed output, terminal, exact usage, Diagnostics, and suppression of later work; private tests are limited to arithmetic/ordering states impractical through valid public plans.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_budget
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run build
```

Replace provisional T8 target names with landed equivalents; do not silently omit a behavior group.

## Ticket-specific migration items

- [ ] Re-baseline #176-landed compiler, schema-v3 documents, plans, kernel, typed terminals/Cancellation, phase results, browser/HTTP boundaries, callers, and tests.
- [ ] Add the seven active limits with schema/Serde parity, tighten-only compiler Diagnostics, canonical ceilings, and mandatory plan values.
- [ ] Add one private ledger and cover every charging point, checked atomic debit, deadline, terminal precedence, and exact usage.
- [ ] Move Source Live Check from `PostingDiscoveryExecutionBudget`/`max_requests_per_strategy` to cumulative `maxRequests: 1`; migrate Search Run, lazy Detail, exports, and deterministic clients.
- [ ] Delete `PostingDiscoveryExecutionBudget`, `posting_discovery_request_limit`, the `maxRequestsPerStrategy` Diagnostic path, resettable counters, duplicate models, wrappers, aliases, and superseded tests after callers move.
- [ ] Verify no byte/retry placeholder, Candidate Resolution work, public generic/mutable budget interface, Diagnostic-code control flow, raw authored runtime input, or provider branch remains.
- [ ] Run and classify landed-name equivalents of these searches:

```bash
rg -n 'BudgetLedger|BudgetScope|StrategySetUsage|BudgetExhausted|request_count|queue\.push|action_count|elapsed|deadline|debit|charge|for .*strateg' src-tauri/src/profile_dsl src-tauri/src/browser_runtime src-tauri/tests --glob '*.rs'
rg -n 'PostingDiscoveryExecutionBudget|max_requests_per_strategy|maxRequestsPerStrategy|posting_discovery_request_limit|execute_posting_(discovery|detail)|\b(legacy|compat)_(discovery|detail|strategy_set|budget)\b' src-tauri/src src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'budget_exhausted|budget_reached|runtime_execution_cancelled|diagnostic\.code|contains_.*(budget|cancel)' src-tauri/src/profile_dsl/runtime src-tauri/src/search --glob '*.rs'
rg -n '\bpub\s+(trait|struct|enum|type|fn)\s+[A-Za-z0-9_]*(BudgetLedger|BudgetScope|Reservation|StrategyExecutor|Attempt|Reducer|PolicyAdapter)|execute_strategy_set' src-tauri/src/profile_dsl/runtime src-tauri/src/lib.rs --glob '*.rs'
rg -n 'maxBytes|maxResponseBytes|body\.len\(\)|bytes_used|retry_count|retries_used|ResolutionCompletion|ResolutionCounts|budgetSkipped|candidateDiagnosticsOmitted' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'SourceDocument|SourceProfileDocument|serde_json::Value|greenhouse|workday|successfactors|profile_key|source_key.*(match|==)' src-tauri/src/profile_dsl/runtime --glob '*.rs'
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
