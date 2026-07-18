# T4a — Expose deterministic Effective Source Profile provenance from `compile_source`

## Result

Every successfully compiled Source exposes deterministic field-level provenance for the effective execution/specialization surface returned by the same compiler operation. Profile-based compilation explains the exact `EffectiveSourceProfile`; Source-owned compilation explains the corresponding execution surface of its distinct Source-owned Access Path. Callers do not reconstruct merge semantics or run a second compiler pipeline.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#170](https://github.com/timjonaswechler/job-radar2/issues/170).
- Blocking: [#175](https://github.com/timjonaswechler/job-radar2/issues/175).
- Readiness: **Blocked** pending #170 and the required post-blocker re-baseline/readiness review.
- Open decision: none. Provenance placement, representation, coverage, origin semantics, and ordering are selected below.

## Consumed contracts

- #166 / PRD Decisions 12–22 and the “Effective Profile Compiler” module decision: one authoritative direct Source, one private merge/validation pipeline, an inspectable Effective Source Profile for profile access, a distinct Source-owned branch, and an immutable Execution Plan runtime boundary.
- #168 and #169: recursive object merge, scalar replacement, stable-key Access Path/Strategy merge, inherited order plus deterministic additions, whole replacement of non-keyed arrays, and complete validation of Source-added keyed entries.
- #170: profile- and Access-Path-level Effective Source Config Schema locations remain distinct; `properties` specialize recursively; `required`/`enum` replace whole; new properties are complete; reusable-profile `title` is inherited but cannot be authored by a direct Source fragment or Source-owned Access Path.
- T7 later renames typed field segments to canonical schema-v3 member names in place. T4a introduces no versioned path wrapper or alias. T4b/#175 later owns fingerprints and freshness.

## Current gap

This section is provisional while #170 is open and must be re-baselined against its landed code before implementation. The current repository still exposes `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan(snapshot, source_key)` from `src-tauri/src/profile_dsl/compiler/mod.rs`; it does not yet contain `compile_source`, `EffectiveSourceProfile`, or provenance types.

`compiler/resolution.rs` resolves selected reusable or Source-owned access and builds only an Execution Plan. `compiler/overrides.rs` mutates cloned selected steps without retaining origins. `compiler/source_config.rs` validates current schema objects but does not track effective-schema origin. Structured Diagnostics in `profile_dsl/diagnostics/mod.rs` use JSON Pointer paths and optional `strategyKey`; these index-based diagnostic locations are not a stable provenance identity language.

Source validation, Source Live Check, Search Run selection, and posting preparation consume the current plan-only compiler facade. Source Live Check fingerprints raw Source/Profile/config/override documents, while `checks/report.rs` and `tests/check_reports.rs` persist no provenance. Existing `compiler_resolution`, `compiler_semantic_validation`, `compiler_security_boundedness`, `source_live_check`, and acceptance-profile tests assert plans and Diagnostics but cannot explain field origin.

After #170 lands, these names may be historical. The remaining ticket-specific gap must still be: the one compiler result has no stable, complete explanation of which terminal effective execution/specialization values came from the reusable Base Source Profile, the direct Source fragment, or a Source-owned Access Path.

## Target delta

The landed `compile_source` contract gains one top-level provenance result:

```rust
pub struct CompiledSource {
    pub access: CompiledSourceAccess,
    pub execution_plan: SourceExecutionPlan,
    pub provenance: CompiledSourceProvenance,
}

pub enum CompiledSourceProvenance {
    Profile { entries: Vec<ProvenanceEntry> },
    SourceOwned { entries: Vec<ProvenanceEntry> },
}

pub struct ProvenanceEntry {
    pub path: ProvenancePath,
    pub origin: ProvenanceOrigin,
}

pub struct ProvenancePath {
    pub segments: Vec<ProvenancePathSegment>,
}

pub enum ProvenancePathSegment {
    Field { name: String },
    AccessPath { key: String },
    Strategy { key: String },
    MapKey { key: String },
}

pub enum ProvenanceOrigin {
    BaseSourceProfile,
    DirectSourceFragment,
    SourceOwnedAccessPath,
}
```

Exact derives and module placement may follow landed code. Field placement, variant responsibilities, one typed path language, and the closed three-origin model may not drift. Serialized fields use camelCase; enum tags/values use snake_case; provenance is tagged with `kind: profile|source_owned`; path segments are internally tagged with `kind: field|access_path|strategy|map_key`. Unknown fields/variants are rejected wherever deserialization is part of the landed public contract. No compact string-path encoding is added.

### Ownership and outcome invariants

1. Provenance is recorded during the same private merge/materialization flow used by `compile_source`, not by replay, JSON diff, re-parsing, or a second inspect compiler.
2. The direct Source remains authoritative even if the Registry Snapshot contains a conflicting same-key Source.
3. Compiler-produced values pair `Profile` provenance only with profile access and `SourceOwned` provenance only with Source-owned access. T4a does not redesign the result merely to prevent callers manually constructing an invalid value.
4. `Compiled` contains complete applicable provenance and warning/info Diagnostics only. `Rejected` contains at least one error Diagnostic and no partial Effective Source Profile, Source-owned Access Path, Execution Plan, or provenance.
5. Runtime continues to receive only `SourceExecutionPlan` and never branches on origin. Provenance performs no filesystem, database, HTTP, browser, provider, or runtime access and is not persisted.

### Provenance surface and terminal coverage

Profile provenance covers only the Effective Source Profile execution/specialization surface: profile- and Access-Path-level `sourceConfigSchema`; Access Paths and Strategies including stable `key` fields and the `name` required for a new Source-added Access Path; phase steps; policies; acceptance; and all execution-relevant safe Strategy fields admitted at the landed boundary. Source-owned provenance covers the analogous Source-owned Access Path execution surface.

Excluded are profile identity (`schemaVersion`, profile `key`, `name`, `kind`), Detection configuration/evidence/captures/proposals, descriptions, support/known-issue metadata, authored Diagnostics, and other non-specializable metadata. Analogous non-execution Source-owned metadata is also excluded.

Within the applicable surface, emit exactly one entry for every terminal value:

- each scalar, typed literal `null` where legal, and in-surface key/name value;
- each non-keyed array as one atomic terminal, with no element paths;
- each empty object that is itself an effective terminal;
- each nested leaf under non-empty objects, including dynamic headers, captures, request bodies, posting metadata expressions, extraction fields, and schema `properties`.

Non-empty structural containers receive no aggregate entry. Keyed Access Paths and Strategies are selected structurally by stable-key segments, but their authored `key` member remains a terminal with its own entry.

### Origin and schema semantics

- `BaseSourceProfile`: inherited terminal value not supplied by the direct Source fragment at that effective location.
- `DirectSourceFragment`: terminal explicitly supplied by the direct fragment, including equal-value replacement, a newly completed nested value, complete new Strategy/Access Path, or whole-array replacement.
- `SourceOwnedAccessPath`: every in-surface terminal belonging to the Source-owned Access Path; there is no base merge.

Mentioning an inherited Access Path/Strategy key only to locate a fragment does not change that effective `key` field’s base origin. Every terminal of a complete Source-added keyed entry, including key and admitted Access Path name, is direct-Source origin. An empty fragment object that changes no terminal creates no Source-origin entry. Compiler-derived plan fields, excluded metadata, concrete Source Config values, and the private composed Source Config validation contract receive no synthetic provenance.

Schema provenance follows #170 exactly. Existing property schemas may mix origins leaf by leaf; a complete new property is direct-Source throughout; `required` and `enum` each have one whole-array origin; executable scalar keywords such as `type`, `pattern`, `format`, `minimum`, and applicable `additionalProperties` use their supplying origin. Reusable-profile `title` is always base origin. Forbidden direct/Source-owned `title` authoring rejects compilation and yields no provenance.

### Stable paths, ordering, and invariant failure

`ProvenancePath` is the only provenance identity and is distinct from Structured Diagnostic JSON Pointers. `Field` selects a static member, `AccessPath` and `Strategy` select keyed entries, and `MapKey` selects a dynamic member. Profile paths begin with `Field("accessPaths"), AccessPath(key)` or `Field("sourceConfigSchema")`. Source-owned paths begin with `AccessPath(source_owned_key)`. A keyed entry’s terminal key appends `Field("key")`.

Entries use one canonical depth-first traversal of the included typed surface. Static member order follows the canonical typed-document field order protected by serialization fixtures. Access Paths/Strategies follow landed deterministic effective semantic order while identities remain key-based. Dynamic object keys are lexicographically ordered at every level. Non-keyed arrays are not traversed.

Semantically equivalent inputs differing only in JSON map insertion order produce equal effective values, plans, typed provenance, and byte-for-byte serialized provenance. Every valid terminal path occurs exactly once. Duplicate or missing coverage rejects compilation with one error-severity `compiler/compiled_provenance_invariant_violation` Diagnostic at the empty document path; `details.reason` is `duplicate_path` or `missing_path`, and `details.provenancePath` contains only the typed identity path. Diagnostics follow canonical provenance traversal. Excluded paths neither appear nor trigger missing-path failure.

### Data minimization and later hard cut

Provenance retains only static field names, stable Access Path/Strategy keys, dynamic member-name identity tokens, and origin enums. It stores no explained terminal payload, Source Config value, credential/cookie/authorization data, response bytes, parsed/provider/runtime/posting data, timestamp, fingerprint, Check result, persistence identifier, or Search Request data. Security-invalid inputs expose no partial provenance.

T7 renames affected `Field` values with their authored/effective members (`postingDiscovery` to `discovery`, `postingDetail` to `detail`) and deletes old values in the same hard cut. T4a adds no `v2`, `v3`, `legacy`, compatibility, alias, conversion, or parallel path representation. Cancellation, runtime completion, budgets, Search Run counts, and statuses are not applicable to this pure compiler result.

## Dependency and deletion decision

Compiler inputs, effective definitions, merge decisions, path construction, origin assignment, coverage validation, canonical ordering, and Diagnostics are in-process typed data/computation. The immutable Registry Snapshot remains input data, not a port. A private recorder behind `compile_source` is justified; no trait/callback or external seam is.

**Deletion test:** Without compiler-owned provenance, inspection/diagnostic consumers, T4b preparation, and compiler tests would each reconstruct recursive merge origin, keyed identity, whole-array replacement, Source-added completeness, schema origin, and the Source-owned distinction. Source validation and Source Live Check may ignore provenance in T4a, but must not become reconstruction sites.

## Examples

1. **Mixed object:** if a fragment replaces `fetch.headers.accept` and `fetch.timeoutMs`, those leaves are `DirectSourceFragment`; inherited `mode`, `url`, and `headers.user-agent` remain `BaseSourceProfile`. Existing Access Path/Strategy key terminals remain base.
2. **Whole array:** replacing `transforms` emits one direct-Source entry at the array field and no `transforms/0/...` entries. Omitting it emits one base-origin array entry.
3. **Schema specialization:** inherited `feedUrl.type`, `format`, and `title` remain base; replacing `region.enum` and root `required` emits whole-array direct entries; a new `pageSize` property’s `type` and `minimum` are direct.
4. **Source-owned:** provenance is `SourceOwned`, every included terminal origin is `SourceOwnedAccessPath`, paths begin with `AccessPath(key)`, and no synthetic Effective Source Profile/root member exists.
5. **Rejection:** an invalid timeout, unsafe header, forbidden title, duplicate path, or missing terminal coverage returns Diagnostics only and no partial compiled material.

## Scope

- Extend the exact #170-landed successful compiler result with top-level matched Profile/Source-owned provenance.
- Add the single typed, serializable path representation and closed origin enum.
- Record origin inside the existing private merge/materialization flow and validate complete unique terminal coverage.
- Cover recursive objects, dynamic maps, empty objects, legal typed nulls, key/name fields, whole non-keyed arrays, existing keyed locators, and complete added Strategies plus selected or unselected Access Paths.
- Implement final #170 schema provenance and deterministic ordering independent of map insertion/hash order.
- Update compiler-facing production callers to accept the extended result without reconstruction; callers may ignore provenance.
- Add external compiler integration coverage and stable serde fixtures. A private fault-injection test is permitted only for duplicate/missing recorder states unreachable through valid typed inputs.
- Delete any temporary placeholder, replay/diff helper, inspect compiler, duplicate recorder, conversion wrapper, alias, or superseded implementation-detail test.
- Keep Structured Diagnostic, runtime, Check Report persistence, and Source Live Check freshness shapes unchanged.

## Adjacent non-goals

- Fingerprints, freshness transitions, canonical schema-v3 hashing, or runtime-binding dependencies: T4b/#175.
- Schema-v3 phase/member renaming: T7/#174.
- UI/Tauri provenance presentation, authoring UI, persisted provenance/audit/history, or promotion suggestions.
- Changing #170’s Source Config Schema language, composition, diagnostics, or `title` policy.
- Detection proposal provenance, runtime attempt provenance, phase-output contribution provenance, or concrete Source Config provenance.
- New Primitives, Strategy Policies, runtime budgets/limits, placement/reordering, deletion/disabling, Candidate Resolution, or provider-specific behavior.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| No fragment | Every included Effective Profile terminal appears once with base origin; excluded metadata is absent | External provenance test |
| Scalar/equal-value specialization | Explicit terminal has direct origin even when value equals base; unrelated leaves remain base | External provenance test |
| Recursive object/dynamic maps | Mixed origins are leaf-specific; `MapKey` identities and lexicographic order are stable | External determinism test |
| Inherited/replaced non-keyed array | One array entry with base/direct origin and no index entries | External tests covering representative `transforms`, `conditions`, `required`, `enum`, `waits`, and `interactions` arrays |
| Existing keyed locator | Locator key terminal remains base; changed descendants are direct; paths use stable keys | External provenance test |
| New Strategy/Access Path | Every included terminal, including key/name, is direct origin; both selected and unselected added Access Paths are covered | External provenance test |
| Keyed fragment mention order | Reordering mentions of inherited keys cannot reorder them; complete additions retain the landed deterministic append order and key-based identity | External compiler determinism test |
| Effective schema | Mixed properties, new properties, whole `required`/`enum`, and inherited `title` follow #170 | External provenance/schema test |
| Forbidden title | Direct/Source-owned attempt rejects; no compiled provenance | Retained #170 tests plus compiler assertion |
| Source-owned access | Matched Source-owned variants, all included origins Source-owned, no Effective Profile | External compiler/API test |
| Profile/provenance pairing | `compile_source` always returns matching variants; both serialized shapes are stable | External API/serde fixture |
| Map insertion order | Effective value, plan, typed entries, and serialized provenance are identical | External determinism test |
| Duplicate/missing terminal | Exact invariant Diagnostic; no partial result | Narrow fault-injection plus external completeness assertion |
| Null/empty object/empty fragment | Legal null and effective empty object each get one origin; no-op fragment invents none | External provenance tests |
| Invalid effective behavior | Existing semantic/security/schema Diagnostic; no profile/plan/provenance | Existing compiler tests extended with absence assertion |
| Direct Source authority | Conflicting snapshot Source cannot alter result or provenance | Existing authority fixture extended |
| Data minimization | Serialization contains only path identities/origins and no terminal/config/runtime/fingerprint data | Serialization assertion/static review |
| Diagnostics and Live Check | Diagnostic JSON Pointers and Check Report persistence/freshness remain unchanged; no reconstruction | Regression tests plus repository search |
| Runtime and T7 readiness | Runtime imports plans only; one unversioned typed path with no aliases/dual encoding | Call-graph/static search |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors plans remain unchanged and provenance is provider-neutral | Existing profile regressions |

Primary tests cross `compile_source(&source, &registry)` and inspect the returned effective/Source-owned definition, immutable plan, ordered provenance, and Diagnostics. They use real typed documents/compiler logic and an in-memory immutable Registry Snapshot; no network or adapter is involved.

### Focused commands

Use exact landed target names after readiness re-baselining:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test effective_profile_provenance
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test check_reports
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
rg -n 'inspect_source|compile_for_inspection|CompiledSourceProvenance|ProvenancePath|provenance' src-tauri/src src-tauri/tests
rg -n 'v2|v3|legacy|compat|postingDiscovery|postingDetail' src-tauri/src/profile_dsl src-tauri/tests --glob '*.{rs,json}'
```

## Ticket-specific migration items

- [ ] Re-inspect #170-landed compiler/result, fragments, merge/schema validation, Source-owned branch, Diagnostics, callers, and exact test targets.
- [ ] Add `CompiledSource.provenance`, matched variants, one typed path contract, and the closed origin enum; update all constructors/destructures directly.
- [ ] Integrate one origin recorder into the existing private materialization flow; prove exact terminal uniqueness/completeness and canonical traversal.
- [ ] Cover equal replacements, keyed locators/additions, recursive maps, arrays, nulls, empty objects/no-op fragments, schema specialization/title, Source-owned roots, authority, rejection, and data minimization.
- [ ] Delete provenance placeholders, post-hoc diffs, replay mergers, inspect compilers, duplicate recorders, public stages, aliases/converters, and superseded private tests.
- [ ] Verify no runtime/Check Report persistence or Diagnostic path change, no index-based/string provenance path, and no v2/v3 compatibility representation remains.
- [ ] Classify every hit from the focused searches; active provenance construction must have one compiler owner and every non-owner caller must only consume or ignore the result.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
