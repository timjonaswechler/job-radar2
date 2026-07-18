# T11c — Share evidence-backed value Primitives and compile typed value contexts

## Result

Discovery and Detail compile every admitted Field Expression into an immutable typed value plan and execute it through exactly one canonical owner per authored value type. Static context mistakes fail in the Profile Compiler, and the value family gains a bounded, deterministic, scalar `first_non_empty` expression without changing existing non-fallback output semantics.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#180 — T11b — Share evidence-backed select Primitives](https://github.com/timjonaswechler/job-radar2/issues/180).
- Blocking: [#193 — T12a](https://github.com/timjonaswechler/job-radar2/issues/193) and [#205 — T14a](https://github.com/timjonaswechler/job-radar2/issues/205).
- Readiness: **Blocked** by #180.
- Open decision: none. The fallback contract, capture-source hard cut, and immutable expression limits are approved.

At readiness review, re-baseline paths, test targets, typed selector/document plans, Diagnostics, and phase operations against the exact #180-landed tree. Stop for design review if the landed implementation would require a second value/document model, duplicate selector traversal, a public per-Primitive API, a compatibility wrapper, or T14a-owned Detection integration.

## Consumed contracts

- #166 / `docs/prd/declarative-profile-strategy-algebra.md`, especially Decisions 7, 22–23, 29–30, and 37–40: typed phase contexts and immutable plans, provider-neutral shared Primitives, one canonical implementation owner, registry completeness, and schema-v3 hard cuts.
- The PRD’s Strategy Set Runtime decision: typed Discovery and Detail operations remain caller-facing; phase adapters retain phase-specific inputs, outputs, reducers, Diagnostics, budgets, and Cancellation.
- #180 supplies the family-qualified Primitive registry and validator, shared parsed documents and selected items, compiled selector plans, canonical JSON/XML/CSS traversal, placement compatibility checks, and typed Discovery/Detail phase operations.
- The directly supplied Source remains authoritative through `compile_source`; profile-based compilation exposes the Effective Source Profile, Source-owned access remains distinct, and runtime receives typed immutable plans rather than authored JSON.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

## Current gap

This section describes the blocked ticket’s current pre-#180 repository and is provisional until readiness review.

- `src-tauri/src/profile_dsl/documents/extract.rs` defines `Cardinality::{One, First, Optional, All}`, twelve `FieldExpression` variants, `ListFieldExpression`, and recursive `CombinePart`. `src-tauri/src/schema/profile-dsl/extract.schema.json` repeats the twelve-key union. No `first_non_empty` shape exists.
- `FieldExpression::Const` stores arbitrary `serde_json::Value`, although the authoritative schema admits only string, number, or boolean constants.
- Field Expressions occur in Discovery and Detail outputs, `select.rs` filters and capture sources, and Detail matching. `src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs` and `src-tauri/src/profile_dsl/execution_plan/posting_detail.rs` clone authored expressions into plans rather than compiling value behavior.
- `src-tauri/src/profile_dsl/compiler/capabilities.rs` and `src-tauri/src/profile_dsl/compiler/templates/*` perform partial recursive checks but do not model all placements as typed contexts. Unavailable values can therefore survive until runtime; Discovery contains a late `unsupported_field_expression` path.
- Discovery and Detail duplicate variant dispatch, scalar conversion, normalization, cardinality, transforms, and `combine` across `src-tauri/src/profile_dsl/runtime/posting_discovery/extract/fields.rs`, `src-tauri/src/profile_dsl/runtime/posting_detail/extract/fields.rs`, and their adjacent `values.rs` modules.
- The phase-local `src-tauri/src/profile_dsl/runtime/posting_discovery/extract/captures.rs` and `src-tauri/src/profile_dsl/runtime/posting_detail/extract/captures.rs` clone the partial capture map before each BTreeMap-ordered capture rule, allowing ordering-dependent references to earlier captures.
- No landed bound covers expression depth, total nodes across one complete effective behavior, and fallback candidate count.

Existing evidence is concentrated in `compiler_semantic_validation`, `schema_validation`, the Discovery/Detail runtime targets, Source Live Check, Search Run/lazy Detail regressions, and the Greenhouse, Workday, and SuccessFactors profile tests.

## Target delta

### Canonical value family and compiled boundary

Create one canonical behavior owner for each value key:

```text
profile_dsl/primitives/value/
  const.rs template.rs source_config.rs posting_meta.rs capture.rs item_field.rs
  json_path.rs xml_text.rs xml_element.rs css_text.rs css_attribute.rs
  combine.rs first_non_empty.rs
```

`value/mod.rs` and the top-level Primitive registry contain registration/dispatch metadata only. Shared sequence, conversion, cardinality, normalization, and typed-context infrastructure may use responsibility-named private support files, but authored-type behavior may not migrate there. The registry’s completeness claim expands only to the thirteen value keys; transforms, filters, captures, outputs, predicates, acceptance, fetch, pagination, and Detection remain outside it.

Exact private names may adapt to #180, but the compiled responsibility is equivalent to:

```rust
enum CompiledValue { /* one plan variant per thirteen value types */ }

struct ValueContext<'a> {
    phase: ValuePhase,
    placement: ValuePlacement,
    source: &'a CompiledSourceValueContext,
    item: Option<TypedSelectedItem<'a>>,
    posting: Option<&'a PostingOccurrenceValueContext>,
    captures: CapturesAvailability<'a>,
}

fn evaluate_value(
    plan: &CompiledValue,
    context: ValueContext<'_>,
    diagnostics: ValueDiagnosticContext<'_>,
) -> ValueOutcome;
```

Every Field Expression placement compiles once. Runtime receives no raw `FieldExpression` or authored JSON. Selector-backed owners call T11b’s compiled traversal directly and own only value lookup/conversion, options, and outcomes.

### Placement-aware contexts

The compiler distinguishes at least these evaluation placements:

| Placement | Available values |
|---|---|
| Discovery capture source | selected Discovery item/document, authoritative Source data, Source Config |
| Discovery filter/output | the above plus the complete strategy capture map |
| Detail capture source | Source, Source Config, concrete posting occurrence, postingMeta; no Detail document or completed captures |
| Detail match/filter/output | selected Detail item/document, Source, Source Config, posting occurrence/postingMeta, completed captures |

Rules:

- `const` is available everywhere; `source_config` and supported `source` template references are available subject to declared-key validation.
- `posting_meta` and `posting` templates are invalid in Discovery and available in Detail when the key belongs to the typed posting occurrence contract.
- Compile-time postingMeta admission remains the union of keys declared across Discovery Strategies. If an admitted key is absent from one occurrence, lookup succeeds with a missing value; the containing placement decides its effect.
- Every direct or nested `capture` expression or `captures` template reference under any `captures.<key>.from` is compiler-invalid. Capture-source compilation receives no partial map. This deliberately deletes sequential BTreeMap capture chaining without warning, alias, or compatibility behavior.
- Captures are otherwise available only after the complete map exists. Item/document-backed expressions are available in Discovery capture/filter/output and Detail match/filter/output, but invalid in Detail capture sources.
- Templates compile against exactly the namespaces available at their placement; transform pipes remain invalid. `combine` and `first_non_empty` recursively validate children in the same context.
- Detail `where[].field` uses the same context as Detail match/output.
- T11b remains authoritative for parse/document compatibility and selector syntax. Detection receives no authored/public value context in this ticket.

An unavailable reference emits an error-severity `compiler` Structured Diagnostic at the exact expression/reference path, includes the Strategy key, and produces no plan. Reuse #180’s equivalent code; otherwise use `value_unavailable_in_context` with `{valueType, phase, placement}` and, where relevant, `{namespace, key}`. Do not retain aliases for superseded template-only responsibilities.

### Authored and compiled `first_non_empty`

```json
{
  "type": "first_non_empty",
  "candidates": [
    { "type": "css_text", "selector": "h1.primary", "cardinality": "optional" },
    { "type": "css_text", "selector": "h1.fallback", "cardinality": "one" }
  ],
  "transforms": [{ "type": "normalize_whitespace" }]
}
```

The wrapper requires one or more complete Field Expressions in `candidates`, admits optional `transforms`, and does not admit `cardinality`. Unknown fields, explicit `null`, an empty candidate array, wrapper cardinality, and a direct nested fallback are rejected consistently by schema and Serde wherever shape validation can decide them. Every candidate is scalar: `cardinality: all` is compiler-invalid with its Diagnostic at `/candidates/<index>/cardinality`. A fallback candidate may be any pre-existing expression, including `combine`, but its subtree may not contain another `first_non_empty`; the compiler recursively rejects nesting at the nested candidate's `type` path when document-shape validation cannot express it. A fallback may itself be a child of a non-fallback composition.

Evaluation is exact:

1. Evaluate candidates sequentially in authored order, including each child’s lookup/traversal, scalar conversion, cardinality, transforms, trim, and whitespace normalization.
2. A successful missing, empty, or whitespace-only child continues without a fallback failure. Numeric `0` and boolean `false` become non-empty strings and win.
3. Any hard child failure—including conversion, transform, dynamic-shape, or cardinality failure—aborts immediately. Preserve the child Diagnostic/path; do not execute later candidates or wrapper transforms.
4. The first non-empty child wins. Apply wrapper transforms exactly once after selection. Wrapper failure is hard at the wrapper expression/transform path and does not backtrack.
5. If all children are empty, return a successful empty scalar outcome; required/optional/filter/match/capture placement semantics remain authoritative.
6. Preserve candidate index, Strategy key, landed phase path, and evaluation-order Diagnostic ordering. Fallback performs no I/O, retry, parallelism, Strategy fallback, or refetch.

### Immutable expression bounds

Enforce the approved backend ceilings before typed recursive compilation and independently after Effective Source Profile merge (or across the complete Source-owned Access Path):

| Dimension | Ceiling | Counting scope |
|---|---:|---|
| Field Expression depth | 16 | root is depth 1; each nested expression edge adds one |
| Total Field Expression nodes | 1,024 | every root and nested node across every placement in the complete effective behavior for one Source |
| Candidates per fallback | 16 | direct `candidates` entries in each wrapper; schema includes `maxItems: 16` |

Profiles and Source specialization may tighten but not raise these ceilings. Use `value_expression_depth_exceeds_limit`, `value_expression_nodes_exceed_limit`, and `first_non_empty_candidates_exceed_limit` unless #180 lands exact equivalents. Diagnostics identify the offending root or `/candidates`, include actual/maximum counts and Strategy key where applicable, contain no resolved values, and produce no partial plan. Every dimension needs deterministic exact-boundary and one-over tests at document loading and post-merge compilation.

### Const parity, preservation, and data minimization

Replace or constrain arbitrary authored `serde_json::Value` constants with a typed scalar admitting exactly string, JSON number, or boolean. Schema, direct Serde, compiler, and runtime reject `null`, arrays, and objects.

Preserve existing `combine` order, required/optional-part behavior, joining and wrapper transforms; transform-before-trim/normalization order; scalar conversion; non-fallback cardinality/list behavior; and Discovery location aggregation. Phase adapters continue to own output mapping, required-field failures, location deduplication, item indices, capture regex application, Detail filtering/matching, acceptance, reducers, budgets, and Cancellation.

Compiled plans retain only identifiers, declared keys, selector plans, cardinality, transforms, and authored constants. Resolved Source Config, postingMeta, captures, selected-document text, and provider responses pass ephemerally through typed runtime contexts and never enter serializable/debuggable plans, registry descriptors, Diagnostics, logs, Check Reports, or derived persistence. Diagnostics may retain paths, identifiers, counts, cardinality, and sanitized errors.

## Dependency and deletion decision

Value plans, contexts, fallback, cardinality/composition, registry metadata, and transform reuse are concrete in-process code; no value/context/plugin trait or speculative adapter is introduced. T11b parsed documents and selector engines are called directly. Existing HTTP/browser seams remain outside value evaluation, and SQLite is unchanged.

**Deletion test:** Without the value-family boundary, context lookup, scalar conversion, cardinality, transforms orchestration, composition, fallback, and Diagnostics would spread back across Discovery and Detail phase/placement callers and later Detection. A forwarding-only family does not pass.

## Examples

1. **Ordered fallback:** preferred title normalizes to whitespace and title normalizes to `Senior Engineer`; the second wins and later candidates do not run.
2. **Hard failure:** candidate 0 produces two values under `one`; its cardinality Diagnostic at `/candidates/0` aborts evaluation, even if candidate 1 would succeed.
3. **Wrapper failure:** candidate 0 wins, then a wrapper regex transform fails; no later candidate is tried.
4. **Context rejection:** `posting_meta.jobId` fails in Discovery but compiles in a valid Detail placement. A selector-backed value fails in a Detail capture source because the Detail document is not yet selected.
5. **Cancellation/bounds:** Cancellation before the containing phase prevents value work through the existing typed path. Value evaluation creates no Resolution Partial Completion; depth 17, node 1,025, or candidate 17 is rejected before execution.

## Scope

- Add the thirteen canonical value owners, value-family registrations, real schema/Rust/compiled-registration parity, and synthetic missing/duplicate registry cases.
- Add matching typed/schema fallback and scalar-const documents plus schema/Serde parity.
- Enforce all three bounds at loading and post-merge compilation.
- Compile every Discovery/Detail Field Expression placement, including capture sources, filters, Detail `where`, matching, outputs, locations, and postingMeta.
- Replace partial template/capability checks with placement-aware context compilation while preserving T11b compatibility checks.
- Route Discovery and Detail directly through the shared evaluator and migrate Search Run, Source Live Check, and lazy Detail only as required by the landed typed interface.
- Delete authored-expression cloning, duplicate phase-local switches/converters/cardinality/normalization/`combine`/template wrappers, late static-context failures, partial-capture chaining, duplicate traversal, forwarding wrappers, aliases, compatibility registrations, and superseded tests after equivalent caller-facing coverage exists.
- Update deterministic fixtures only where typed plans, earlier static rejection, const parity, bounds, or fallback require it.

## Adjacent non-goals

- Detection value integration belongs to T14a/#205; this ticket adds no public Detection shape or placeholder.
- PostingOccurrence/provider-value/hint semantics and requested Detail/reducers belong to T12a/#193 and T12b/#195.
- URL-component selection, selector-language expansion, new transform types/family registration, and a general scalar/list/cardinality redesign.
- Hard-failure recovery, refetching fallback, Strategy-policy changes, parallel evaluation, Candidate Resolution, persistence/status changes, or resumability.
- A general expression language, scripts, public per-Primitive APIs, dynamic registries, or context ports.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Twelve existing types | Each valid type compiles to and executes through one canonical owner | Table-driven external compiler and phase tests |
| Ordered/all-empty/false-zero fallback | First non-empty wins; exhausted fallback is successful empty; `0`/`false` win | Discovery and Detail operation tests |
| Child options and failures | Child transforms/cardinality complete before selection; empty continues; hard failure aborts with child path | Cross-phase external tests |
| Wrapper options | Transforms run once after selection; failure is reported at the wrapper path and does not backtrack | Cross-phase external tests |
| Invalid fallback document shape | Empty/null/unknown/wrapper-cardinality and direct nested-fallback shapes fail schema/direct Serde | Schema/Serde parity tests |
| Invalid fallback semantics | Candidate `all` fails at `/candidates/<index>/cardinality`; recursively nested fallback fails at its nested candidate `type` path; no plan | External compiler tests |
| Bounds | 16/1,024/16 pass and one-over fails, at both enforcement layers, with no partial plan | Loading and post-merge compiler tests |
| Const parity | String/number/boolean pass; null/array/object fail schema and direct Serde | Table-driven parity tests |
| Discovery context | Direct/nested postingMeta or posting template is rejected at its exact path | External compiler tests |
| Detail posting context | Valid postingMeta/posting templates compile in Detail capture, match, filter, and output; unavailable/unknown references reject | Table-driven compiler and typed Detail tests |
| Detail capture document access | Direct/nested `item_field`, JSONPath, XML, and CSS values reject because the Detail document is unavailable | Table-driven external compiler tests |
| Detail `where` | Direct and nested valid/invalid values receive the same context admission and deterministic behavior as Detail match/output | Table-driven compiler and typed Detail tests |
| Source Config keys | Known/unknown keys are tested through direct, template, `combine`, and fallback expressions; unknown rejects at the exact child path | Table-driven external compiler tests |
| Completed capture keys | Known/unknown keys are tested in filters, outputs, and fallback; unknown rejects before runtime | Table-driven external compiler tests |
| Capture-source hard cut | Direct/template/nested capture references fail in both phases; no ordered partial-map chaining remains | Table-driven compiler tests and search |
| postingMeta union | Union-declared key compiles; absence on one occurrence is successful missing | External Detail test with two Discovery shapes |
| Selector delegation | JSON/XML/CSS values use T11b traversal; no second parser/traverser exists | Phase regressions and ownership search |
| Existing semantics | `combine`, transforms, scalar/list cardinality, locations, field mapping, matching, Diagnostics and ordering remain stable | Discovery/Detail and profile regressions |
| Cancellation/budget | Existing terminal/usage order remains; no later work and no persistable Partial Completion | Strategy Set/Search Run regressions |
| Registry/ownership boundary | Exact thirteen-key parity; synthetic missing/duplicate fails; one file owner per key; no later-family claim | Registry tests, filesystem inventory, reviewed searches |
| Data minimization | Sentinel runtime/provider values do not occur in plans, metadata, Diagnostics, logs, reports, or derived persistence | Compiler/phase/report tests and log/search review |
| Production callers | Source Live Check, Search Run Discovery, and lazy Detail preserve behavior except earlier static rejection | Existing caller regressions |
| Acceptance profiles | Greenhouse, Workday, SuccessFactors outputs and Diagnostic order remain generic and stable | Existing profile targets |
| Deletion | No raw authored expression, phase-local duplicate, selector duplicate, context fallback, capture chaining, or compatibility path remains | Reviewed static checks |

Tests cross `compile_source` and the landed typed Discovery/Detail operations. They use real compiler/registry/value logic and existing deterministic HTTP/browser adapters; private tests are limited to narrow conversion/fallback edges not economically visible through these interfaces.

### Focused commands

Re-baseline target names after #180, then run the landed equivalents:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test primitive_registry
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test posting_discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test posting_detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting

find src-tauri/src/profile_dsl/primitives/value -maxdepth 1 -type f -name '*.rs' -print | sort
rg -n 'FieldExpression|CompiledValue|unsupported_field_expression|raw_field_values|evaluate_string_field|combine_field_values' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'
rg -n 'first_non_empty|FirstNonEmpty|MAX_.*EXPRESSION|expression.*(depth|nodes|limit)|constExpression|AuthoredScalar' src-tauri/src/profile_dsl src-tauri/src/schema src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'CaptureRule|context_captures|captures\.clone|FieldExpression::Capture|namespace.*captures|where.*field' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'
rg -n 'resolve_simple_json_path|Matcher::new|xml_path_|descendants\(\)|legacy|compat|placeholder|forward|url_component' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'
rg -n 'SourceExecutionPlan|source_config|posting_meta|captures|response|body|document|diagnostic|tracing::|log::' src-tauri/src/profile_dsl src-tauri/src/checks src-tauri/src/search src-tauri/tests --glob '*.rs'
rg -n 'greenhouse|workday|successfactors|profile_key|source_key.*(match|==)' src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives --glob '*.rs'
```

Full-suite and frontend requirements follow the shared delivery contract; the frontend build applies because authored schemas/serialized contracts change.

## Ticket-specific migration items

- [ ] Add the thirteen canonical value files and value-only registrations; prove real parity and synthetic missing/duplicate rejection.
- [ ] Add typed `first_non_empty`, scalar const, schema/Serde parity, and both-layer exact-boundary/one-over limit coverage.
- [ ] Compile every Field Expression placement into a typed plan and remove authored expression clones from execution plans.
- [ ] Replace partial template/capability checks with the four placement contexts, including Detail `where` and capture-source rejection.
- [ ] Delete BTreeMap partial-capture-map cloning and all direct/nested capture-source chaining behavior.
- [ ] Route selector-backed values only through T11b traversal and delete remaining value-local selector helpers.
- [ ] Migrate Discovery, Detail, Source Live Check, Search Run, and lazy Detail callers/tests to the landed compiled interfaces.
- [ ] Delete duplicated phase-local value switches, converters, cardinality/normalization/transform orchestration, `combine`, template wrappers, late unavailable-context branches, forwarding aliases, compatibility registrations, and superseded tests.
- [ ] Inspect serialized/debug plans, metadata, Diagnostics, logs, reports, and derived persistence with sentinel values; classify every retained value/path.
- [ ] Run and classify every hit from the focused ownership, duplication, compatibility, and provider-dispatch searches.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
