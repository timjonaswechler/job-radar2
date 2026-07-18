# T2 — Complete deterministic Effective Source Profile merge semantics for inherited entries

## Result

A Source that selects a reusable Source Profile can specialize every T2-admitted execution-relevant value inside existing keyed Access Paths and Strategies. `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)` deterministically returns the recursively merged `EffectiveSourceProfile` and matching immutable `SourceExecutionPlan` without changing inherited order, mutating authored inputs, or exposing raw fragments to runtime.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#167](https://github.com/timjonaswechler/job-radar2/issues/167).
- Currently blocks: [#169](https://github.com/timjonaswechler/job-radar2/issues/169).
- Readiness: **Blocked**; #167 is open, and the landed compiler/fragment baseline must be re-inspected before assignment. The issue has no readiness label.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decisions 12–21: direct typed Source fragments, keyed merge, whole-array replacement, post-merge validation, and immutable compiled plans.
- #166 / PRD “Effective Profile Compiler” module decision: the direct Source is authoritative, the Registry Snapshot is immutable input data rather than a port, lifecycle admission remains outside the compiler, and merge/validation order stays private.
- #167 supplies the single `compile_source(source, registry)` entry point, `CompileSourceOutcome`, profile-based `effective_profile`, the distinct Source-owned branch, and the initial typed scalar fragment/schema definition.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.
- #169 owns complete Source-added Strategies and Access Paths; T2 therefore rejects fragment keys absent from the base profile.

## Current gap

The repository is still at the pre-#167 baseline. `src-tauri/src/profile_dsl/compiler/mod.rs` exposes `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, and `compile_source_execution_plan(snapshot, source_key)`; it selects and lifecycle-checks a Source from the snapshot. `compiler/resolution.rs` resolves an Access Path and builds a plan without exposing an Effective Source Profile, while `compiler/overrides.rs`, `profile_dsl/documents/overrides.rs`, `schema/profile-dsl/overrides.schema.json`, `source/documents.rs`, and `schema/source.schema.json` still implement `sourceOverrides.strategyOverrides` rather than direct typed fragments.

`compiler/keys.rs` detects duplicate base Access Path and Strategy keys with authored array indices. `compiler/source_config.rs` uses `HashSet` iteration for required properties, schema properties, and intersections that can reach Diagnostic emission without canonical sorting. Production callers in Source validation, Source Live Check, Search Run selection, and posting Detail still use the old compiler entry point. Relevant coverage exists in `compiler_resolution`, `compiler_semantic_validation`, `compiler_security_boundedness`, `schema_validation`, `source_profile_registry`, document Serde tests, and the Greenhouse, Workday, and SuccessFactors profile targets.

Because #167 has not landed, these exact paths and names are provisional. At readiness review, re-baseline against #167 and extend its landed typed fragment and private merge implementation; do not restore any wrapper, snapshot, lifecycle check, or key-based compiler facade that #167 removed. The remaining intended gap is recursive merge beyond T1’s single scalar, complete admitted fragment coverage, keyed-entry order/duplicate behavior, whole replacement of non-keyed arrays, and map-order-independent results and Diagnostics.

## Target delta

T2 does not change #167’s public compiler responsibilities: callers still pass one authoritative `SourceDocument` and one immutable `SourceProfileRegistrySnapshot`, and receive either a complete `CompiledSource` plus non-error Diagnostics or rejection with at least one error and no partial profile or plan. Exact Rust names may follow the landed #167 implementation.

For profile-based compilation, the private merge adds these rules:

1. **Admitted fragment fields.** Extend #167’s dedicated typed fragment documents and Source-schema `$def` across existing `postingDiscovery` and `postingDetail` steps: step acceptance; Strategy fetch; Discovery pagination; parse; select; conditions/filters; captures; Detail match; extraction and field expressions; transforms; and Strategy acceptance. Profile/Access Path names and descriptions, identity, `detection`, support/known-issue metadata, authored Diagnostics, Source Config Schema, Search Request criteria, persistence fields, deletion/control fields, and wrapper/patch shapes remain unrepresentable.
2. **Recursive objects and scalars.** A supplied object recursively merges into its corresponding base object; unmentioned siblings survive and supplied scalar leaves replace base leaves. Dynamic objects such as headers, captures, posting-metadata expressions, and typed request bodies follow the same rule.
3. **Tagged objects.** A `mode` or `type` discriminator is an ordinary scalar replacement. Changing it does not erase inherited siblings. The complete merged value must deserialize and validate as one legal variant; there is no variant-switch shortcut.
4. **Keyed arrays.** Access Paths merge by stable `key`; Discovery and Detail Strategies merge by stable `key` within their phase and Access Path. T2 accepts only entries already present in the base profile. Fragment order cannot reorder them: Effective Access Paths and Strategies retain base order, and the selected Access Path’s plan retains base Strategy order.
5. **Non-keyed arrays.** Every admitted array without stable element identity is replaced as a whole, never appended or merged by index. This includes transforms, conditions/filters, acceptance `requiredFields`, browser waits, browser interactions, and equivalent lists.
6. **No implicit removal.** Structural `null` at an optional fragment member is schema-/Serde-invalid and cannot delete inherited data. A literal JSON `null` remains data only in an existing typed DSL leaf that admits arbitrary JSON. Deletion, disabling, authored placement/reordering, and operation/path/value patches are invalid and yield no compiled output.
7. **Duplicates and unknown keys.** Duplicate Access Path or Strategy keys in either the base profile or one direct Source fragment reject compilation. Every occurrence after the first emits exactly one compiler Diagnostic in authored-array order. Each unknown fragment key likewise emits one Diagnostic. Base duplicate paths use base indices; fragment duplicate/unknown paths use direct-fragment indices.
8. **Diagnostic paths and keys.** Paths are real JSON Pointers to concrete authored entries, such as `/accessPaths/0/postingDiscovery/strategies/1/key`, never keyed pseudo-paths. Stable keys appear in `details` and, for Strategies, `strategyKey`. Post-merge Diagnostics use the indices of the base-ordered Effective Source Profile.
9. **Determinism.** Semantically equivalent inputs differing only in JSON object/map insertion order produce equal Effective Source Profiles, equal plans, and identically ordered Structured Diagnostics. Every Diagnostic-producing Set/Map traversal—including existing Source Config required/property/intersection paths—is canonically sorted before emission. Contractually meaningful authored array order remains meaningful.
10. **Validation and result integrity.** Validate the complete Effective Source Profile after merge and before Source Config validation, selected Access Path resolution, and plan construction. Existing semantic, capability, template, boundedness, security, support, and strict-plan checks apply to merged values. `Compiled` contains no error Diagnostic; rejection exposes no `CompiledSource`, Effective Source Profile, or partial plan.
11. **Boundaries.** Base profile and Source inputs remain unchanged. Merge, duplicate detection, lookup, validation sequencing, and plan construction stay private. Runtime receives only `SourceExecutionPlan`; Source-owned access preserves #167 behavior and is neither merged nor represented as an Effective Source Profile. This pure operation adds no Cancellation, runtime budget, completion, provenance, or persistence state. Backend-owned global ceilings remain outside Source specialization; #169 only applies the landed security and boundedness checks to complete additions.

## Dependency and deletion decision

Typed Sources/fragments, Source Profiles, Effective Source Profiles, the Registry Snapshot, merge logic, validators, Diagnostics, and plan construction are in-process data/computation and are tested with the real implementation. Registry loading, SQLite, HTTP, and browser execution remain outside this compiler operation. No merge, registry, or validator trait/adapter is introduced.

**Deletion test:** Removing the Effective Profile Compiler boundary would force Source validation, Source Live Check, Search Run preparation, posting-detail preparation, and tests to reconstruct keyed lookup, recursive merge, inherited order, complete validation sequencing, deterministic Diagnostics, and plan construction. Private helpers may change or disappear only while that complexity remains behind `compile_source`.

## Examples

1. **Recursive merge:** if base `fetch.headers` contains `accept` and `user-agent`, and the fragment replaces only `headers.accept` plus `timeoutMs`, the effective profile and plan retain `user-agent`, replace the supplied leaves, and leave both authored inputs unchanged.
2. **Whole-array replacement:** replacing base transforms `[trim, dedupe]` with `[normalize_whitespace]` yields exactly one effective transform; nothing is retained, appended, or positionally merged.
3. **Inherited order:** base Strategies `[primary, fallback]` remain in that order in the Effective Source Profile and selected plan even when the fragment mentions `[fallback, primary]`.
4. **Duplicate/invalid behavior:** a second `list_jobs` Discovery Strategy under fragment Access Path index `0` emits exactly one duplicate Diagnostic at `/accessPaths/0/postingDiscovery/strategies/1/key` with `strategyKey: list_jobs`. Further duplicates each add one ordered Diagnostic. An unknown key or a merged timeout rejected by boundedness also rejects compilation without partial output.

## Scope

- Extend #167’s typed direct-fragment Rust documents and matching Source-schema `$def` for all T2-admitted existing-entry fields.
- Implement one private deterministic recursive merge for existing Access Paths and Discovery/Detail Strategies, preserving base order and replacing non-keyed arrays whole.
- Reject duplicate/unknown keys, structural `null`, deletion, disabling, and placement/reordering with the specified paths, key details, cardinality, and result integrity.
- Revalidate the complete Effective Source Profile and continue through Source Config validation, selected Access Path resolution, and immutable plan compilation.
- Canonically sort every Diagnostic-producing Set/Map traversal, including existing `source_config.rs` required-key, property, and intersection paths.
- Add/move external compiler tests through `compile_source` and schema/Serde parity fixtures for admitted and forbidden shapes.
- Keep all production callers on #167’s single compiler entry point; move any landed scalar-specific caller knowledge behind it.
- Delete T1-only scalar special cases, superseded narrow temporary fragment types, duplicate private merge paths, conversion snapshots, and equivalent implementation-detail tests after caller-facing coverage exists.
- Retain the landed pre-v3 `postingDiscovery`/`postingDetail` authored names until the schema-v3 owner moves them; add neither old/new aliases nor a v2/v3 dispatcher.

## Adjacent non-goals

- Complete new Source-added Strategies/Access Paths, append order, or selecting a newly added Access Path: #169.
- Source Config Schema specialization or its constrained schema subset: #170.
- Effective-profile provenance and fingerprints: #171 and #175.
- Strategy Policies/runtime budgets, new backend-ceiling dimensions or values, schema-v3 phase names, or the v3 hard cut.
- Deletion, disabling, structural-null merge semantics, or authored placement/reordering.
- New Profile DSL Primitives, provider-specific Rust behavior, or a general JSON Schema validator.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Scalar replacement | One supplied scalar changes in the Effective Source Profile and plan; unrelated values and inputs remain unchanged | External compiler integration test |
| Recursive object | One nested member changes while inherited siblings survive in the effective value and plan | External compiler integration test |
| Several inherited entries | Existing Strategies across existing Access Paths/phases merge by key; all effective paths and the selected plan are correct | External compiler integration test |
| Inherited order | Fragment order cannot reorder effective Access Paths/Strategies or selected-plan Strategies | External compiler integration test |
| Non-keyed arrays | Representative transforms, conditions, required fields, waits, and interactions replace complete inherited arrays | External compiler integration tests |
| Duplicate Access Path | One ordered Diagnostic per occurrence after the first, at the concrete base/fragment array index with key details; no partial result | External compiler integration test |
| Duplicate Strategy | Same cardinality/path rule, with `strategyKey`, independently for Discovery and Detail | External compiler integration test |
| Unknown keyed entry | One concrete fragment-path Diagnostic per unknown Access Path/Strategy; no T3 addition behavior | External compiler integration test |
| Tagged variant change | An illegal fully merged variant is rejected without implicit sibling deletion | Schema/Serde parity plus compiler test |
| Forbidden or incomplete fragment shape | Schema and Serde reject profile/Access Path identity, `detection`, Search Request, persistence, unknown-property, and incomplete typed shapes | Schema/Serde parity fixtures |
| Structural `null` or control field | Schema and Serde reject structural null/deletion/disable/placement; typed JSON null remains data only where admitted | Schema/Serde parity fixtures |
| Invalid effective behavior | Post-merge semantic, capability, template, boundedness, security, support, or strict-plan failure rejects without a plan | External compiler semantic/security tests |
| Boundedness | A specialized zero/missing or otherwise invalid current bound is deterministically rejected with no partial result; global-ceiling specialization is not added | `compiler_security_boundedness` integration test |
| Map-order determinism | Object insertion-order variants produce equal effective values/plans and identical ordered Diagnostics, including Source Config set paths | External compiler determinism tests |
| Direct Source authority | A conflicting same-key Source in the Registry Snapshot cannot affect the direct argument | Retained #167 external regression |
| Source lifecycle | Equivalent draft/active/disabled Sources compile identically; admission remains with callers | Retained #167 external regression |
| Source-owned access | Existing Source-owned plan behavior remains; no Effective Source Profile is fabricated | External compiler regression |
| Result integrity | Success has no error Diagnostic; rejection has no partial profile or plan | External compiler integration test |
| Runtime boundary | Runtime imports/receives only `SourceExecutionPlan`, never fragments/raw Source/Profile JSON | Integration assertion plus repository search and call-graph review |
| Regression | Greenhouse, Workday, and SuccessFactors behavior is unchanged without direct fragments | Existing deterministic profile targets |
| Cancellation | No Cancellation, `ResolutionCompletion`, or runtime partial-completion type is added | Static API/repository review |

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

If #167 adds a dedicated external Effective Profile Compiler target, add that exact target after readiness re-baselining. The full crate regression follows the shared delivery contract.

## Ticket-specific migration items

- [ ] Inspect landed #167 and extend its exact public/compiler and typed-fragment baseline rather than introducing a parallel model.
- [ ] Add schema/Serde parity fixtures for recursive/scalar/tagged values, whole-array replacement, duplicates, unknown keys/properties, incomplete typed shapes, forbidden identity/`detection`/Search Request/persistence fields, structural versus typed-data `null`, and forbidden control fields.
- [ ] Implement one private deterministic merge; preserve base order and validate the complete effective value before downstream compiler stages.
- [ ] Canonically sort all Diagnostic-producing Set/Map traversals, including Source Config required/property/intersection emission; statically review remaining traversals.
- [ ] Move affected tests to `compile_source` and retain production callers solely on that entry point.
- [ ] Delete T1-only scalar merge code/narrow temporary fragment types, duplicate merge implementations, public/pass-through merge stages, conversion snapshots, aliases/forwarding wrappers, and superseded implementation-detail tests.
- [ ] Confirm the operation-list `sourceOverrides`/`StrategyOverride` path is not restored as an active alternative and Source selection/lifecycle logic does not return to the compiler.
- [ ] Confirm runtime receives only the immutable typed Execution Plan.
- [ ] Classify every remaining hit from ticket-specific searches over active Rust, schemas, and fixtures for `sourceOverrides`, `StrategyOverride`, scalar-only compatibility paths, public merge stages, duplicate merge implementations, and unsorted Diagnostic-producing `HashSet`/Map traversal.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
