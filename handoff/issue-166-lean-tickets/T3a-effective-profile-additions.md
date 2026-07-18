# T3a — Compile complete Source-added Strategies and Access Paths

## Result

A Source selecting a reusable Source Profile can add complete Strategies, Detail steps, and Access Paths through the direct typed Source fragment. New keyed entries append deterministically after inherited entries, and a newly added Access Path can be selected only after the complete Effective Source Profile and existing Source Config contract have passed validation.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#168](https://github.com/timjonaswechler/job-radar2/issues/168).
- Blocking: [#170](https://github.com/timjonaswechler/job-radar2/issues/170).
- Readiness: **Blocked**; #168 remains open, so this ticket must be re-baselined against the landed #167/#168 compiler, fragment, merge, Diagnostic, and test names before assignment.
- Open decision: none. If implementation requires a new global ceiling rather than applying the existing security and boundedness contract, stop for an explicit decision.

## Consumed contracts

- #166 / PRD Decisions 12–21 and 38: typed direct Source fragments, stable-key merge, append-only additions, rejection of `null`/deletion/disabling/reordering, and complete Effective Source Profile validation before Source Config validation and Access Path resolution.
- #166 / PRD “Effective Profile Compiler” module decision: the direct Source is authoritative, the Registry Snapshot is immutable input data, and callers receive one Effective Source Profile plus immutable Execution Plan for profile-based access while Source-owned access remains distinct.
- #168 provides recursive merge for inherited keyed entries, base-order preservation, whole replacement of non-keyed arrays, deterministic duplicate handling, and canonical Set/Map Diagnostic traversal.
- T3b/#170 owns all Source Config Schema specialization, the constrained effective schema contract, shared Compiler/Detection validation, Diagnostic-category changes for concrete Source Config, and Built-in schema migration.
- `handoff/issue-166-delivery.md` supplies the shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

## Current gap

The repository is still at the pre-#167/#168 baseline:

- `src-tauri/src/profile_dsl/compiler/mod.rs` exposes `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan(snapshot, source_key)`; `compiler/resolution.rs` selects a Source from the snapshot, performs lifecycle admission, resolves the selected Access Path before specialization, and builds the plan directly.
- `compiler/overrides.rs`, `profile_dsl/documents/overrides.rs`, `source/documents.rs`, `schema/profile-dsl/overrides.schema.json`, and `schema/source.schema.json` implement `sourceOverrides.strategyOverrides`. Unknown Strategies are rejected, and neither a new Strategy nor a new Access Path can be materialized or selected.
- Complete authored shapes currently live in `documents/access_path.rs`, `posting_discovery.rs`, and `posting_detail.rs`; duplicate-key, boundedness, security, and typed-plan behavior live under `compiler/{keys,boundedness,security}.rs` and `execution_plan/`.
- Current compilation primarily validates executable behavior for the selected path. Relevant coverage is in `compiler_resolution`, `compiler_semantic_validation`, `compiler_security_boundedness`, `schema_validation`, `source_profile_registry`, document Serde tests, and the Greenhouse, Workday, and SuccessFactors regressions.

After #168 lands, its exact replacement paths and names become authoritative. The remaining gap must be only its deliberate unknown/new-key rejection: T3a replaces that rejection with complete additions and consequent whole-profile validation and selection behavior; it does not recreate the compiler or fragment model.

## Target delta

The public compiler interface and outcome responsibilities landed by #167/#168 remain unchanged. Exact Rust syntax follows the landed baseline:

```rust
pub fn compile_source(
    source: &SourceDocument,
    registry: &SourceProfileRegistrySnapshot,
) -> CompileSourceOutcome;
```

### Complete additions

1. A fragment key found at the corresponding base Access Path or Strategy collection follows #168 inherited-entry merge semantics. A key absent there denotes a new entry rather than an unknown-key error.
2. A new entry inherits nothing and must become complete:
   - Access Path: `key`, `name`, and complete `postingDiscovery`;
   - Discovery Strategy: `key`, `fetch`, `parse`, `select`, and complete Discovery `extract`;
   - Detail Strategy: `key`, `fetch`, `parse`, `select`, and complete Detail `extract`;
   - Detail step added where none existed: a complete step with at least one complete Detail Strategy.
3. Missing values are never borrowed from another path, phase, Strategy, selected path, or fixture.
4. `name` may complete a new Access Path only. It remains invalid on an inherited Access Path and cannot rename it.
5. `sourceConfigSchema` remains absent from every direct fragment in T3a. A wholly new Access Path may use only keys from the existing profile-level Source Config Schema and cannot borrow another path's keys or introduce path-specific requirements. A new Strategy under an inherited Access Path may use only keys admitted by that inherited profile/path contract. Existing template validation remains authoritative.
6. Profile identity, `detection`, descriptions/support/known-issue metadata, authored diagnostics, Search Request criteria, and lifecycle/persistence fields remain outside direct fragments.
7. Structural `null`, deletion, disabling, keyed-array whole replacement, and authored placement/reordering remain invalid.

### Append-only order

- Existing Access Paths retain base order; new Access Paths append in first Source-fragment order.
- Existing Strategies retain base order within each phase and Access Path; new Strategies append in first Source-fragment order.
- Interleaving inherited and new keys cannot move inherited entries.
- A wholly new Access Path preserves authored Strategy order within each complete phase.
- Effective Source Profile and selected Execution Plan expose the same applicable Strategy order.

### Completeness and duplicate Diagnostics

Complete Effective Source Profile validation emits:

- one `incomplete_source_added_access_path` per new Access Path missing one or more direct required members;
- one `incomplete_source_added_step` per newly introduced phase object missing `strategies`;
- one `incomplete_source_added_strategy` per new Strategy missing one or more direct required members.

Each Diagnostic points to the concrete effective entry or phase object because a missing child has no pointer. `details` contains `accessPathKey`, `step`, and `strategyKey` where applicable; Strategy Diagnostics also set top-level `strategyKey`. `missingFields` contains lexicographically sorted authored field names. Diagnostics are per incomplete entry, not per missing field. An absent parent emits only its parent Diagnostic; present nested entries may emit their own.

Ordering is Effective Access Path order, then Access Path entry, Discovery step and Strategies, then Detail step and Strategies. Duplicate entries preserve #168 cardinality: each occurrence after the first emits exactly one duplicate Diagnostic in authored-array order with its real Source-fragment pointer and stable key details.

### Validation and selection gate

The private sequence is:

```text
base Source Profile resolution
  → direct Source fragment merge, including append-only additions
  → complete Effective Source Profile validation
  → existing Source Config validation
  → selected Access Path resolution
  → immutable Execution Plan compilation
```

Every effective Access Path and Strategy, including unselected additions, passes duplicate/completeness, semantic, capability, template, strict-plan construction, boundedness, and security validation. An error gates every later stage. An invalid unselected addition therefore rejects the complete Source before selection or plan construction for an otherwise valid selected path.

`Compiled` contains no error Diagnostic. `Rejected` contains no `CompiledSource`, Effective Source Profile, selected Access Path, or partial Execution Plan. Runtime receives only the immutable plan and cannot inspect fragments or addition provenance.

Existing backend security and boundedness checks apply equally to inherited and added entries. Source fragments cannot author, disable, or raise a backend-owned ceiling; Strategy-local values may only tighten execution under existing caller/runtime controls. T3a adds no ceiling dimension/value, Strategy Policy, cumulative phase budget, Cancellation input, Partial Completion, Candidate Resolution count, or status variant.

## Dependency and deletion decision

Documents, inherited/new classification, completeness, append order, duplicate handling, validators, and plan construction are in-process and use their real implementations. `SourceProfileRegistrySnapshot` is immutable input data, not a port. Registry/file loading, SQLite, HTTP/browser execution, and runtime budget/Cancellation remain outside this compiler operation. No new trait or adapter is justified.

**Deletion test:** Without the Effective Profile Compiler boundary, Source validation, Source Live Check, Search Run preparation, posting-detail preparation, and tests would each reconstruct inherited-versus-new classification, completeness, append order, whole-profile validation gates, security/boundedness checks, selected-path timing, Diagnostics, and plan construction.

## Examples

1. **New Strategy:** base Discovery order `[primary, fallback]` plus a complete fragment Strategy `tenant_feed` produces `[primary, fallback, tenant_feed]` in both Effective Source Profile and selected plan.
2. **New selected path:** a complete `tenant_api` Access Path with `name` and at least one complete Discovery Strategy can be selected only after whole-profile and existing Source Config validation. A `sourceConfigSchema` field in that fragment is rejected.
3. **Incomplete Strategy:** a new `tenant_feed` containing only `key` and `fetch` emits one `incomplete_source_added_strategy` at its effective Strategy pointer with `missingFields: ["extract", "parse", "select"]`, stable key details, and no partial result.
4. **Invalid unselected addition:** an unselected added path with a forbidden `authorization` header rejects compilation before a different valid selected path is resolved or compiled.

## Scope

- Extend the one typed direct-fragment model and Source-schema `$def` landed by #168; add no parallel addition document.
- Classify keyed entries against their corresponding base collection and admit complete new Discovery/Detail Strategies, Detail steps, and Access Paths.
- Admit `name` contextually for new Access Paths only; keep `sourceConfigSchema` and T3b behavior out.
- Preserve inherited order, append additions deterministically, and add the exact completeness Diagnostic contract.
- Replace only #168's unknown/new-key rejection while preserving duplicate, `null`, deletion, disabling, whole-replacement, and reordering rejection.
- Validate every effective entry through the real validators before Source Config validation and path selection; allow selection of a complete added Access Path only after those gates.
- Keep all production callers on the single `compile_source` entry point and Source-owned access explicitly distinct.
- Add external compiler ordering/validation tests plus Source-schema and Serde fixtures; retain affected generic profile regressions.
- Delete the superseded unknown-key branch, duplicate addition implementations, pass-through stages, and superseded implementation-detail tests after interface coverage exists.

## Adjacent non-goals

- Effective Source Config Schema specialization or validator consolidation: T3b/#170.
- Effective-profile provenance or fingerprints: T4a/#171 and T4b/#175.
- Strategy Policies or cumulative Strategy Set budgets: T5/#172 and later runtime tickets.
- Schema-v3 phase renaming/hard cut: T7/#174.
- Detection specialization, Source Proposal behavior, or Built-in profile schema migration.
- Placement/reordering, deletion, disabling, structural-null semantics, or keyed-array whole replacement.
- New Primitives, provider-specific Rust, compatibility facades, parallel execution, resumability, promotion, persistence, or lifecycle behavior.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| New Discovery Strategy | `Compiled`; appended after inherited Strategies in effective profile and selected plan | External compiler test |
| New Detail Strategy | `Compiled`; appended after inherited Detail Strategies | External compiler test |
| New Detail step | Complete added step appears in effective path and plan | External compiler test |
| New unselected Access Path | `Compiled`; path follows all inherited paths | External compiler test |
| Select added Access Path | Whole profile and existing Source Config validate before selection | External ordering test |
| Mixed inherited/new order | Inherited order is unchanged; additions follow first fragment order | External compiler test |
| Incomplete Strategy | One fixed Strategy Diagnostic with sorted fields; no partial output | External compiler test |
| Incomplete path or step | Per-entry Diagnostics in required parent/Discovery/Detail order | External ordering test |
| Duplicate new key | #168 cardinality, authored pointer, and stable details remain | External compiler test |
| Invalid unselected addition | Unsafe, incompatible, or unbounded entry rejects before selection | Semantic/security/boundedness tests |
| `sourceConfigSchema` attempt | Source schema and Serde reject root/path authoring | Schema/Serde parity fixtures |
| Fragment schema/Serde parity | Complete and admitted partial addition fragments deserialize; forbidden identity, Detection, metadata, Search Request, persistence, control, `null`, deletion, and disabling shapes are rejected | Positive/negative schema/Serde parity fixtures |
| New path Source Config key | A wholly new Access Path may use an existing profile-level key but cannot borrow another path's key or introduce a path-specific requirement | External semantic tests |
| Inherited-path Strategy key | A new Strategy under an inherited Access Path may use only its existing composed profile/path contract; any other key rejects deterministically | External semantic test |
| Strategy maximum | One-over-landed maximum rejects before selection | Boundedness test |
| Security prohibition | Forbidden request/browser behavior rejects before selection | Security test |
| Direct Source authority | Conflicting same-key snapshot Source has no effect | Retained #167 regression |
| Source-owned access | Distinct compiled branch; no Effective Source Profile fabricated | Compiler regression |
| Cancellation/status boundary | No new Cancellation, Partial Completion, count, or status type | Static review |
| Acceptance profiles | No-fragment Greenhouse, Workday, and SuccessFactors behavior is unchanged | Existing deterministic targets |
| Runtime boundary | Only immutable typed plan reaches runtime | Import/call-graph search |

Primary behavior tests cross `compile_source(&source, &registry)` with real fragment merge, validators, and plan builder and inspect the outcome, effective order, plan order, and ordered Diagnostics. Schema/Serde tests cross the real Source document boundary. No network test is required.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml profile_dsl::documents::serde_tests
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
```

Also run the dedicated Effective Profile Compiler target landed by #167/#168. The shared delivery contract owns the full Rust regression command.

## Ticket-specific migration items

- [ ] Inspect and extend the exact #167/#168 compiler outcome, fragment types/schema, keyed merger, Diagnostics, and external tests.
- [ ] Replace the landed unknown/new-key rejection with one inherited-versus-new classification and complete-addition path.
- [ ] Add exact completeness Diagnostics and regressions for append order, duplicates, invalid unselected additions, Source Config references, security, and boundedness.
- [ ] Keep `sourceConfigSchema` unrepresentable in direct fragments and preserve existing Source Config validation behavior.
- [ ] Keep Source-owned access distinct and every production caller on `compile_source`.
- [ ] Delete any T2-only unknown-key branch, duplicate addition model/path, public pass-through stage, or superseded private test.
- [ ] Confirm blockers' old `sourceOverrides`, `ProfileCompilerSnapshot`, and key-based facade are not restored.
- [ ] Confirm runtime imports only immutable typed plans and lifecycle admission remains outside the compiler.
- [ ] Classify every remaining hit from searches for the landed unknown-key Diagnostic/code and for:

```bash
rg -n '\b(ProfileCompilerSnapshot|CompileSourceExecutionPlanResult|compile_source_execution_plan)\b|sourceOverrides|strategyOverrides' src-tauri/src src-tauri/tests
rg -n 'sourceConfigSchema' src-tauri/src/schema/source.schema.json src-tauri/src/profile_dsl src-tauri/src/source src-tauri/tests
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
