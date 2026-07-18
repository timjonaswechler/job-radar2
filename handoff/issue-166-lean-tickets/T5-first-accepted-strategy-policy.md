# T5 — Make ordered fallback explicit as compiled `first_accepted`

## Result

Every compiled Discovery and Detail step carries one mandatory typed `first_accepted` Strategy Policy, and both public phase operations dispatch on it while preserving today’s observable ordered-fallback behavior: the first accepted Strategy wins, rejected or failed attempts permit recovery, exhaustion is deterministic, and Cancellation stops later work.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#170](https://github.com/timjonaswechler/job-radar2/issues/170) — Define and specialize one Effective Source Config Schema contract.
- Blocking: [#173](https://github.com/timjonaswechler/job-radar2/issues/173) — Move internal phase modules directly to `detection`, `discovery`, and `detail`.
- Readiness: **Blocked — not ready for agent execution.** #170 is open; re-check readiness after it lands.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 2–4 and 26–28: Strategy Sets use typed policies; policy success is based on accepted attempts; recovered failures remain visible; execution is deterministic and bounded.
- #166 / PRD Decision 36: T5 makes `first_accepted` explicit before the internal phase rename, authored schema-v3 hard cut, and shared Strategy Set runtime.
- The landed #170 compiler must retain `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)`, the authoritative direct Source, a completely validated Effective Source Profile before plan construction, distinct Source-owned access, and one immutable typed Execution Plan. Exact future names and paths must be re-baselined after #170 lands.
- #171 is not a blocker: T5 neither requires nor records Effective Profile provenance or runtime-attempt provenance.
- Shared readiness, hard-cut, test, migration, and evidence rules follow `handoff/issue-166-delivery.md`.

## Current gap

The repository is still on the pre-#170 baseline, so this section is provisional until readiness review:

- `src-tauri/src/profile_dsl/compiler/mod.rs` exposes `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan(snapshot, source_key)` rather than the blocker-provided compiler interface.
- `profile_dsl/documents/posting_discovery.rs` and `posting_detail.rs` define strict authored phase steps with ordered Strategies and optional `accept_when`, but no policy field.
- `profile_dsl/execution_plan/posting_discovery.rs` and `posting_detail.rs` compile ordered Strategies and phase acceptance without a typed policy.
- `profile_dsl/runtime/posting_discovery.rs` and `posting_detail.rs` each own a separate implicit fallback loop. Their private `strategy.rs` modules return phase-specific `{ result, accepted }` attempts.
- Both loops already preserve failed/rejected diagnostics, stop on the first accepted attempt, append `fallback_exhausted` after exhaustion, and suppress later Strategies and exhaustion after `runtime_execution_cancelled`.
- `PostingDiscoveryExecutionBudget` remains a caller-owned per-Strategy request constraint, not a cumulative Strategy Set budget.
- Search Run, Source Live Check, and lazy Detail call the public typed phase operations. Fallback and Cancellation coverage exists in `src-tauri/tests/posting_discovery_runtime/{fallback_acceptance,cancellation}.rs` and `src-tauri/tests/posting_detail_runtime.rs`.

The gap is that this executable policy is an undocumented convention duplicated in two phase entry points rather than mandatory immutable plan data.

## Target delta

Add one shared closed compiled enum and mandatory field on both compiled phase steps; exact module/type names may follow the landed #170 baseline:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanStrategyPolicy {
    FirstAccepted,
}

pub struct ExecutionPlanPostingDiscoveryStep {
    pub policy: ExecutionPlanStrategyPolicy,
    // existing strategies and phase acceptance
}

pub struct ExecutionPlanPostingDetailStep {
    pub policy: ExecutionPlanStrategyPolicy,
    // existing strategies and phase acceptance
}
```

Ticket-specific contract:

1. Every valid compiled Discovery and Detail step has a non-optional typed policy serialized exactly as `"first_accepted"`. There is no raw string, implicit default, `Unknown` variant, or policy on an individual Strategy.
2. `compile_source` materializes `FirstAccepted` for steps from reusable, inherited, Source-specialized, Source-added-Strategy, selected Source-added-Access-Path, and Source-owned access shapes. Existing keyed Strategy order and append-only order for Source-added Strategies remain unchanged.
3. Unknown serialized compiled-policy values fail deserialization. Valid plans cannot omit policy.
4. Authored Discovery and Detail documents remain unchanged and strict. Authored `policy` remains unknown and is rejected; T7 owns authored policy syntax.
5. Empty authored Strategy lists remain rejected by existing compiler semantic/boundedness validation. Defensive runtime Diagnostics for an externally constructed invalid empty plan may remain, but valid compiled plans never rely on them.
6. Each typed phase entry point explicitly matches the compiled policy and executes Strategies sequentially in immutable plan order. There is no parallel/speculative launch.
7. An attempt is accepted only after existing phase execution-failure checks plus phase-level and Strategy-level acceptance. Transport success alone is insufficient. T5 does not change acceptance predicates or failure classification.
8. An accepted attempt returns its phase output and accumulated diagnostics immediately. No later Strategy runs and `fallback_exhausted` is absent.
9. A rejected or failed attempt contributes diagnostics in existing within-attempt and Strategy order; any partial output is discarded. A later accepted attempt succeeds despite the recovered earlier failure.
10. If every attempt completes without acceptance, return empty phase output and append exactly one existing `runtime/fallback_exhausted` Diagnostic after all attempt Diagnostics at the existing phase Strategies path.
11. Pre-attempt or mid-attempt Cancellation returns empty phase output, retains prior Diagnostics, emits the existing single `runtime_execution_cancelled` Diagnostic, suppresses later Strategies and `fallback_exhausted`, and continues through the existing Search Run cancellation path without persistable Resolution Partial Completion or a new status.
12. Existing per-Strategy fetch, pagination, browser, retry, response, timeout, and caller-owned Discovery request bounds remain unchanged. T5 adds no cumulative Strategy Set budget, production ceiling, completion model, count, or persistence behavior.
13. Discovery and Detail keep private phase-specific attempt/result, failure, acceptance, output, and Diagnostic logic. A private phase-local `execute_first_accepted` helper is allowed; a shared kernel, phase-neutral attempt enum, reducer, callback engine, attempt history, typed-cancellation kernel, or public policy executor is not.
14. Public callers continue to invoke typed Discovery and Detail operations and receive the existing result shapes. They do not choose policies or iterate Strategies. Runtime receives only the typed immutable plan.

## Dependency and deletion decision

Policy materialization, dispatch, ordering, acceptance, and terminal selection are in-process. The Registry Snapshot is immutable input data. Existing HTTP/browser production and deterministic test implementations remain the only relevant external seams; this ticket adds no trait or port.

**Deletion test:** Removing the compiled policy would force compiler, Discovery runtime, Detail runtime, tests, and the later shared-runtime migration to rely again on the implicit “ordered Strategies mean first accepted” convention. Conversely, a new forwarding policy facade without owned behavior fails this test and must not be introduced.

## Examples

1. **Materialization:** authored `postingDiscovery` with `[primary, fallback]` and no policy compiles to the same ordered Strategies plus `policy: "first_accepted"`; authored JSON remains unchanged.
2. **Recovery:** `primary` fetches successfully but fails `minResults`; `fallback` accepts. Requests are `[primary, fallback]`, fallback output is returned, primary acceptance Diagnostics remain, and exhaustion is absent.
3. **Failure then success:** primary fetch/parse failure is retained, partial primary output is discarded, and accepted fallback output is returned without making the phase terminally failed.
4. **Exhaustion:** all attempts reject/fail; empty output is returned with attempt Diagnostics followed by exactly one `fallback_exhausted`.
5. **Cancellation during fallback:** completed first-attempt Diagnostics precede one `runtime_execution_cancelled`; no third Strategy runs, no exhaustion appears, and no `ResolutionCompletion::Partial` is created.

## Scope

- Re-baseline against the exact landed #170 compiler, plan types, Source fragment shapes, Source-owned path, Diagnostics, callers, and tests.
- Add the single shared compiled policy enum and mandatory fields to compiled Discovery and Detail steps.
- Materialize `FirstAccepted` through every compiler path listed in the Target delta; keep Strategies policy-free.
- Dispatch explicitly on compiled policy in `execute_posting_discovery_with_clients_and_context` and `execute_posting_detail_with_clients_and_context` while preserving current phase-private behavior.
- Cover compiler-plus-runtime behavior through `compile_source` and public Discovery/Detail operations using deterministic existing clients.
- Update Search Run, Source Live Check, lazy Detail, and fixtures only where mandatory plan construction, serialization, or destructuring requires it.
- Delete policy-agnostic entry-point iteration once the two explicit temporary phase-local policy arms own it, plus any duplicate enum, raw parser, optional default, conversion, alias, wrapper, or superseded implementation-detail test introduced or replaced by this slice.

## Adjacent non-goals

- T6/#173 internal phase renaming or T7 authored schema-v3/policy syntax.
- T8 shared Strategy Set kernel, typed attempt history/Cancellation, reducers, or cumulative budgets.
- `all_required`, `at_least`, `collect_all`, additional policies, parallelism, or Strategy placement/reordering.
- Detection policy execution, provenance/fingerprints, Primitive extraction, Candidate Resolution, requested Detail fields, matching, persistence, or status changes.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Compiler coverage | Every reusable, specialized, Source-added, and Source-owned compiled Discovery/Detail step carries `FirstAccepted`; Strategies do not | External `compile_source` integration test |
| Source-added order | Existing keyed order and append-only order for Source-added Strategies are unchanged while policy is materialized | External compiler regression |
| Empty Strategy list | Existing compiler semantic/boundedness rejection remains; valid compiled plans never depend on defensive runtime handling | Compiler boundedness regression |
| Serialized boundary | Compiled value is exactly `first_accepted`; missing/unknown compiled values and authored `policy` are rejected without defaulting | Plan Serde plus Source/Profile schema/Serde fixtures |
| Accepted first | First output returns; later Strategy and exhaustion are absent | Public Discovery and Detail tests with deterministic clients |
| Rejected/failed recovery | Later accepted output returns; earlier ordered Diagnostics remain; partial output is discarded | Cross-phase runtime tests |
| Acceptance composition | Transport success does not accept; existing phase and Strategy predicates both apply | Existing/focused acceptance tests |
| Exhaustion/order | Empty output; attempt Diagnostics remain ordered; exactly one terminal `fallback_exhausted` | Discovery and Detail deterministic tests |
| Pre-/mid-Cancellation | Empty output; one cancellation Diagnostic; prior Diagnostics retained; no later work/exhaustion | Context-aware Discovery and Detail tests |
| Search Run Cancellation | Existing Search Run cancellation path is preserved; no persistable Resolution Partial Completion or new status appears | Search Run/runtime regression and static review |
| Existing bounds | Per-Strategy/caller-owned budget behavior is unchanged; no cumulative accounting appears | Existing budget regression and plan/type review |
| Production callers | Search Run, Source Live Check, and lazy Detail retain typed operations and no caller-owned policy loop | Caller regressions plus repository search |
| Profile regression | Greenhouse, Workday, and SuccessFactors plans carry the policy with otherwise unchanged behavior | Existing deterministic profile tests |

Primary behavior tests cross `compile_source`, then the public Discovery or Detail operation, and assert output, request order/count, Diagnostics, and stop behavior. Existing real phase code and deterministic HTTP/browser implementations are used; tests do not depend on private step builders or attempt structs.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test posting_discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test posting_detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
rg -n '\bExecutionPlanStrategyPolicy\b|first_accepted|fallback_exhausted|runtime_execution_cancelled' src-tauri/src src-tauri/tests
rg -n 'execute_posting_(discovery|detail)_with_clients_and_context' src-tauri/src src-tauri/tests
```

At readiness review, add a differently named landed Effective Profile Compiler target only if #170 introduces one.

## Ticket-specific migration items

- [ ] Add one mandatory compiled `FirstAccepted` field to both phase-step types and every compiler construction path; prove no Strategy carries policy.
- [ ] Keep authored phase documents policy-free and reject authored `policy`; reject missing/unknown compiled policy without compatibility defaults.
- [ ] Replace policy-agnostic iteration in `execute_posting_discovery_with_clients_and_context` and `execute_posting_detail_with_clients_and_context` with explicit policy dispatch while retaining private phase attempts and public operations.
- [ ] Move primary fallback coverage to plans obtained through `compile_source`; update only mandatory-policy construction/destructuring in production callers and fixtures.
- [ ] Delete duplicate policy enums, raw-string dispatch, optional/default policy paths, forwarding wrappers, compatibility conversions, and superseded policy-agnostic loops/tests.
- [ ] Classify all remaining hits from focused searches for `first_accepted`, policy construction/defaulting, `fallback_exhausted`, phase Strategy loops, and provider-specific dispatch; no valid compiled step may omit or ignore policy.
- [ ] Confirm the diff contains no cumulative Strategy Set budget, shared phase-neutral kernel, Candidate Resolution/Partial Completion type, or new status variant.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
