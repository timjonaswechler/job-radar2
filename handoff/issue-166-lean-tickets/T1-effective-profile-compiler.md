# T1 — Compile one direct Source scalar into an Effective Source Profile

## Result

`compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)` becomes the single compiler entry point for a concrete authoritative Source. For a profile-based Source it materializes an inspectable `EffectiveSourceProfile`, applies one typed direct Source scalar to an existing keyed Strategy, validates the complete result, and produces the current immutable Execution Plan behavior.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: none.
- Blocking: [#168](https://github.com/timjonaswechler/job-radar2/issues/168).
- Readiness: **Ready for agent execution**; GitHub currently carries `ready-for-agent`.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 12–21: direct typed Source fragments, deterministic merge/validation order, and immutable compiled plans.
- #166 / PRD “Effective Profile Compiler” module decision: the directly supplied Source is authoritative; the Registry Snapshot is immutable input data, not a port.
- `handoff/issue-166-delivery.md`: shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.
- T2 owns general recursive merge; T3a owns complete new keyed Strategies/Access Paths. Neither is part of this slice.

## Current gap

The repository still uses a key-based compiler facade:

- `src-tauri/src/profile_dsl/compiler/mod.rs` exposes `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan(snapshot, source_key)`;
- `compiler/resolution.rs` selects a Source from the compiler snapshot, resolves the selected Access Path before specialization, applies `sourceOverrides`, and builds a plan directly;
- `compiler/overrides.rs`, `profile_dsl/documents/overrides.rs`, `schema/profile-dsl/overrides.schema.json`, `source/documents.rs`, and `schema/source.schema.json` implement the narrow wrapper-based override model;
- `compiler/source_config.rs` validates against base profile/path schemas rather than an Effective Source Profile;
- Source validation, Source Live Check, Search Run selection, lazy posting Detail, registry loading, and external tests construct or consume `ProfileCompilerSnapshot`.

`SourceProfileRegistrySnapshot` already exists in `source_profile/registry/snapshot.rs`, and `SourceExecutionPlan` already forms the typed runtime boundary. Existing behavior is primarily covered by `compiler_resolution`, `compiler_semantic_validation`, `compiler_security_boundedness`, `schema_validation`, `source_profile_registry`, and the three built-in profile regression targets.

## Target delta

```rust
pub fn compile_source(
    source: &SourceDocument,
    registry: &SourceProfileRegistrySnapshot,
) -> CompileSourceOutcome;

pub enum CompileSourceOutcome {
    Compiled {
        source: CompiledSource,
        diagnostics: Diagnostics, // warning/info only
    },
    Rejected {
        diagnostics: Diagnostics, // at least one error
    },
}

pub struct CompiledSource {
    pub access: CompiledSourceAccess,
    pub execution_plan: SourceExecutionPlan,
}

pub enum CompiledSourceAccess {
    Profile { effective_profile: EffectiveSourceProfile },
    SourceOwned { access_path: SourceOwnedAccessPath },
}
```

Ticket-specific invariants:

1. The direct `SourceDocument` argument is authoritative. A conflicting same-key Source in `registry.sources` cannot affect compilation.
2. Profile lookup may use the immutable Registry Snapshot; Source lookup may not. The snapshot remains data and gains no compiler port/trait.
3. Source lifecycle admission moves outside the compiler. Draft, active, and disabled Sources with identical execution data compile identically.
4. A profile-based Source exposes `EffectiveSourceProfile` as `effective_profile`. Source-owned access remains explicitly distinct and must not receive a fake Effective Source Profile or naming-only wrapper.
5. The private compiler sequence is: base-profile resolution → typed fragment merge → complete Effective Source Profile validation → Source Config validation → selected Access Path resolution → immutable plan compilation.
6. `Compiled` contains no error Diagnostic. `Rejected` contains no partial plan or partial Effective Source Profile.
7. Runtime continues to receive only `SourceExecutionPlan`; it cannot inspect authored fragments.
8. No public stage functions for merge, validation, or path resolution are introduced.

### Admitted fragment slice

T1 introduces dedicated typed fragment documents and a matching Source-schema `$def` for exactly one nested scalar replacement on an existing Access Path and existing Strategy, for example:

```json
{
  "accessPaths": [
    {
      "key": "api",
      "postingDiscovery": {
        "strategies": [
          {
            "key": "list_jobs",
            "fetch": { "timeoutMs": 5000 }
          }
        ]
      }
    }
  ]
}
```

Given a base timeout of `10000`, only the matching effective and compiled timeout becomes `5000`. Unknown fields, `null`, removal/disabling, profile identity, `detection`, Search Request criteria, and persistence fields are unrepresentable. The authored pre-v3 phase name remains until T7; T1 adds no v2/v3 alias.

## Dependency and deletion decision

All compiler documents, merge logic, validation sequencing, Diagnostics, and plan construction are in-process. `SourceProfileRegistrySnapshot` is immutable input data. Source/profile loading stays with application/filesystem callers; HTTP, browser, and SQLite remain outside this compiler operation.

**Deletion test:** Without the Effective Profile Compiler boundary, Source validation, Source Live Check, Search Run preparation, lazy Detail preparation, and tests would each need to know profile resolution, fragment merge order, Effective Profile validation, Source Config validation order, Access Path resolution, safety checks, and plan construction.

## Examples

1. **No fragment:** a profile-based Source compiles to the same plan as today and exposes its unchanged Effective Source Profile.
2. **Valid scalar:** an existing `api/list_jobs` Strategy replaces only `fetch.timeoutMs`; all other effective/profile/plan values remain unchanged.
3. **Invalid reference/value:** an unknown Access Path/Strategy or out-of-bounds timeout rejects compilation with a stable compiler Diagnostic and no partial plan.
4. **Conflicting snapshot Source:** compilation uses the direct argument and ignores the snapshot's same-key Source document.

## Scope

- Add the minimum dedicated typed fragment documents and matching Source JSON Schema `$def`.
- Add schema/Serde parity fixtures for admitted and forbidden shapes.
- Materialize and completely validate an Effective Source Profile before Source Config validation and path resolution.
- Merge one scalar only when both stable keys identify existing base entries.
- Re-run existing semantic, capability, template, boundedness, security, and strict-plan checks on the effective result.
- Replace the key-based compiler entry point and migrate all production callers/tests directly.
- Move Source lookup and lifecycle admission to callers.
- Delete the old compiler snapshot/result/function after migration.

## Adjacent non-goals

- General recursive merge of inherited fields: T2/#168.
- New Strategies/Access Paths or Source Config Schema specialization: T3a/#169 and T3b/#170.
- Effective Profile provenance or freshness fingerprints: T4a/#171 and T4b/#175. T1 adds no placeholder fields for either capability.
- Authored Strategy Policies, schema-v3 phase names, complete legacy Source Overrides removal, UI authoring, deletion, disabling, reordering, or a general JSON Schema interpreter.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| No direct fragment | Current plan behavior is unchanged; profile access exposes unchanged `effective_profile` | External compiler integration test plus profile regressions |
| Valid existing path/Strategy timeout | `Compiled`; only the effective and planned timeout changes | External compiler integration test |
| Unknown Access Path | `Rejected`; stable fragment-path Diagnostic; no plan | External compiler integration test |
| Unknown Strategy | `Rejected`; stable fragment-path Diagnostic; no plan | External compiler integration test |
| Zero/out-of-bounds timeout | `Rejected` through existing boundedness rules | Compiler boundedness regression |
| Unknown property or explicit `null` | Source schema and Serde both reject | Schema/Serde parity fixtures |
| Profile key/name/kind/support or `detection` | Shape is unrepresentable or schema-invalid | Explicit schema/Serde parity fixtures |
| Search Request or persistence field | Shape is unrepresentable or schema-invalid | Schema/Serde parity fixtures |
| Draft/active/disabled equivalents | Identical compiler outcome | External compiler integration test |
| Conflicting same-key snapshot Source | Direct Source argument wins | Dedicated external regression |
| Profile-based access | `CompiledSourceAccess::Profile { effective_profile }` | External compiler integration test |
| Source-owned access | Explicit `SourceOwned` result; no fake effective profile | Existing/focused compiler regression |
| Error Diagnostic | `Rejected`, no partial plan/profile | External compiler integration test |
| Warning/info only | `Compiled` with non-error Diagnostics retained | External compiler integration test |
| Runtime boundary | Production runtime callers receive `SourceExecutionPlan` only | Call-graph/import review |
| Acceptance profiles | Greenhouse, Workday, SuccessFactors behavior unchanged without fragments | Existing deterministic profile tests |

Primary tests cross `compile_source(&source, &registry)` and inspect `CompileSourceOutcome`, `effective_profile`, Structured Diagnostics, and the immutable plan. Schema tests cross the Source schema entry point; Serde parity uses the same valid/invalid fixtures. Private merge tests are justified only for an edge not observable through `compile_source`.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
```

## Ticket-specific migration items

- [ ] Add the minimal typed fragment document family, Source-schema `$def`, and parity fixtures.
- [ ] Make the direct Source authoritative and add the conflicting same-key snapshot regression.
- [ ] Move Source lookup and lifecycle admission to every caller.
- [ ] Migrate Source validation, Source Live Check, Search Run selection, lazy Detail preparation, registry integration, and external tests to `compile_source`.
- [ ] Delete `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan` after all callers move.
- [ ] Delete aliases, forwarding functions, conversion snapshots, and status-mutation workarounds created solely for the old entry point.
- [ ] Verify runtime imports only the immutable typed Execution Plan.
- [ ] Classify every remaining hit from:

```bash
rg -n '\b(ProfileCompilerSnapshot|CompileSourceExecutionPlanResult|compile_source_execution_plan)\b' src-tauri/src src-tauri/tests
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
