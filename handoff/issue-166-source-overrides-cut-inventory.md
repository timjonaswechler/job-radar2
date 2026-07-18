# Issue #166 Phase 2 — `sourceOverrides` hard-cut inventory

Status: read-only repository inventory under accepted D-001/D-011  
Repository inspected: `/Users/tim-jonaswechler/GitHub-Projekte/job-radar2`  
Decisions treated as fixed: `handoff/issue-166-phase-1-decisions-handoff.md` and `handoff/issue-166-contract-decisions.md` were read completely. This inventory does not reopen D-001–D-013.

## 1. Fixed cut contract and classification

D-001 makes **Direct Source Specialization** the only final model. D-011 requires the retained final compiler foundation in this responsibility order:

```text
final compiler interface
→ existing-entry merge
→ new Strategies/Access Paths
→ Effective Source Config Schema
→ mandatory first_accepted Policy
→ final phase names
→ complete provenance
→ canonical schema-v3 fingerprints
→ atomic activation/hard cut
```

The activation owner must switch all authored and productive callers at once and delete the complete old surface. There may be no compatibility wrapper, old-to-new translator, alias, fallback, migration DTO, or old/new productive route.

This document distinguishes:

* **Exact old symbols/serialized names:** `sourceOverrides`, `strategyOverrides`, `source_overrides`, `SourceOverrides`, `StrategyOverride`, `OverridableStep`, `apply_source_overrides`, the `source_overrides` fingerprint kind, and files/modules dedicated to them.
* **Conceptual prose:** human-readable “Source Overrides” or “Source Override model” references. These are not executable symbols, but active normative/authoring prose must change at activation. Clearly historical comparison can remain only when unmistakably classified as history.
* **Generic “override” vocabulary:** unrelated uses were not classified as this cut merely because they contain the word “override”. No generic patch/operation/path/value model was found in the current executable sourceOverrides implementation.

## 2. Current productive call chain

```text
Frontend Create/Edit
  SourceOverridesEditor + sourceOverridesFromText
  → SourceDocument.sourceOverrides
  → Tauri create_source/update_source
  → pretty JSON in app-data/sources/<key>.json
  → registry loader deserializes SourceDocument
  → derive_source_validation_state
  → compile_source_execution_plan(snapshot, source key)
  → resolution::compile_profile_access_path
  → apply_source_overrides (clone selected base steps, mutate matching Strategy)
  → semantic/capability/template/boundedness/security validation
  → immutable SourceExecutionPlan
  → Search Run selection / lazy posting Detail / Source Live Check
```

Source Live Check separately fingerprints the complete Source document and emits a dedicated `source_overrides` fingerprint. Source documents are filesystem JSON, not SQLite rows. `src-tauri/resources/sources/` contains only `.gitkeep`; no tracked production Source JSON currently carries the old field.

## 3. Exact executable/authored inventory

In the tables, “final owner” means the retained replacement responsibility, while “activation” is always the one atomic D-001/D-011 switch. Current ticket labels are responsibility labels only, not approved final ticket boundaries.

### 3.1 Rust document model, module/export, and persistence boundary

| Current path / symbol | Current responsibility and productive caller/authored surface | Retained final replacement owner | Activation migration | Exact deletion target / proof | Uncertainty |
|---|---|---|---|---|---|
| `src-tauri/src/profile_dsl/documents/overrides.rs:12-37` — `SourceOverrides`, `StrategyOverride`, `OverridableStep` | Strict Serde model for `strategyOverrides[]`; permits only `postingDiscovery`/`postingDetail` and optional whole `fetch`, `select`, field-map `extract`, `acceptWhen`. Consumed by `SourceDocument` and compiler. | Final typed direct Source-fragment document family from the compiler-foundation chain (existing keyed specialization, additions, schema specialization, Policy, final phase names). | Schema-v3 Source root directly authors typed profile fragments in the same nested vocabulary; no wrapper or conversion. | Delete the entire file. Search proof: no `SourceOverrides|StrategyOverride|OverridableStep` in active production/schema/resources; negative hard-cut fixtures may mention serialized old names only to prove rejection. | Exact final filenames/types do not exist yet and must follow the landed foundation rather than the old lean-draft names.
| `src-tauri/src/profile_dsl/documents/mod.rs:6,22` — `pub mod overrides`; public re-exports | Makes old types available throughout the crate-internal document namespace. It is the only direct old-type export; `src-tauri/src/lib.rs` does **not** separately re-export these three types. | Final document module exports for direct fragments. | Replace imports at the Source document/compiler boundary with final typed fragment exports. | Delete module declaration and re-export line; prove no old type import/export remains. | None.
| `src-tauri/src/source/documents.rs:4-22` — import and `SourceDocument::source_overrides` | Canonical Rust Source document owns optional serialized `sourceOverrides`; all registry, commands, compiler, checks, search, and public `SourceDocument` consumers transitively carry it. | Schema-v3 `SourceDocument` with direct Source specialization fields/fragments and final phase names. | Replace field in place at the authored boundary; strict Serde rejection for old field after cut. | Delete import and field. `deny_unknown_fields` becomes direct rejection proof for `sourceOverrides`. | Final direct-fragment field layout depends on landed foundations; it must be direct-root, not a renamed wrapper.
| `src-tauri/src/lib.rs:63` — public `SourceDocument` export | External tests and crate consumers receive the old field transitively through the exported type even though old override types are not directly exported. | Same public `SourceDocument`, schema-v3 shape. | Recompile all external callers/tests against the final Source type. | No need to delete the `SourceDocument` export; proof is absence of old field/type from its definition and all external constructors. | None.
| `src-tauri/src/app/commands.rs:534-640` — `create_source`, `update_source`, `write_source_document` | Productive persistence route. Accepts typed Source from Tauri, pretty-serializes it, and writes `app_data/sources/<key>.json`; reloads registry afterward. | Existing command/persistence responsibility consuming final schema-v3 `SourceDocument`. | Frontend sends only final Direct Source Specialization; commands serialize final type directly. No DTO translator. | Retain commands; delete old field through the type. Caller/round-trip tests must show old JSON rejected and final JSON written. | Runtime app-data files lie outside the repository and cannot be enumerated here.
| `src-tauri/src/app/commands.rs:1082-1104` — `command_test_source_document`, `source_overrides: None` | Test helper constructs the persisted Source Rust struct and therefore hard-codes the old optional field. | Existing command test helper updated to final Source shape. | Construct final Source document directly. | Delete the struct member assignment (not necessarily the helper). Search proof includes snake_case old field. | None.
| `src-tauri/src/source_profile/registry/loading.rs:70-98,247-281` — Source loading and validation | Productive reader of built-in/custom filesystem JSON: deserializes `SourceDocument`, builds compiler snapshot, derives validation for every Source. Old field reaches compiler without a separate registry interpretation. | Final registry + single final compiler interface. | Strictly deserialize schema-v3 Source; invoke final compiler with authoritative Source and immutable registry. | Retain loader; prove it contains no compatibility parse/version dispatcher and old field is rejected. | JSON Schema is exercised in tests/UI, while production registry parsing shown here is Serde-based; activation must retain strict parity.

### 3.2 Compiler and validation

| Current path / symbol | Current responsibility and productive caller | Retained final replacement owner | Activation migration | Exact deletion target / proof | Uncertainty |
|---|---|---|---|---|---|
| `src-tauri/src/profile_dsl/compiler/mod.rs:15` — `mod overrides` | Registers the old private compiler implementation. | Final Effective Profile Compiler modules. | Compiler resolves base, merges direct fragments, validates complete Effective Source Profile, validates Source Config, resolves selected path, builds immutable plan. | Delete module declaration with the file. | None.
| `src-tauri/src/profile_dsl/compiler/overrides.rs:10-232` — entire module | Sole old behavior implementation. Clones selected Discovery/Detail steps, detects duplicate `(step,strategyKey)`, replaces fetch/select/acceptance, mutates admitted extract fields, and emits old pointer/code diagnostics for duplicate/unknown/unsupported items. | Final compiler interface + deterministic typed direct-fragment merge (existing keyed entries, complete additions, whole-array replacement), final validators, Policy, names and provenance. | Productive compiler calls final merge directly. No `EffectiveAccessPathSteps` adapter and no translation of operation lists. | Delete entire file including `EffectiveAccessPathSteps`, `apply_source_overrides`, both apply helpers, diagnostic helpers and `step_name`. Prove no old diagnostic codes/paths remain except explicit negative hard-cut assertions: `duplicate_strategy_override`, `unknown_strategy_override`, `unknown_extract_override_field`, `unsupported_extract_override_field`. | The exact mapping of every old allowed behavior is broader in the accepted replacement, but parity must specifically cover fetch/select/extract/acceptance for existing keyed Discovery/Detail Strategies before activation.
| `src-tauri/src/profile_dsl/compiler/resolution.rs:22,128-174` — import and profile branch | Productive compiler resolves profile and Access Path **before** old specialization, applies old mutations only to selected steps, validates and compiles them. | Final compiler pipeline in D-011: authoritative Source → Effective Source Profile → full validation → Source Config → selected path → plan. | Replace this route with the landed final compiler interface and effective-profile result. Every caller switches directly in activation. | Delete import/call/old `EffectiveAccessPathSteps` parameter use; no old/new branch. Proof through compiler integration and call search. | Foundation may move code/files; inventory identifies behavior, not a required final location.
| `src-tauri/src/profile_dsl/compiler/resolution.rs:249-313` — `validate_profile_access_path(... EffectiveAccessPathSteps ...)` | Runs key/template/capability/boundedness/security validation after old selected-step mutation. | Complete Effective Source Profile and shared Effective Source Config validation, before selected-path resolution. | Validate final effective profile, including unselected additions, once; do not retain this old wrapper shape. | Remove `EffectiveAccessPathSteps` dependency and any validation route limited to old selected-step overlay. | Exact helper retention/renaming is implementation-dependent; old type dependency is the deletion invariant.
| `src-tauri/src/profile_dsl/compiler/resolution.rs:368-401` — source-owned prohibition | Emits `source_overrides_not_supported_for_source_owned_access_path` at `/sourceOverrides`. | Final Source-owned branch remains distinct and admits no fake profile specialization. Strict final Source schema/type makes old wrapper unrepresentable. | Delete old diagnostic branch; final Source-owned validation enforces only the final model’s real constraints. | Delete `source.source_overrides` check and old diagnostic/code/path. Negative schema/Serde test proves old field rejection. | Direct Source specialization applies only to profile-selected Sources; source-owned paths remain edited directly under accepted architecture.
| `src-tauri/src/profile_dsl/compiler/mod.rs:23-93` — `ProfileCompilerSnapshot`, `CompileSourceExecutionPlanResult`, `compile_source_execution_plan` | Current key-based public compiler facade selects Source from mutable snapshot, checks status, then enters resolution. It is not named after overrides but is the sole productive interpreter entry. | Final `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)`-style deep interface and closed compiled/rejected outcome; Direct Source is authoritative and lifecycle admission is caller-owned. | All production/test callers migrate directly in the atomic activation; no forwarding old function. | Delete old facade types/function and exports after every caller moves; search `ProfileCompilerSnapshot|CompileSourceExecutionPlanResult|compile_source_execution_plan`. | Exact final symbol follows landed foundation; no alias may preserve old call signature.
| `src-tauri/src/source/validation.rs:30-96` — `derive_source_validation_state` | Productive registry/live-check validation caller. Clones snapshot, forces Source active, compiles, and maps compiler diagnostics to derived state. Thus old overrides affect canCompile/canExecute. | Final Source validation calling final compiler; lifecycle admission remains outside compiler. | Call final compiler directly with authoritative Source/registry; remove status-mutation workaround and old snapshot. | Retain derived-state feature, delete old compiler imports/snapshot clone/status workaround. Caller tests cover invalid direct fragments. | Exact category mapping may change under Effective Source Config contract; do not reconstruct via wrapper.
| `src-tauri/src/profile_dsl/diagnostics/mod.rs:16-21` — Compiler doc comment | Conceptual code documentation says compiler includes Source Overrides. | Final diagnostics vocabulary referring to Direct Source Specialization/Effective Source Profile. | Rewrite comment in activation. | No old conceptual wording in active Rust docs. | None.
| `src-tauri/src/search/run/service/selection.rs:95-139` — Search Run Source selection | Productive Search Run caller validates execution eligibility, optionally forces draft active in a copied snapshot, compiles, and converts plan to runtime input. Old overrides affect every Search Run. | Final Search Run preparation consuming only `CompiledSource.execution_plan`; lifecycle decision outside compiler. | Call final compiler directly; remove old status-mutation/snapshot and no productive choice. | Retain source selection; delete old compiler symbols and workaround. Search + Search Run regressions prove only immutable plan crosses runtime. | `allow_draft_source` behavior must be preserved outside compiler.
| `src-tauri/src/search/posting/service.rs:180-230` — lazy posting Detail preparation | Productive UI/posting path checks validation, recompiles Source and requires a Detail plan. Old overrides can change Detail fetch/select/extract/acceptance. | Final candidate-scoped Detail preparation consuming final compiled plan. | Switch directly to final compiler and final Detail naming/plan; no old fallback. | Retain service; delete old compiler calls/imports and old diagnostic vocabulary. Lazy Detail regressions prove parity. | This path is productive independently of Search Run and must not be omitted from activation.
| `src-tauri/src/checks/source_live/mod.rs:160-288,384-401` — validation + compile for live execution | Productive Source Live Check derives validation, compiles a draft by forcing active, then executes Discovery and optional Detail; old overrides directly change checked behavior. | Final Source Live Check using final compiler and immutable plan. | Compile authoritative Source directly, with lifecycle admission outside compiler; execute canonical final plan. | Delete old compiler call and status-mutation helper shape; caller test proves Direct Source Specialization affects checked behavior. | Browser cut is separate D-007 work; this row covers only specialization/compiler migration.

### 3.3 JSON Schema and schema consumers

| Current path / symbol | Current responsibility/authored surface | Retained final replacement owner | Activation migration | Exact deletion target / proof | Uncertainty |
|---|---|---|---|---|---|
| `src-tauri/src/schema/profile-dsl/overrides.schema.json:1-35` — entire schema, `$defs.sourceOverrides`, `$defs.strategyOverride` | Canonical old JSON Schema for the wrapper/list and allowed operation payload. | Matching schema-v3 typed direct-fragment `$def` in the Source schema/catalog, produced by compiler foundations. | Source root exposes direct fragments only; schema admits final fields and rejects wrapper/list. | Delete entire file. No `$id`, `$ref`, filename, `sourceOverrides` or `strategyOverrides` in positive schema/resources. | Final schema may be in `source.schema.json` or a dedicated fragment schema; only the direct-root contract is invariant.
| `src-tauri/src/schema/source.schema.json:20,39-53` | Exposes `sourceOverrides` and conditionally prohibits it only for source-owned Access Paths. | Schema-v3 Source document with typed direct specialization for profile-selected Source and canonical phase names. | Replace property/conditional in place; old field becomes invalid through `additionalProperties:false`. | Delete old property `$ref` and `not required sourceOverrides`; add explicit negative old-field fixture. | The complete activation also changes schema version/names/Policy; this inventory does not widen beyond identifying their D-011 prerequisites.
| `src-tauri/tests/schema_validation.rs:6-19,42-54,92-113` — schema registry and cases | Registers old schema URI, accepts positive fixture containing old field, rejects an unsupported `transforms` member via old schema. | Final schema registry and schema/Serde parity suite for direct fragments. | Remove old schema from registry; register final fragment schema if separate; rewrite positives and add strict old-wrapper negatives. | Delete `SCHEMA_FILES` old entry and obsolete old-shape invalid fixture reference. Proof: schema suite + no unresolved `$ref`. | The generic harness remains.
| `src/features/sources/shared/profile-dsl-schema-catalog.ts:5,20-38` — import, `profileDslSchemaRefs.sourceOverrides`, catalog entry | Frontend imports backend old schema and exports it to editor/details schema introspection. This is a real authoring/export route. | Frontend catalog entry/refs for final direct Source specialization schema. | Replace directly with final schema reference; no old ref alias. | Delete old import, ref key and catalog entry. Search old filename/ref/export. | Final editor may need multiple direct fields rather than one wrapper ref; do not preserve a convenience wrapper solely for UI.

### 3.4 TypeScript document/API and create/edit/details UI

| Current path / symbol | Current responsibility and productive caller/authored surface | Retained final replacement owner | Activation migration | Exact deletion target / proof | Uncertainty |
|---|---|---|---|---|---|
| `src/lib/api/sources.ts:178-188` — `SourceDocument.sourceOverrides?: JsonValue` | Public frontend DTO accepted by `createSource`/`updateSource` (`:360-365`) and returned in registry snapshots. Weak `JsonValue` typing leaves backend schema/compiler authoritative. | Final schema-v3 frontend Source type with explicit direct specialization shape (as strongly typed as landed UI contracts allow). | Change DTO and invoke payload in place; no legacy DTO/translator. | Delete property and all accesses. Typecheck plus old-symbol search proves removal. | Exact TS fragment types are not yet present.
| `src/features/sources/source-form/source-overrides.ts:7-69` — parser/starter/helpers | Parses raw JSON object; produces a `strategyOverrides` starter targeted at first Discovery/Detail Strategy. Used by Create/Edit hooks/models and tests. | Final direct-specialization authoring helpers, if needed, built against final schema and root fields. | Replace with direct-fragment editing/model construction, not a renamed JSON wrapper parser. | Delete entire file and all imports, unless filename is replaced wholesale with a final, non-wrapper specialization module. Required proof is no exported old helper names or `strategyOverrides` starter. | Rich UI is out of scope; a schema-guided final editor is sufficient, but must author the real final shape.
| `src/features/sources/source-form/source-overrides-editor.tsx:20-113` — `SourceOverridesEditor` | Productive schema-guided raw JSON UI with add/remove template, old German copy, and old schema ref. | Final Direct Source Specialization authoring surface using actual root fragments/schema. | Replace component directly in both Create/Edit; no component alias forwarding old props. | Delete file/component/copy/ARIA labels old vocabulary. UI contract/static search proves no old authoring affordance. | Final component boundaries may differ.
| `src/features/sources/create/source/source-create-model.ts:80-134,268,321-385` — draft fields and `buildCreatedSourceDocument` | Carries raw old text through dirty-state/detection state; parses it and conditionally sets `document.sourceOverrides`. | Final create model representing direct specialization and building one schema-v3 Source. | Rename/restructure draft to final authored fields; detected proposals still begin without Source specialization unless explicitly authored. | Delete all `sourceOverridesText`, `overridesErrors`, parser import, and assignment. Do not retain old field in preview JSON. | Exact final draft shape depends on schema-guided UI.
| `src/features/sources/create/source/use-source-create.ts:10,60,90-145,179-266,325-349` | Productive hook state, starter, dirty tracking, reset on profile/path/detection changes, preview, and create API call. | Final create hook for direct specialization. | Wire final editor/model; preserve intentional reset when base profile/path changes because fragment keys/contracts may no longer apply. | Delete old state/actions/starter imports and names. Hook/create tests prove final payload and unsaved-change behavior. | Whether all final fragment state should reset on base change is a product behavior to preserve unless final compiler/UI provides a safer explicit reconciliation; no translator is allowed.
| `src/features/sources/create/source/source-create-drawer.tsx:19,137-144` | Productive Create authoring placement between Source Config and JSON preview. | Final direct specialization editor placement. | Render final component directly. | Delete old import/JSX. Component/static contract proves no old labels. | None.
| `src/features/sources/edit/source/source-edit-model.ts:12-135` — drafts/build | Reads old DTO into pretty JSON, tracks dirty raw input, parses and sets/deletes old field while preserving other document fields. | Final edit model round-tripping direct schema-v3 specialization. | Populate and save final direct fields directly. No on-load conversion from old data. | Delete old draft/error/parser/property logic. Tests prove final Source round-trip and strict old rejection. | Pre-production hard cut means old external documents are not automatically converted.
| `src/features/sources/edit/source/use-source-edit.ts:10,68-145,199-225` | Productive edit session state/starter/baseline/dirty/reset. Computes `supportsProfileOverrides` from selected Access Path type. | Final edit hook: Direct Source Specialization authorable only for profile-selected Sources; source-owned stays direct inline editing. | Replace state/editor inputs with final direct fragments and real schema. | Delete old helper/state/data/action names and `supportsProfileOverrides` old wording; retain the semantic profile-vs-source-owned gate under final terminology. | Exact UI affordance for fragment additions is not landed.
| `src/features/sources/edit/source/source-edit-drawer.tsx:32,133-143` | Productive conditional old editor in Edit drawer. | Final direct specialization editor. | Render final component for eligible profile-selected Source. | Delete old import/JSX. UI search/contract test. | None.
| `src/features/sources/registry/source/source-details.tsx:112-118` | Productive details/inspection renders old authored wrapper with old schema ref. | Details view of the final direct Source specialization and, where available, Effective Source Profile/provenance. | Replace preview directly; no “legacy overrides” panel. | Delete old title/description/property/schema ref. Details contract + search. | Effective/provenance display depth is bounded by landed UI scope; at minimum authored final data must be inspectable.
| `dist/assets/sources-page-CoFLO4jB.js` (ignored generated Vite build; 43 `sourceOverrides`, 3 `strategyOverrides`, 6 “Source Overrides” occurrences) | Local generated frontend bundle reproduces the current productive UI. It is not tracked by Git and is not source of truth. | Rebuilt frontend artifact from final source, if local `dist/` is regenerated. | Run final build after source migration; never hand-edit/minify-translate. | Delete stale ignored artifact or regenerate; post-build search must have no old productive UI names. | `dist/` is ignored/untracked, so it is not a commit deletion target but is repository-working-tree residue relevant to exhaustive search.

## 4. Tests, fixtures, examples, and persistence assumptions

| Current path / symbol | Current responsibility/authored surface | Final replacement owner | Activation migration | Exact deletion/rewrite target and proof | Uncertainty |
|---|---|---|---|---|---|
| `src-tauri/tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json:1-25` | Shared positive schema/Serde/compiler/live-check fixture; active schema-v2 persisted Source example with `sourceOverrides.strategyOverrides[].acceptWhen`. | Final schema-v3 direct-fragment positive fixture. | Rewrite in place to final direct root fields, phase names and required Policy. Do not keep v2/v3 pair. | No old names in valid fixtures; compiler parity must still show acceptance threshold specialization. | Fixture is reused widely; all transitive tests must be re-baselined.
| `src-tauri/tests/fixtures/source-profile-dsl/invalid/source-override-transforms.json:1-23` | Proves old override schema rejects `transforms`. | Final parity negatives for forbidden direct-fragment shapes plus explicit old-wrapper rejection. | Remove/rename obsolete operation-list fixture; add focused strict rejection for `sourceOverrides`/`strategyOverrides`. | Delete old fixture filename/shape unless retained only as clearly named hard-cut negative. Positive surfaces must have zero old hits. | Negative old literals are allowed only as rejection proof and must be classified in search output.
| `src-tauri/src/profile_dsl/documents/serde_tests.rs:5,79-109,116-123` | Directly deserializes/round-trips old types and shared Source fixture. | Final direct-fragment Serde/schema parity tests. | Rewrite tests to final Source document through public boundary; no old type-only compatibility test. | Delete `source_overrides_fixture_deserializes_structurally`, old imports/assertions; retain rewritten Source round-trip. | None.
| `src-tauri/tests/compiler_resolution.rs:11-66,94-119` | Public compiler proof that old overrides are applied before plan compile and raw wrapper is absent from plan. | Final compiler integration: Direct fragment → Effective Source Profile → immutable plan, raw fragments absent from runtime. | Rewrite using final compiler/outcome and final fixture. | Delete old-name assertions/messages and old compiler facade use. Preserve behavioral acceptance threshold check in final vocabulary. | None.
| `src-tauri/tests/compiler_semantic_validation.rs:93-175` | Tests unknown Strategy old diagnostic, source-owned prohibition, and post-override capability validation. | Final fragment-key/completeness/schema rejection and post-merge full validation tests. | Rewrite to final direct fragment paths/codes and compiler interface. | Delete old diagnostic assertions and struct field mutations. | Exact final diagnostic codes/paths follow landed compiler foundation.
| `src-tauri/tests/compiler_security_boundedness.rs:119-141` | Injects forbidden/unbounded Fetch through old wrapper and proves validation runs after mutation. | Final post-specialization security/boundedness compiler test. | Express same unsafe change through final direct fragment. | Delete old JSON paths/test name; preserve no-plan and security/bounds behavior. | D-012 separately removes retry assumptions, not part of this inventory’s cut.
| `src-tauri/tests/source_profile_registry.rs:300-352` | Writes a custom persisted Source with unknown old Strategy override and proves registry surfaces compiler diagnostic. | Final custom schema-v3 Source with invalid direct fragment; final compiler/registry diagnostics. | Rewrite persisted JSON directly; no loader conversion. | Delete old JSON and `unknown_strategy_override` assertion; preserve registry invalidity coverage with final diagnostic. | None.
| `src-tauri/tests/source_live_check.rs:19,285-307,419-449` | Includes shared old fixture; expects dedicated `source_overrides` fingerprint and stale transition after old acceptance change. | T4b canonical schema-v3 fingerprint suite: base profile, direct specialization, Effective Source Profile, provenance, Source Config, selected path/bindings/version tail. | Rewrite to final compiler/fingerprint components and direct specialization mutation. | Delete `source_overrides` fingerprint expectation and old JSON mutation. Prove the old kind is absent and final `direct_source_specialization`/other landed kinds change correctly. | Exact T4b component names follow landed canonical implementation; accepted design currently names `direct_source_specialization`.
| `src-tauri/tests/schema_validation.rs:42-54,92-113` | See schema table; accepts shared old positive and rejects old transforms shape. | Final schema-v3 hard-cut tests. | Rewrite/replace cases. | Old schema registry and positive names gone; old negatives classified. | None.
| `src/features/sources/tests/source-form/source-config-contract-tests.ts:153-201` | Unit/static contract for old raw parser, schema export, and starter generation. | Final direct-specialization model/schema UI contract. | Replace wholesale with final fragment schema and builder tests. | Delete old helper/ref/assertions. | None.
| `src/features/sources/tests/source-form/source-create-contract-tests.ts:292-316,338-414` | Proves Create payload, dirty tracking, and invalid raw old text. | Final Create contract. | Build/dirty/error assertions use final specialization. | Delete old field/string/assertions; retain analogous final payload/dirty/error behavior. | None.
| `src/features/sources/tests/source-form/source-edit-contract-tests.ts:10-23,69-80,120-164` | Fixture contains old field; proves draft round-trip, removal, dirty state, and object validation. | Final Edit contract. | Rewrite fixture and assertions to final direct fields. | Delete every old property/state/error string. | None.
| `src-tauri/src/app/commands.rs:1015-1054,1082-1104` | Command persistence round-trip helper currently supplies `source_overrides: None`; does not test a non-empty old persisted field. | Final Source persistence round-trip. | Construct final Source shape; ideally cover non-empty Direct Source Specialization through real file round-trip. | Remove old member; no compatibility reader. | Current command test has no non-empty specialization persistence proof.
| `src-tauri/resources/sources/.gitkeep` | Confirms no tracked built-in Source documents exist. | No replacement content required. | None. | No deletion. Repository search proves no resource Source old assumption. | User-created app-data Sources are external and unenumerated.
| `src-tauri/migrations/20260609000000_current_schema.sql` and all DB code (searched) | No `sourceOverrides`/`strategyOverrides`; Sources are not persisted in SQLite under this model. | Existing filesystem registry persistence. | No DB migration for old field. Pre-production hard cut rejects/requires re-authoring old files. | No deletion target. Proof: exact repository search has no database hit. | External stale Source files will fail strict load until manually recreated; accepted D-001 explicitly forbids an automatic translator.

## 5. Active conceptual documentation inventory

These are **conceptual prose**, not exact executable symbols. They still belong to activation because they are active domain/architecture/authoring guidance. Historical handoff/issue records are excluded from deletion proof unless used as active guidance.

| Current path / lines | Current conceptual responsibility | Retained final replacement owner | Activation migration / exact proof | Uncertainty |
|---|---|---|---|---|
| `CONTEXT.md:15,17-19,50,58,98` | Canonical domain glossary defines Source Overrides and uses them in Profile Compiler, Diagnostics, and Source Live Check definitions. | Activation documentation owner under D-001/D-013 vocabulary migration. | Replace with Direct Source Specialization, Effective Source Profile and final phase/compiler/live-check language. No active canonical “Source Overrides” definition remains. The lowercase occurrence at line 15 is an `_Avoid_` synonym and must be reconsidered so it does not misleadingly forbid the new concept. | File is pre-existing staged/modified; activation must re-baseline and not overwrite unrelated work.
| `docs/prd/declarative-source-profile-dsl.md:13,36,100,123,127-131,205,227` | Older normative PRD describes wrapper overlays as accepted model. | Activation docs slice updates or explicitly supersedes stale clauses using canonical #166 PRD. | Remove/supersede every listed clause; no active authoring instruction for old wrapper. | Decide edit-vs-supersession form without changing accepted semantics.
| `docs/adr/0001-source-config-as-json-schema.md:1,5` | ADR title/body separates Source Config from old Source Overrides. | Activation ADR migration: preserve separation of Source Config from behavior specialization, rename/model final Direct Source Specialization and Effective Schema semantics. | Update/supersede title/body atomically with behavior. Search active ADRs and review residual history. | ADR status format is minimal; final change may amend or supersede.
| `docs/adr/0009-declarative-source-profile-dsl.md:5,7` | ADR states runtime comes from old selected profile/config/overrides and catalogs Source Overrides as primitive vocabulary. | Activation ADR migration to final direct specialization and schema-v3 phase language. | Update/supersede clauses; no current old model claim. | None.
| `docs/source-profile-production-agent.md:175` | Active agent guidance says old override changes stale a live check. | Activation authoring/operations docs owner, backed by T4b canonical fingerprints. | Say direct Source specialization/effective behavior changes stale the report; no old authoring term. | File is pre-existing staged/modified; re-baseline.
| `docs/prd/declarative-profile-strategy-algebra.md:11` | Historical/current-gap sentence says current Source Overrides are narrower. | Canonical #166 PRD may retain only as unmistakable historical comparison. | Classify explicitly as historical at activation or rewrite after cut so it cannot be read as active authoring. | File is staged/modified; re-baseline.
| `docs/prd/declarative-profile-strategy-algebra.md:235` | Acceptance rule requires no old operation-list Source Override model. | Retain as hard-cut proof, optionally use exact serialized names in search documentation. | This is conceptual rejection/history, not residue to delete. Reviewer classifies it as allowed hard-cut language. | None.
| `docs/profil source algebra refactor.md:1510-1540` | Historical design document lists old schema/document/compiler files as hard-cut deletions and describes direct root fragments. It does not use exact “Source Overrides” phrase in the searched excerpt. | Historical record only. | Leave unchanged if clearly historical, per accepted activation guidance. Do not treat paths as live exports. | Filename/content is staged/modified; classification should be preserved.

No old conceptual references were found in `README.md`, `AGENTS.md`, `docs/dev-search-run-smoke.md`, source-evidence docs, JSON profile resources, scripts, or `search-run-result.json` by exhaustive case-insensitive search.

## 6. Fingerprint/freshness hard cut

Current implementation (`src-tauri/src/checks/source_live/mod.rs:324-349`) emits, in order:

1. `live_check_logic`;
2. `source_document` (already transitively includes old field);
3. `source_config`;
4. optional dedicated `source_overrides`;
5. selected `source_profile_document`.

The current freshness comparator is generic and need not understand the old name. The hard cut is nevertheless atomic:

* T4b foundation creates canonical schema-v3 fingerprints before activation, non-productively.
* Activation makes T4b the sole implementation and removes both the old dedicated `source_overrides` component and the old whole-document freshness assumptions that conflict with canonical components.
* There is no pre-v3 fingerprint migration or equivalence mapping. Existing reports may naturally become stale/unexpected under the new component set; no translator is introduced.
* Required proof: final Source Live Check tests cover direct specialization, Effective Source Profile, provenance/config/selector and fresh/stale transitions; `rg 'source_overrides'` has no production hit; old report fixtures are not maintained as a compatibility pair.

## 7. Complete production-caller migration map

| Productive surface | Current route | Activation route | Same-slice deletion proof |
|---|---|---|---|
| Source Create | Create drawer → old editor/helpers/model → TS DTO → `create_source` | Final schema-guided Direct Source Specialization → final Source DTO → same command | No old component/helper/property/import/copy; UI contract + typecheck.
| Source Edit | Edit drawer/hook/model reads and rewrites old JSON | Read/write only final direct fragments | No on-load translator, legacy panel or dual save field.
| Source Details | Old wrapper preview and schema ref | Final authored specialization/effective information | No old property/ref/title.
| Registry loading | Serde `SourceDocument` → compiler validation | Strict schema-v3 Serde → final compiler | Old field rejected; no version dispatcher/alias.
| Source validation | Snapshot/status mutation → old compiler | Authoritative Source + immutable registry → final compiler | Old facade/status workaround absent.
| Search Run | Selection compiles old model to plan | Lifecycle admission then final compiler; runtime gets immutable plan only | Search Run imports/call graph contain no old compiler/specialization route.
| Lazy posting Detail/UI | Posting service recompiles old model | Final compiler/Detail plan | Old compiler call absent; lazy behavior regression passes.
| Source Live Check | Validation + old compiler + old fingerprints | Final compiler + sole T4b canonical fingerprints | No old compiler route/fingerprint kind.
| Tauri commands/filesystem | Typed old Source written/read | Typed final Source written/read | Old JSON strict rejection; no DTO/translator.
| Tests/fakes | Shared old fixture and struct/JSON mutations | Final fixture and public final compiler/UI seams | No positive old fixture; negatives classified solely as rejection proof.

`compile_source_execution_plan` has exactly four production call sites today: Source validation, Source Live Check, Search Run source selection, and lazy posting Detail (`rg` proof). Registry calls it indirectly through Source validation. Every one is listed above.

## 8. Atomic activation checklist and deletion proof

Activation is ready only after the D-011 foundations support all of these without exposing a second productive authored route:

1. existing keyed Access Path/Strategy specialization;
2. complete new Strategies/Access Paths;
3. deterministic keyed recursive merge and whole non-keyed array replacement;
4. Effective Source Config Schema/shared validator;
5. mandatory Policy;
6. final `detection`/`discovery`/`detail` names;
7. complete provenance;
8. canonical schema-v3 fingerprints.

Then one slice must:

* switch Source schema, Rust and TS documents, Create/Edit/Details UI, commands/registry, validation, all four productive compiler caller sites, Source Live Check and fingerprints;
* rewrite positive fixtures/examples/tests directly to schema v3;
* delete dedicated old files and every old export/helper/diagnostic/fingerprint;
* reject old JSON strictly, with no migration action;
* update active domain/PRD/ADR/agent prose;
* rebuild or remove ignored `dist` residue.

Minimum static proof after activation:

```bash
rg -n 'sourceOverrides|strategyOverrides|SourceOverrides|StrategyOverride|OverridableStep|source_overrides|apply_source_overrides|EffectiveAccessPathSteps' \
  src-tauri/src src-tauri/resources src --glob '*.{rs,json,ts,tsx}'
# Expected: no hits.

rg -n 'sourceOverrides|strategyOverrides|SourceOverrides|StrategyOverride|OverridableStep|source_overrides' \
  src-tauri/tests src/features/sources/tests --glob '*.{rs,json,ts,tsx}'
# Expected: only explicitly classified negative hard-cut rejection fixtures/assertions.

rg -ni '\bSource Overrides?\b|old operation-list Source Override model' CONTEXT.md README.md AGENTS.md docs --glob '*.md'
# Expected: no active guidance; only reviewed historical/rejection comparisons.

rg -n 'ProfileCompilerSnapshot|CompileSourceExecutionPlanResult|compile_source_execution_plan' \
  src-tauri/src src-tauri/tests --glob '*.rs'
# Expected: no hits after final compiler caller migration.
```

Behavioral proof must include focused schema/Serde, compiler resolution/semantic/security, registry, Source Live Check/freshness, Search Run, lazy Detail, three built-in profile regressions, frontend source form contracts, and `npm run build`; then full Rust tests. No network-dependent default test is required.

## 9. Residual uncertainties and risks

1. **Final symbol/file names are not landed.** This inventory names responsibility and final behavior, not a premature filename. The activation owner must re-baseline against the actual serial foundation.
2. **External app-data is not enumerable.** Repository production resources contain no Source JSON, but users may have schema-v2 files under their app-data directory. Accepted D-001 says there is no compatibility migration; strict rejection/manual recreation is the expected hard-cut behavior.
3. **Generated `dist/` is stale but ignored.** It contains the old productive UI vocabulary. It must be regenerated/removed for a clean working-tree search, but is not a tracked source edit.
4. **Pre-existing staged changes exist.** They include `CONTEXT.md`, canonical #166 PRD, historical design, and production-agent guidance. Any later activation must re-baseline and preserve unrelated edits. This inventory did not alter or unstage them.
5. **Production schema enforcement is primarily Serde at registry load.** JSON Schema is enforced by schema tests and frontend guidance; final schema/Serde parity must remain explicit.
6. **Create base-change reset semantics.** Current UI clears old overrides when profile/path/detection changes. Final direct fragments should not be silently translated to a new base; preserve explicit clearing/re-authoring unless an approved final UI behavior says otherwise.
7. **No command test currently persists a non-empty old specialization.** Final activation should add a real final Source document filesystem round-trip so persistence is not inferred solely from Serde.
8. **Conceptual history classification requires review.** The canonical PRD’s current-gap and hard-cut sentences are allowed only as clear history/rejection proof, while older PRD/ADRs are active stale guidance and must be updated/superseded.

## 10. Meta-prompt handoff contract

**Goal:** Plan the final schema-v3 activation/hard-cut owner so every inventory row moves directly to the retained Direct Source Specialization/compiler/fingerprint interfaces and every exact old path is deleted in that same slice.

**Context/evidence:** Use sections 3–7 above. The current old behavior is centralized in `compiler/overrides.rs`, but its authored surface spans Rust/Schema/TS/UI/filesystem documents, four productive compiler caller sites, live-check fingerprints, tests, and active docs. D-001 forbids wrappers/translators/dual productive paths. D-011 requires all serial foundations and T4b canonical fingerprints before activation.

**Success criteria:** Every exact symbol/file/property has one deletion action; every productive caller uses only the final compiler; positive authored surfaces are schema-v3 direct fragments; old JSON is strictly rejected; final fingerprints are sole; active docs use final terminology; focused/full validations pass; repository searches have only classified negative/history hits.

**Hard constraints:** Do not reopen D-001–D-013; do not create a compatibility DTO/alias/forwarder/translator; do not expose pre-activation foundations as a second productive authoring/execution path; do not migrate Search Request criteria into Source/Profile specialization; runtime receives immutable typed plans only.

**Suggested approach:** Re-baseline after each serial foundation lands, keep foundation modules final and non-authorable, then execute one bounded cross-stack activation using this inventory as a deletion checklist. Preserve behavior-specific tests by rewriting them through the final public compiler/UI seams rather than retaining old implementation tests.

**Validation:** Run the static proofs in section 8, targeted Rust suites (`schema_validation`, compiler suites, registry, Source Live Check, Search Run/posting Detail, built-in profiles), frontend source UI contracts, `npm run build`, then full `cargo test`.

**Stop/escalation:** Stop if any foundation is incomplete, if a proposed change requires an old/new productive route, or if external-file automatic migration is requested; that would reopen accepted D-001/D-011 and needs explicit decision escalation. Ordinary final naming/layout choices should follow landed foundation evidence and do not justify reopening decisions.

**Resolved assumptions:** No repository Source resource needs data migration; old app-data receives no translator; source-owned access remains distinct; `dist` is generated ignored residue; conceptual historical/rejection prose is not an executable residue but must be explicitly classified.

## Acceptance report

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced only the configured read-only inventory artifact; no project/source or GitHub state was modified and D-001/D-011 scope was not widened."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Inventory includes exact files/symbols and lines, productive call chains, final owners, activation actions, deletion proofs, tests/fixtures/docs/persistence, exhaustive search classification, validation plan, and residual risks."
    }
  ],
  "changedFiles": [
    "/tmp/job-radar-166-phase2/source-overrides-inventory.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short; find repository files; exhaustive rg searches for sourceOverrides/strategyOverrides and Rust/TS variants",
      "result": "passed",
      "summary": "Established pre-existing staged baseline and enumerated exact old symbols across source, tests, fixtures, docs, and generated dist."
    },
    {
      "command": "Call-chain rg/read for compile_source_execution_plan, derive_source_validation_state, SourceDocument, create/update commands, registry, Search Run, posting Detail, and Source Live Check/fingerprints",
      "result": "passed",
      "summary": "Confirmed four direct production compiler call sites plus indirect registry/UI/filesystem routes and no SQLite persistence of Source overrides."
    },
    {
      "command": "Case-insensitive documentation search and JSON/resource/fixture searches",
      "result": "passed",
      "summary": "Classified active conceptual prose, historical/rejection prose, two JSON fixtures, no tracked production Source JSON, and ignored dist residue."
    },
    {
      "command": "Read both required decision handoffs completely and inspect all matched implementation/caller/test/schema/UI files",
      "result": "passed",
      "summary": "Applied accepted D-001/D-011 without reopening D-001–D-013."
    }
  ],
  "validationOutput": [
    "Exact old-symbol source file set was enumerated and mapped; no database migration or tracked resource Source JSON contains the old field.",
    "compile_source_execution_plan production calls found in Source validation, Source Live Check, Search Run selection, and lazy posting Detail; registry reaches it through validation.",
    "Ignored dist/assets/sources-page-CoFLO4jB.js contains 43 sourceOverrides, 3 strategyOverrides, and 6 Source Overrides occurrences and is explicitly classified as generated residue.",
    "No project test was run because this task is a read-only inventory; validation consisted of exhaustive repository search and call-chain reads."
  ],
  "residualRisks": [
    "Final foundation symbols/files are not landed and require activation-time re-baseline.",
    "External app-data Source JSON cannot be inventoried and will receive no compatibility migration under D-001.",
    "Repository has unrelated pre-existing staged files, including active docs that later require careful re-baseline.",
    "Review gate is still required."
  ],
  "noStagedFiles": false,
  "diffSummary": "Added one out-of-repository /tmp analysis artifact only; project working tree was left untouched. Pre-existing staged files remain.",
  "reviewFindings": [
    "no blockers in the inventory itself",
    "review-required: independently verify every row and classification before using it to size the atomic activation"
  ],
  "manualNotes": "noStagedFiles is false because staged files predated this read-only task; none were created, modified, unstaged, or attributed to this work."
}
```
