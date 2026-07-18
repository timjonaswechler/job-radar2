# T6 — Move internal phase modules directly to `detection`, `discovery`, and `detail`

## Result

All Rust modules, types, fields, functions, imports, exports, helpers, test targets, and production callers use the final phase vocabulary `detection`, `discovery`, and `detail`. Current schema-v2 Source/Profile JSON continues to read and write `detect`, `postingDiscovery`, and `postingDetail`, with unchanged runtime behavior and Structured Diagnostics, until T7 performs the authored schema-v3 hard cut.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#172](https://github.com/timjonaswechler/job-radar2/issues/172).
- Blocking: [#174](https://github.com/timjonaswechler/job-radar2/issues/174).
- Readiness: **Blocked** by #172. Re-baseline the provisional paths and names below against the landed predecessor before assignment.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 1 and 36–38: final phase vocabulary, compiler-first delivery, and direct pre-production hard cuts without compatibility runtimes.
- #166 / PRD Strategy Set and module decisions: runtime consumes immutable typed Execution Plans through phase-specific public operations; Detection remains a distinct typed Source Proposal operation.
- #172 provides the public `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)` flow, distinct profile-based and Source-owned access, and mandatory typed `FirstAccepted` on compiled Discovery and Detail steps.
- #172 preserves sequential accepted-first execution, rejected/failed fallback recovery, exhaustion, deterministic Diagnostics, existing bounds, and Cancellation behavior. T6 renames that landed implementation without changing it.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

## Current gap

The repository is still pre-predecessor, so this baseline is provisional until #172 lands. Authored document modules are `profile_dsl/documents/posting_discovery.rs` and `posting_detail.rs`; `ReusableAccessPathDocument`, `SelectedAccessPath`, and related compiler code use `posting_discovery`/`posting_detail`. `SourceProfileDocument` exposes authored Detection as `detect`.

Compiled modules are `profile_dsl/execution_plan/posting_discovery.rs` and `posting_detail.rs`, and `SourceExecutionPlan` has posting-prefixed fields. Runtime modules and private directories live under `profile_dsl/runtime/posting_discovery*` and `posting_detail*`. Crate exports expose `PostingDiscovery*`, `PostingDetail*`, `execute_posting_discovery*`, and `execute_posting_detail*`; `RuntimeExecutionContext` exposes `PostingDiscoveryExecutionBudget` and posting-prefixed budget methods.

Production callers include Search Run execution, Source Live Check and activation, lazy posting Detail, and app-command fetcher construction. External targets include `posting_discovery_runtime`, `posting_detail_runtime`, compiler/schema tests, `source_profile_detection`, `source_live_check`, and the Greenhouse, Workday, and SuccessFactors regressions.

The gap is naming, not behavior: schema-v2 authored terminology has leaked into internal modules, compiled plans, public Rust operations, tests, and callers. Detection already runs under `source_profile::detection`; T6 must not create a parallel Detection runtime.

## Target delta

The direct cut establishes this internal contract, adapting exact qualifiers only to the code landed by #172:

| Concern | Final contract |
|---|---|
| Authored Detection | Internal `detection`/`Detection*`; explicit Serde name `detect` |
| Authored Discovery | `profile_dsl::documents::discovery`; `Discovery{Step,Strategy,Extraction,Fields}` |
| Authored Detail | `profile_dsl::documents::detail`; `Detail{Step,Strategy,Extraction,Fields}` |
| Effective/direct fragments | Internal `discovery` and `detail`; explicit old Serde names only where still authored schema v2 |
| Compiled phases | `execution_plan::{discovery,detail}`; `ExecutionPlanDiscovery*` and `ExecutionPlanDetail*` |
| Compiler | `compile_discovery_step`, `compile_detail_step`, `TemplateContext::{Discovery,Detail}`, and final-vocabulary helpers/locals |
| Execution Plan | `SourceExecutionPlan::{discovery,detail}`, serialized as `discovery`/`detail` |
| Runtime | `runtime::{discovery,detail}` and matching private directories |
| Public Discovery API | `DiscoveryCandidate`, `DiscoveryExecutionResult`, `DiscoveryFetch*`, `DiscoveryFetcher`, `ReqwestDiscoveryFetcher`, `execute_discovery*` |
| Public Detail API | `DetailExecutionResult`, `DetailPostingOccurrence`, `DetailFetch*`, `DetailFetcher`, `ReqwestDetailFetcher`, `execute_detail*` |
| Existing request bound | `DiscoveryExecutionBudget`, `with_discovery_budget`, `discovery_request_limit`; semantics unchanged |

`detect_source_proposal*`, `SourceProposal`, and existing Detection HTTP/browser types already use domain-appropriate names and need not change for symmetry. Detection keeps its current Source Proposal responsibility and does not move into the compiled Discovery/Detail runtime.

Old phase spellings remain allowed only as observable schema-v2 boundary data:

1. authored Source/Profile properties, schema `$defs`/`$ref`s and required lists, Built-in profiles, authored fixtures, and explicit Serde values for `detect`, `postingDiscovery`, and `postingDetail`;
2. any still-landed serialized schema-v2 Source-fragment enum value required to parse current documents;
3. authored JSON literals that prove schema-v2 parsing, serialization, and compilation;
4. existing Structured Diagnostic pointers, codes, and messages that refer to schema-v2 authored locations, including posting-prefixed diagnostic values.

No old Rust module, identifier, field, function, method, import/export, test target/directory, compiled-plan field, alias, wrapper, bridge, or migration name may remain. Compiled-plan serialization changes to `discovery`/`detail` and gains no old-name deserialization alias.

Both compiled phase steps retain mandatory typed `FirstAccepted` and serialized `first_accepted`. Strategy order, accepted-first stopping, rejected/failed recovery, exhaustion output and its single terminal `fallback_exhausted`, Diagnostic content/order, candidate/result shapes, Detail laziness, and production reports remain unchanged. Pre- or mid-phase Cancellation retains earlier Diagnostics, prevents later Strategies, returns the same empty output with one `runtime_execution_cancelled`, suppresses `fallback_exhausted`, and follows the existing Search Run cancellation path.

Existing per-Strategy fetch, pagination, browser, response-size, timeout, retry, and caller-owned Discovery request bounds remain unchanged. T6 adds no cumulative Strategy Set budget. Runtime continues to receive only immutable typed plans.

## Dependency and deletion decision

Authored and compiled documents, compiler naming, phase acceptance/reducers, and runtime control remain in-process. Existing HTTP fetch and browser seams are renamed in place; their production implementations remain the Reqwest and managed-browser clients, and their deterministic test implementations migrate directly. SQLite behavior and schema do not change.

No new trait, port, common executor, or naming facade is justified.

**Deletion test:** Removing the final phase modules would spread document compilation, typed phase execution, Search Run Discovery, Source Live Check, and lazy Detail complexity across compiler and production callers. A re-export-only or forwarding naming facade would remove no such complexity and therefore must not be introduced.

## Examples

1. **Schema-v2 authored input:** `postingDiscovery` and optional `postingDetail` deserialize into `DiscoveryStep` and `DetailStep`; Rust accesses `.discovery` and `.detail`, while authored round trips retain the old property spellings.
2. **Compiled plan:** `compile_source` returns a plan whose Rust and serialized members are `discovery` and `detail`; callers invoke `execute_discovery*` and `execute_detail*` directly. No old compiled field or public wrapper exists.
3. **Fallback parity:** if the first Strategy rejects or fails and the second accepts, calls and Diagnostics remain ordered, the second output is returned, and `fallback_exhausted` is absent.
4. **Cancellation during fallback:** earlier Diagnostics remain, the current phase returns the existing empty result plus one `runtime_execution_cancelled`, later Strategies do not run, and no exhaustion or persistable Resolution Partial Completion is created.
5. **Diagnostic compatibility:** an invalid schema-v2 Discovery selector still points to `/accessPaths/0/postingDiscovery/strategies/0/select`; only the Rust identifier that emits it changes.

## Scope

- Re-baseline against #172, then move authored Discovery/Detail document modules and full Rust type families to final names.
- Rename internal Detection document fields/types while preserving explicit schema-v2 Serde; keep `source_profile::detection` as the sole Detection implementation.
- Rename Access Path, Source-owned Access Path, Effective Source Profile, direct-fragment, compiler, and template identifiers. Do not restore any Source Override model removed by predecessors.
- Move compiled modules/types/builders and `SourceExecutionPlan` fields/serialization to final names while retaining mandatory `FirstAccepted`.
- Move runtime modules/private directories, all public/private symbols, and the existing caller-owned Discovery request budget to final names.
- Migrate crate-root exports and every Search Run, Source Live Check/activation, lazy Detail, app-command, compiler/provenance, and deterministic-client caller directly.
- Rename external runtime targets, support directories, test helpers, and Rust identifiers. Keep authored schema-v2 fixtures and JSON literals on their current serialized vocabulary.
- Add external `phase_module_naming` coverage proving schema-v2 input compiles into final typed/plan names and executes unchanged behavior through final public operations.
- Preserve schema-v2 Diagnostic values while renaming the Rust identifiers that emit them.
- Delete all replaced files, directories, symbols, exports, tests, aliases, wrappers, bridges, compatibility conversions, and phase-migration names in the same slice.

## Adjacent non-goals

- T7/#174's authored schema-v3 migration and any dual-v2/v3 reader.
- T8's shared Strategy Set kernel, attempt model, cumulative ledger, or Detection convergence.
- Additional policies or acceptance semantics beyond T5's compiled `FirstAccepted`.
- Primitive extraction, Candidate Resolution, requested multi-field Detail, persistence, provenance/fingerprint redesign, or Source Config Schema/merge changes.
- Provider-specific Rust, parallel/speculative execution, resumability, or new status variants.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Detection authored boundary | `detect` maps to final internal names with unchanged Source Proposal and Diagnostics | `source_profile_detection` |
| Discovery/Detail authored boundary | Current JSON parses/round-trips with old spellings through final Rust types | Schema/Serde and compiler integration tests |
| Compiled naming | Rust and serialized plan use `discovery`/`detail`; both steps retain mandatory `FirstAccepted` | `phase_module_naming` and policy assertions |
| Profile and Source-owned access | Both compile through final names and remain behaviorally distinct | Compiler regressions |
| Source specialization | Landed merge, append order, validation, and policy behavior are unchanged | #168–#172 regression targets |
| Accepted/recovered/exhausted | Discovery and Detail preserve exact output, calls, Diagnostics, and terminal behavior | Final runtime targets |
| Diagnostic compatibility | Schema-v2 paths/codes/messages and deterministic order are unchanged | Focused assertions/snapshots |
| Existing Discovery bound | Renamed budget enforces the same per-Strategy request limit and Diagnostic | Discovery pagination/budget regression |
| Pre-/mid-attempt Cancellation | Same empty output, retained Diagnostics, one cancellation Diagnostic, no later Strategy/exhaustion | Context-aware runtime tests |
| Production callers | Search Run, Source Live Check/activation, lazy Detail, and commands use final APIs with unchanged outcomes | Existing caller tests plus static search |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors schema-v2 fixtures retain behavior | Existing profile regressions |
| Deletion and allowlist | No forbidden internal old name/path or compatibility surface; every residual old spelling is classified as authored schema-v2, Diagnostic data, or unrelated Job Posting vocabulary | Reviewed searches below |
| Runtime boundary | Final operations receive immutable typed plans only | External compiler/runtime test and import review |

Primary behavior tests cross `compile_source`, public Detection, final Discovery/Detail operations, and production caller interfaces. Existing deterministic HTTP/browser implementations remain the external test seams; private tests remain only for narrow phase-local edges.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test phase_module_naming
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
```

Run these ticket-specific deletion/boundary inventories and review every residual hit:

```bash
rg -n '\b[A-Za-z0-9_]*(posting_discovery|posting_detail)[A-Za-z0-9_]*\b|\bPosting(Discovery|Detail)[A-Za-z0-9_]*\b|\bexecute_posting_(discovery|detail)[A-Za-z0-9_]*\b' \
  src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '(^|::)posting_(discovery|detail)(::|;|\b)|\bmod\s+posting_(discovery|detail)\b|\.(posting_discovery|posting_detail)\b|\bpub\s+(posting_discovery|posting_detail)\s*:' \
  src-tauri/src src-tauri/tests --glob '*.rs'
find src-tauri/src src-tauri/tests \
  \( -path 'src-tauri/tests/fixtures' -o -path 'src-tauri/tests/fixtures/*' \) -prune -o \
  \( -name '*posting_discovery*' -o -name '*posting_detail*' \) -print
rg -n '\b((new|old|legacy|compat(ibility)?)_(posting_)?(detection|discovery|detail)[A-Za-z0-9_]*|(posting_)?(detection|discovery|detail)_(v[23]|legacy|compat(ibility)?|old|new)[A-Za-z0-9_]*|(Detection|Discovery|Detail)(V[23]|Legacy|Compat(ibility)?|Old|New)[A-Za-z0-9_]*)\b' \
  src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\bdetect\b' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '\bdetect\b|postingDiscovery|postingDetail|posting_discovery|posting_detail' \
  src-tauri/src/schema src-tauri/resources src-tauri/tests/fixtures
find src-tauri/tests/fixtures -type f -print | \
  rg '\bdetect\b|posting[-_]?discovery|posting[-_]?detail|postingDiscovery|postingDetail' || true
rg -n '"(detect|postingDiscovery|postingDetail)"|"[^"\n]*(postingDiscovery|postingDetail|posting_discovery|posting_detail)[^"\n]*"' \
  src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'execute_(discovery|detail)|Discovery(Fetcher|ExecutionResult|Candidate|ExecutionBudget)|Detail(Fetcher|ExecutionResult|PostingOccurrence)|SourceExecutionPlan' \
  src-tauri/src/lib.rs src-tauri/src src-tauri/tests
```

## Ticket-specific migration items

- [ ] Inspect #172's landed compiler result, fragments, `FirstAccepted`, phase modules, tests, and production callers before renaming.
- [ ] Move `documents/posting_discovery.rs` and `documents/posting_detail.rs`, execution-plan modules, runtime modules/private directories, and external runtime targets to their exact final paths; delete every old path.
- [ ] Rename all document, compiled-plan, runtime, fetcher, budget, compiler-context, helper, caller, fake, and export identifiers; add no alias, wrapper, bridge, or duplicate implementation.
- [ ] Preserve old spellings only through explicit schema-v2 authored serialization and existing observable Diagnostic values; classify every residual search hit.
- [ ] Ensure compiled-plan serialization uses only `discovery`/`detail` and retains mandatory typed `FirstAccepted`.
- [ ] Migrate Search Run, Source Live Check/activation, lazy Detail, app commands, and any landed provenance code directly to final APIs.
- [ ] Add `phase_module_naming`; retain equivalent external fallback, bounds, Cancellation, Detection, caller, and profile coverage under final names.
- [ ] Confirm runtime still receives only immutable typed plans and no T7/T8-or-later behavior entered this slice.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
