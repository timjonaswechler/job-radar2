# T4b — Fingerprint canonical schema-v3 Source behavior for Source Live Check freshness

## Result

A Source Live Check Report remains fresh exactly while the canonical schema-v3 execution behavior that was checked remains unchanged. Granular SHA-256 fingerprints cover the applicable Base Profile, direct Source specialization, Effective Source Profile, execution-relevant compiler provenance, Source Config, complete selected Access Path identity, actually required Source runtime bindings, compiler/runtime versions, and the approved immutable globals. Unreferenced lifecycle/descriptive metadata is excluded.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#171](https://github.com/timjonaswechler/job-radar2/issues/171) and [#174](https://github.com/timjonaswechler/job-radar2/issues/174).
- Blocking: none.
- Readiness: **Blocked — not ready for agent execution**; both direct blockers are open and the issue has no `ready-for-agent` label.
- Open decision: none. T8 is not a blocker. Re-baseline exact names, paths, rejected-check persistence, immutable-limit owners, and tests after #171/#174 land.

## Consumed contracts

- #166 / PRD Decisions 12–23 and 33–37: canonical schema-v3 direct specialization, one immutable compiled result, distinct Profile/Source-owned branches, and no pre-v3 fingerprint compatibility.
- #171 provides `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)`, one successful `CompiledSource` with access, immutable Execution Plan, canonical Effective Source Profile where applicable, and deterministic typed `CompiledSourceProvenance`; Source-owned compilation fabricates no Profile, and rejection exposes Diagnostics only.
- #174 establishes the only active authored model as schema version 3 with `detection`, `discovery`, and `detail`, direct typed Source specialization, and no schema-v2 parser, alias, runtime, fixture pair, or migration.
- T4b, not #171, adds compiler-owned typed runtime-binding dependencies to the exact landed successful compiler result. Rejected output remains diagnostics-only.
- Shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules follow `handoff/issue-166-delivery.md`.

## Current gap

The current pre-blocker repository still uses `ProfileCompilerSnapshot` and `compile_source_execution_plan`. `src-tauri/src/checks/source_live/mod.rs` independently hashes raw `source_document`, `source_profile_document`, `source_config`, and optional `source_overrides`, plus a redundant `live_check_logic` digest of `SOURCE_LIVE_CHECK_LOGIC_VERSION`. Report construction derives validation, fingerprints, and execution through separate paths. `src-tauri/src/checks/source_live/activation.rs` reloads the status-mutated Source and recomputes fingerprints after successful activation/reactivation.

`src-tauri/src/checks/{fingerprints,report,freshness}.rs` already define strict camel-case `CheckFingerprint { kind, reference, sha256 }`, Check Report schema v1, SHA-256 storage, and deterministic freshness comparison by `(kind, reference)` plus `logicVersion`; `checks/persistence.rs` overwrites `source-live-checks/<source_key>.json`. Current global facts are the per-Strategy Source Live Check pagination budget `1`, compiler fallback maximum `50`, and forbidden-request-key behavior in `profile_dsl/compiler/{boundedness,security}.rs`. `source_profile/registry/snapshot.rs` supplies typed registry data.

`src-tauri/tests/{check_reports,source_live_check}.rs` assert the existing report/freshness/persistence contract and obsolete raw fingerprint behavior. Tauri commands and frontend calls in `src-tauri/src/app/commands.rs`, `src-tauri/src/lib.rs`, `src/lib/api/sources.ts`, and `src/features/sources/registry/source/source-live-check-section.tsx` are public-contract regression surfaces. This section is provisional until the blockers land.

## Target delta

### Retained report contract and single preparation flow

Retain `CheckFingerprint`, SHA-256 lowercase 64-hex output, Check Report schema version 1 and all fields, `(kind, reference)` identity, existing missing/changed/unexpected and `LogicVersionChanged` semantics, derived freshness without changing persisted `result`, latest-only persistence, and:

```rust
pub fn evaluate_check_report_freshness(
    report: &CheckReport,
    current_logic_version: impl AsRef<str>,
    current_fingerprints: &[CheckFingerprint],
) -> CheckReportFreshness;
```

Responsibility-level sketch; names may follow landed code:

```rust
pub(crate) struct SourceLiveCheckFingerprintInput<'a> {
    pub source: &'a SourceDocument,
    pub resolved_base_profile: Option<&'a SourceProfileDocument>,
    pub compile_outcome: &'a CompileSourceOutcome,
}

pub(crate) fn source_live_check_fingerprints(
    input: SourceLiveCheckFingerprintInput<'_>,
) -> Result<Vec<CheckFingerprint>, FingerprintPreparationError>;

pub struct CompiledSource {
    // exact #171 fields
    pub runtime_binding_dependencies: SourceRuntimeBindingDependencies,
}

pub struct SourceRuntimeBindingDependencies {
    pub bindings: Vec<SourceRuntimeBinding>, // unique, canonical enum order
}
pub enum SourceRuntimeBinding { Name }
```

For each check/status/activate/reactivate operation, resolve the authoritative Source once and call the public compiler exactly once. Reuse that exact outcome for validation, execution, compiler-owned binding projection, and fingerprints. Fingerprinting must not rerun merge logic, inspect raw template/plan strings, parse Effective Profile/provenance JSON, or introduce an inspect compiler. Successful status-only activation persists the already prepared fingerprints: no post-transition reload, compilation, preparation, or reconstruction.

### Stable components

Each independently serialized component uses `kind = "source_behavior"` and one fixed `reference`:

| Branch | Reference | Canonical material |
|---|---|---|
| Profile | `base_source_profile` | Execution-relevant resolved reusable Base Profile, including executable Source Config Schema and all Access Path/Strategy behavior; excludes metadata listed below. |
| Profile, optional | `direct_source_specialization` | Normalized typed authored fragment, including explicit equal-value terminals and unselected executable additions. Absent and a fragment with no execution terminal normalize to absence; any explicit terminal emits it. |
| Profile success | `effective_source_profile` | Exact canonical Effective Source Profile returned by #171. |
| Both success | `compiler_provenance` | Typed #171 provenance filtered to the execution surface, preserving typed path/origin/canonical order; remove terminal Source Config Schema `title` paths and other excluded metadata only. |
| Both | `source_config` | Complete concrete typed JSON data with every dynamic object map recursively sorted. |
| Both | `selected_access_path` | Authoritative typed selector: `{ branch: "profile_access_path", profileKey, pathKey }` or `{ branch: "source_owned_access_path", key }`. |
| Both success, conditional | `source_runtime_bindings` | Only concrete values required by compiler-owned dependencies in compiled Discovery/Detail behavior; initially `{ name: source.name }` for `source:name`. Absent when unreferenced. |
| Source-owned | `source_owned_access_path` | Full execution-relevant typed Source-owned Access Path; success uses the paired compiled path, rejection may use the independently available authored typed path. |

Source-owned output never contains Base/direct/Effective Profile material or synthetic Profile provenance.

Append this fixed six-entry tail in table order:

| `kind` / `reference` | Exact material and owner |
|---|---|
| `behavior_version` / `profile_compiler` | Hash `profile-compiler/v1`; compiler owners bump it when validation, merge, plan, or provenance semantics change without canonical input change. |
| `behavior_version` / `profile_runtime` | Hash `profile-runtime/v1`; runtime owners bump it when execution semantics/output/acceptance change without plan/material change. |
| `behavior_version` / `immutable_globals` | Hash `immutable-globals/v1`; bump when inventory membership or interpretation changes. |
| `immutable_global_behavior` / `source_live_check_pagination_smoke_budget` | Typed landed per-Strategy smoke request budget, currently `1`. |
| `immutable_global_behavior` / `compiler_max_fallback_strategies` | Typed landed compiler maximum, currently `50`. |
| `immutable_global_behavior` / `security_forbidden_request_key_behavior` | `explicitForbiddenHeaders: [authorization, cookie, proxy-authorization, set-cookie, x-api-key]` lexicographically; `explicitForbiddenHeadersAppliesTo: header_names`; and `secretLikeKeyAppliesTo: [header_names, form_body_field_names, recursively_visited_json_object_keys]` in that fixed order. Never include values. |

The global inventory is closed to those three rows. `is_secret_like_key` algorithm changes bump `profile_compiler`; do not add another algorithm token. Version tokens are hashed, never persisted raw.

`CheckReport.logicVersion` and `SOURCE_LIVE_CHECK_LOGIC_VERSION` remain the sole Source Live Check algorithm partition (candidate acceptance, Detail-smoke choice, pass/fail derivation). Delete `live_check_logic`; compiler/runtime/global versions remain separate fingerprints.

### Canonicality, order, and counts

Use private closed typed projection structs/enums near the compiler/check ownership boundary. Serialize each component independently with `serde_json::to_vec`, then SHA-256 it. Static fields follow projection-type order; canonical keyed Access Paths/Strategies and ordinary arrays retain semantic order. Recursively sort every dynamic object map, including nested Source Config and request/header/body/extraction/schema maps. Preserve null/boolean/number/string distinctions, arrays, empty arrays, and empty objects. Do not introduce public/generic canonical JSON, string concatenation, Unicode/number normalization, or a broad map-walker framework.

Filter compiler provenance directly in memory; never deserialize, re-sort, infer origins, or reconstruct merge semantics. Produce runtime bindings only from typed dependencies emitted during the same template-validation/plan-compilation flow; future bindings require compiler/runtime support and direct fingerprint coverage, not a checks-owned string fallback.

Authoritative success order is:

- Profile: Base; optional direct; Effective; provenance; config; selector; conditional bindings; fixed tail.
- Source-owned: owned path; provenance; config; selector; conditional bindings; fixed tail.

Rejected compilation emits only independently available typed authored inputs in the same relative order, then the fixed tail. Effective/provenance/bindings are absent because no partial `CompiledSource` exists. An unchanged persisted rejection may be fresh; later successful compilation adds identities and is stale. Persist unresolved-Base rejection only if the landed Source Live Check already does so; otherwise preserve its error/no-report behavior. Missing Source and unresolved Base follow landed behavior without a fabricated digest/report. Never fabricate selected effective behavior or a report to meet a count.

| Outcome | Exact total |
|---|---:|
| Profile success: neither / exactly one / both optional direct and conditional bindings | 11 / 12 / 13 |
| Source-owned success: without / with bindings | 10 / 11 |
| Rejected Profile, resolved Base: without / with non-empty direct | 9 / 10 |
| Rejected Source-owned | 9 |
| Rejected Profile, unresolved Base: without / with non-empty direct, only if persisted after blockers | 8 / 9 |

Every `(kind, reference)` is unique; duplication is a preparation error and no ambiguous report is persisted. Preparation failure occurs before persistence and omits nothing silently. Fingerprinting has no Cancellation, Partial Completion, or Source-status effect.

### Inclusion, exclusion, and data minimization

Include executable schema/policy/acceptance/Strategy behavior, Source Config, complete selected branch identity, applicable typed provenance, conditional required Source bindings, and owned versions/globals. Exclude Source schema/key/status/origin/path/activation/persistence; Source `name` except when required by `source:name`; Profile schema/name/kind/origin/path/custom-vs-built-in identity; reusable Profile key from Base/Effective metadata (but not selector); Detection/probes/evidence/captures/proposals; descriptions/labels/support/known issues/notes/catalog metadata; schema `title`; Diagnostics/timestamps/results/details; provider/browser/attempt/candidate/posting/Search/Match/database data; and unproven application constants.

Excluded-only edits stay fresh. Referenced Source-name changes are stale; unreferenced names stay fresh. Profile-key/path selection and branch switches are stale through changed or missing/unexpected identities. `title` add/change/remove stays fresh because projections and terminal provenance paths exclude it.

Persist only existing identity/digest fields. Projection material, secrets, raw version tokens, and values must not enter reports, Diagnostics, SQLite/files, or logs; errors identify stable component identity only. Invalid documents expose Diagnostics/no partial result and cannot bypass security.

## Dependency and deletion decision

Typed Source/Profile/compiler values and registry snapshots are in-process immutable data. Projection, sorting, serialization, hashing, version selection, and inventory are private pure computation, not traits/ports. Existing Check Report filesystem persistence remains local-substitutable. Existing HTTP/browser clients are used by Source Live Check, not fingerprint preparation, and deterministic clients remain the test seam. No new external dependency is introduced.

**Deletion test:** Without one fingerprint-preparation boundary and one compiler outcome per operation, report construction/status/activation/reactivation would each need binding, provenance filtering, selector, projection, exclusion, canonical ordering, version/global, rejection, and Source-owned knowledge. Conversely, generic canonical JSON, aggregate/compatibility wrappers, raw-template scanners, second compilers, and post-transition refreshers hide no unique responsibility and must not exist.

## Examples

1. Base timeout `10000` specialized to `5000` emits distinct Base/direct/Effective/provenance/config/selector digests. Changing it to `6000` changes direct and Effective; unchanged origins leave provenance stable. Explicit equal-value specialization still emits direct authorship.
2. `{{source:name}}` produces compiler dependency `Name`; renaming changes `source_runtime_bindings`. Without that dependency the component is absent and rename remains fresh.
3. Equivalent nested maps with opposite insertion order hash identically; semantic array order remains significant. Status, Detection, support/description, and schema `title` changes remain fresh.
4. Switching behavior-identical `profile_a/api` to `profile_b/api`, or Profile to Source-owned access, is stale because selector/component identities change.
5. A resolved-Base rejected Profile Source emits Base, optional direct, config, selector, and six tail entries (9/10), with no Effective/provenance/bindings. Later success adds identities and is stale.
6. Runtime semantics change only `behavior_version/profile_runtime`; pagination-budget material changes its global digest; candidate acceptance changes only report `logicVersion`.
7. Successful activate/reactivate compiles and prepares once; status mutation persists the checked fingerprints unchanged.

## Scope

- After blockers land, inspect exact compiler/schema-v3/provenance/Source-owned/runtime/security/limit/orchestration/report paths and tests.
- Add closed typed projections, recursive map sorting, provenance filtering, stable identities, exact ordering/counts, duplicate rejection, and independent SHA-256 hashes.
- Extend successful `CompiledSource` with compiler-owned typed runtime-binding dependencies produced by the existing compile flow; preserve diagnostics-only rejection.
- Add exact three behavior-version digests and closed three-global inventory with code-adjacent ownership/bump documentation and tests.
- Migrate check creation, status, activation, and reactivation to the single preparation/compiler outcome; preserve Tauri/frontend APIs and report/persistence results.
- Delete raw Source/Profile/override and redundant logic fingerprints, duplicate preparation/compiler paths, raw scanners, post-transition refresh, and superseded expectations.

## Adjacent non-goals

- Check Report field/schema changes, new persistence tables/history, report rewriting, UI redesign, or automatic rechecking.
- Pre-v3 fingerprints, fixtures, parsers, aliases, migration warnings/tests, or compatibility branches.
- Changes to compiler merge/provenance semantics, Source Config Schema language, runtime/check algorithms, immutable limit values, or forbidden-key policy; this ticket fingerprints landed behavior.
- T8 Strategy Set runtime, new policies/primitives, Detection convergence, Candidate Resolution, Search Run matching/persistence, or Partial Completion.
- Generic canonical JSON, a new/keyed/salted/encrypted hash, secret storage, provider-specific constants/branches, Cancellation changes, or new status variants.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Stable Profile and map order | Repeated equivalent behavior, including differently inserted nested maps, has identical ordered identities/digests and is fresh | `source_live_check_fingerprints` integration/projection test |
| Profile optional components | Neither/either/both direct and bindings yield totals 11/12/13 in authoritative order | External ordered identity/count assertions |
| Source-owned and branch switch | Totals 10/11, no Profile material; branch switch stale | Source Live Check integration |
| Rejection | Resolved Profile 9/10; Source-owned 9; unresolved Profile 8/9 only if persisted; no compiled-only material; correction stale | Integration count/order tests against landed behavior |
| Base/direct/effective/provenance | Execution leaf, equal-value authorship, keyed additions, or retained-origin-only change affects only applicable components; empty fragment normalizes absent | Compiler + Source Live Check integration using typed T4a fixtures |
| Source Config and selector | Nested/scalar config change is stale; behavior-identical Profile/path selection change is stale | Integration tests |
| Runtime binding | Referenced name stale; unreferenced name fresh; no raw template scan | Compiler + Source Live Check integration |
| Excluded metadata | Source lifecycle/identity where addressable, Profile metadata/Detection, and schema `title` add/change/remove remain fresh; terminal title provenance absent | Mutation integration + serialized-material search |
| Versions and globals | Each compiler/runtime/inventory version, each approved global, and security applicability mutation changes only its owned partition | Focused parameterized inventory/version tests |
| Logic version | One `LogicVersionChanged`; no `live_check_logic` identity/detail | `check_reports` + Source Live Check tests |
| Uniqueness/error | Every identity is unique; duplicate/preparation failure persists no ambiguous/partial report | Narrow invariant + integration test |
| Report/persistence | Existing strict schema/fields/result/reasons/SHA-256 and latest-only overwrite remain unchanged; missing/changed/unexpected details retain expected/actual SHA behavior | `check_reports` evaluator, serialization, schema, and persistence tests |
| Cardinality/reuse | Check/activate/reactivate compiles once; successful status transition performs no reload/recompile/reprepare and persists identical fingerprints | Injected/structural counter + activation tests + search |
| Excluded operational data | Diagnostics, prior report result/timestamp/details, and deterministic provider-response changes do not affect current behavior fingerprints | Regression mutation/static assertion |
| Data minimization | Secret-bearing config produces only digest; no raw material/token/title/value in report/error/log | Integration serialization assertion |
| Regressions | Greenhouse, Workday, SuccessFactors remain provider-neutral; no pre-v3 compatibility exists | Existing profile targets + reviewed search |
| Cancellation/Partial Completion | No new input, output, or status variant | Static API review |

Tests cross `compile_source`, Source Live Check report/status and activation operations with real typed schema-v3 documents, real compiler/provenance, temporary persistence, and deterministic clients. Private tests are limited to duplicate identity, deep recursive map ordering, and parameterized immutable material.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check_fingerprints
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test check_reports
cargo test --manifest-path src-tauri/Cargo.toml --test effective_profile_provenance
cargo test --manifest-path src-tauri/Cargo.toml --test schema_v3_authored_hard_cut
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run build

rg -n 'source_document|source_profile_document|source_overrides|live_check_logic' src-tauri/src/checks src-tauri/tests --glob '*.rs'
rg -n 'fingerprint|Fingerprint|serde_json::to_(vec|string|value)|Sha256|sha256' src-tauri/src/{checks,profile_dsl,source,source_profile} src-tauri/tests --glob '*.rs'
rg -n 'MAX_FALLBACK_STRATEGIES|MAX_PAGINATION_REQUESTS|FORBIDDEN_HEADERS|is_secret_like_key' src-tauri/src --glob '*.rs'
rg -n 'schema.?v2|pre.?v3|legacy.*fingerprint|fingerprint.*migration|compat.*fingerprint' src-tauri/src src-tauri/tests --glob '*.{rs,json}'
```

Use exact landed replacement target names and add blocker-landed Source Config Schema, direct-fragment, compiler-authority, activation/reactivation, Discovery, and Detail targets where applicable. The shared full-suite/build requirements remain in the delivery contract.

## Ticket-specific migration items

- [ ] Inventory every fingerprint identity/constructor/destructure, caller, compiler call, constant, and fixture; centralize exactly the approved three globals.
- [ ] Add typed Base/direct/Effective/Source-owned/config/selector/provenance/binding projections with recursive dynamic-map sorting and exact branch ordering/counts.
- [ ] Extend successful `CompiledSource` with unique ordered typed runtime-binding dependencies from the compile flow; keep rejected output partial-result-free.
- [ ] Move report creation/status/activate/reactivate directly to one compiler outcome and preparation boundary; delete post-status reload/recompile/re-fingerprint.
- [ ] Delete `source_document`, `source_profile_document`, `source_overrides`, and `live_check_logic` identities/helpers/fixtures and superseded stale tests; the first search must have no active old identity.
- [ ] Verify every remaining fingerprint/serialization/hash hit and classify historical/negative hits; no raw digest, reconstructed provenance, insertion-order dependence, aggregate compatibility helper, scanner, or second compiler may remain.
- [ ] Verify the final compatibility search finds no pre-v3 implementation or success fixture and that reports/errors contain no raw secrets, projection material, or version tokens.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
