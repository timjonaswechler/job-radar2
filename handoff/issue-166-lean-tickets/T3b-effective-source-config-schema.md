# T3b — Define and specialize one Effective Source Config Schema contract

## Result

The Effective Profile Compiler and Profile Detection use one constrained, evidence-backed Effective Source Config Schema implementation. Direct Source specialization merges schema fragments predictably; invalid definitions reject compilation or make a reusable Source Profile ineligible for Detection; and the same compiled contract validates saved Source Config and Detection proposals with context-correct Structured Diagnostics. Existing Greenhouse, Workday, and SuccessFactors constraints remain enforceable.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#169](https://github.com/timjonaswechler/job-radar2/issues/169).
- Blocking: [#171](https://github.com/timjonaswechler/job-radar2/issues/171) and [#172](https://github.com/timjonaswechler/job-radar2/issues/172).
- Readiness: **Blocked**; #169 is open, and this ticket must be re-baselined against its landed compiler, fragment types, validation order, diagnostics, and tests before assignment.
- Open decision: none.

## Consumed contracts

- #166 / PRD Decision 47 defines the selected constrained schema language, specialization rules, shared validation responsibility, and diagnostic-category split.
- #166 / PRD Decisions 12–21 and the “Effective Profile Compiler” module decision keep `compile_source(&SourceDocument, &SourceProfileRegistrySnapshot)` as the one compiler entry point, make the direct Source authoritative, validate the complete Effective Source Profile before Source Config and selected-path resolution, and expose only an immutable typed Execution Plan to runtime.
- T3a/#169 provides the final typed direct Source-fragment model, complete Source-added Strategies and Access Paths, deterministic append order, and complete validation of every effective Access Path—including unselected additions—under existing security and boundedness rules.
- T3a adds no direct Source Config Schema fragments. T3b extends its landed model and preserves its profile/path composition rule: property declarations and requirement sets combine across locations, `additionalProperties: false` at either location closes the composed contract, and the same property declared independently at both locations is an error.
- `handoff/issue-166-delivery.md` owns shared readiness, hard-cut, testing, migration, deletion, Definition-of-Done, and PR-evidence rules.

## Current gap

This baseline is provisional while #169 remains open. Today `JsonSchemaObject` is a raw `serde_json::Map<String, Value>` (`profile_dsl/documents/support.rs`), and profile-, Access-Path-, and Source-owned documents store that raw shape. `schema/profile-dsl/common.schema.json#/$defs/jsonSchemaObject` accepts any non-empty object.

Backend semantics are duplicated and incomplete:

- `profile_dsl/compiler/source_config.rs` reads `properties`, string `required` entries, boolean `additionalProperties`, and scalar property `type`. Malformed or unsupported shapes are treated as absent, and unknown property types match. Saved-value failures are currently emitted as `compiler` rather than `source_validation`.
- `source_profile/detection/proposal.rs` separately implements required/allowed properties, type, JSON-equality enum, and Rust-regex pattern checks with `detection` diagnostics. The two implementations can drift.
- `profile_dsl/compiler/resolution.rs` validates profile/path composition, including property redefinition, but no typed compiled schema contract owns complete definition validation.
- Registry loading retains Source Profile documents with compiler diagnostics. `app/commands.rs::detect_source_proposal_from_url_with_clients` passes all retained profile documents to Detection, so an invalid schema definition can reach Detection and I/O.

Frontend schema introspection remains broader and intentionally non-authoritative. `source-config-schema.ts`, `schema-introspection.ts`, and `config-value-control.tsx` use `title`, enum choices, URI format, and constraint hints; `source-config-contract-tests.ts` proves pattern, enum, and unknown-property enforcement is left to the backend.

Built-in evidence is active: Greenhouse uses `pattern` and `title`; Workday uses patterns, `format: "uri"`, and titles; SuccessFactors uses URI formats, numeric `minimum`, and titles. Generic Detection/UI tests exercise `enum`. Relevant coverage is spread across schema/Serde, compiler, registry, Detection, Source Live Check, the three profile regressions, and Source UI tests.

## Target delta

The public compiler interface and `CompiledSource` responsibilities do not change. Exact private names may follow landed T3a code, but there is one implementation equivalent to:

```rust
pub(crate) fn compile_source_config_schema_location(
    schema: Option<&JsonSchemaObject>,
    path: &str,
) -> Result<CompiledSourceConfigSchema, Vec<SchemaDefinitionViolation>>;

pub(crate) fn compose_source_config_contract(
    profile: Option<&CompiledSourceConfigSchema>,
    access_path: Option<&CompiledSourceConfigSchema>,
    paths: SourceConfigSchemaPaths,
) -> Result<CompiledSourceConfigContract, Vec<SchemaDefinitionViolation>>;

pub(crate) fn validate_source_config_value(
    contract: &CompiledSourceConfigContract,
    value: &JsonObject,
) -> Vec<SourceConfigValueViolation>;
```

These private immutable types contain the accepted subset and neutral violations; they do not know compiler/Detection result types, UI, Tauri commands, registry DTOs, or phases.

### Accepted authored language

```text
RootSchema:
  type?: "object"
  properties?: object<string, PropertySchema>
  required?: string[]
  additionalProperties?: boolean

PropertySchema:
  type: "string" | "number" | "integer" | "boolean" |
        "object" | "array" | "null"
  pattern?: string
  enum?: scalar[]
  format?: "uri"
  minimum?: number
  title?: string  // reusable Source Profile authoring only
```

Definition invariants:

1. Optional root `type` is exactly scalar `"object"`; type arrays and other values are invalid. `properties` is an object whose values are objects. Every complete property has exactly one recognized scalar `type`. `additionalProperties` is boolean.
2. `required` contains unique strings. After same-location specialization and profile/path composition, each entry identifies exactly one composed property declaration; it never declares a property implicitly.
3. `pattern` is a string only on `string`, compiles with Rust `regex`, and uses `Regex::is_match` (authors add anchors for full-string matching).
4. `enum` is a non-empty array of unique JSON scalar values—string, number, boolean, or null—compatible with the declared type. Object/array members and enum on object/array properties are invalid.
5. `format` is only `"uri"` on `string`. Values must be syntactically valid absolute URIs with a scheme; transport/scheme authorization remains compiler/runtime security behavior.
6. `minimum` is an inclusive finite JSON number only on `number` or `integer`.
7. `title` is a non-empty string only on reusable Source Profile properties, including profile-owned Access Paths. It is inherited unchanged, affects authoring labels/inspection only, and never affects concrete value acceptance. Direct Source fragments and Source-owned Access Paths cannot represent, add, or replace it; programmatically constructed invalid input is compiler-rejected.
8. Keyword/type incompatibility is a definition error. Search Request-like property names remain forbidden. Every unlisted root/property keyword or shape—including nested schemas, `items`, `$ref`, `const`, defaults, descriptions, maximum/length bounds, combinators, conditionals, and dependencies—is rejected rather than ignored.

This is not a general JSON Schema interpreter.

### Direct Source specialization and composition

T3b adds profile-level and Access-Path-level `sourceConfigSchema` to T3a’s typed direct Source fragment and matching Source-schema `$def`.

- Merge corresponding root objects recursively. Root `type` and `additionalProperties` replace scalar values.
- Merge `properties` by key. Existing properties retain omitted members and replace supplied executable scalar members (`type`, `pattern`, `format`, `minimum`). `enum` replaces the complete inherited array.
- A Source-added property inherits nothing, has no `title`, and must become one complete valid executable property including `type`.
- `required` replaces the whole array at that authored location; it is never unioned, appended, deduplicated, or merged by index. `[]` removes all requirements at that location, after which composed declaration rules still apply.
- Inherited reusable-profile `title` survives every Source merge unchanged. A fragment introducing a schema where none existed must supply a complete executable schema and cannot contain `title`.
- Structural `null` never deletes a keyword/property. Results and diagnostics are independent of JSON map insertion order; base profile and Source inputs remain unchanged.
- Same-location Source merge precedes profile/path composition. Profile- and path-level schemas remain separate authored locations. During cross-level composition, their property and `required` sets combine; requirements contributed at either level remain required in the composed contract. `additionalProperties: false` at either level closes the contract, and independent duplicate property declarations remain errors. A T3a Source-added Access Path may own a complete executable local schema; a Source-owned Access Path uses the same executable subset. Neither can author `title`.

### Definition/value validation and ordering

Complete Effective Source Profile validation compiles the profile-level schema once and every effective Access Path’s local schema once, then composes each path contract when both contributors are locally valid. An invalid profile-level schema does not suppress independent path-local diagnostics, but a local failure suppresses only dependent composition for that path. Any definition error on an unselected path rejects the complete profile.

Deterministic definition ordering is:

1. profile-local diagnostics once;
2. each effective Access Path in effective order: path-local diagnostics once, then cross-level composition diagnostics when both locations are valid;
3. within a schema: root `type`; `properties` lexicographically; known property keywords in `type`, `pattern`, `enum`, `format`, `minimum`, `title` order; unsupported property keywords lexicographically; `required` in array order; `additionalProperties`; unsupported root keywords lexicographically.

Duplicate `required` entries produce one violation per occurrence after the first; undeclared entries produce one per array entry. Both point to `/required/<index>` and include the property in details.

Concrete validation checks missing required properties lexicographically, then present properties lexicographically. Per value it checks `type`, `pattern`, `enum`, `format`, `minimum`, stopping dependent checks when type fails. One violating check emits one diagnostic.

Definition violations always map to error-severity `compiler` diagnostics. Stable codes include:

- `unsupported_source_config_schema_keyword`, `invalid_source_config_schema_shape`, and `invalid_source_config_schema_type`;
- `invalid_source_config_schema_pattern`, `invalid_source_config_schema_enum`, `invalid_source_config_schema_format`, `invalid_source_config_schema_minimum`, and `invalid_source_config_schema_title`;
- `source_config_schema_title_not_source_specializable`;
- `duplicate_source_config_schema_required_property`, `undeclared_source_config_schema_required_property`;
- existing property-redefinition and forbidden-Search-Request-property codes where applicable.

Stable value codes are `missing_source_config_required_property`, `unknown_source_config_property`, `invalid_source_config_property_type`, `invalid_source_config_property_pattern`, `invalid_source_config_property_enum`, `invalid_source_config_property_format`, `invalid_source_config_property_minimum`, and `forbidden_search_criteria_in_source_config`.

Saved Source values map to `source_validation` at `/sourceConfig/<escaped-property>`; Detection proposal values map to `detection` at `/profiles/<profile-index>/detect/sourceConfig/<escaped-property>`. Definition paths target effective `/sourceConfigSchema/...` or `/accessPaths/<effective-index>/sourceConfigSchema/...`.

Compiler order is Effective Source Profile merge → complete Effective Source Profile/schema validation → reuse the selected path’s compiled contract → saved Source Config validation → selected Access Path resolution → plan. `Compiled` has no error Diagnostic; `Rejected` has no partial effective profile or plan.

Detection compiles the profile-level and every Access Path local/composed contract before evidence acquisition. Any definition error makes the whole reusable profile ineligible: preserve and surface the deterministic compiler diagnostics through the public Detection result, execute zero probes, and produce no proposal. Registry eligibility may gate probes but must not filter away or change those required diagnostics/cardinalities. Otherwise Detection uses the recommended-path compiled contract after bounded evidence/captures and emits a proposal only when its Source Config passes the shared value validator. Detection never applies direct Source specialization. Source-owned compilation uses the same definition/value implementation through its distinct branch.

Schema compilation/value validation is pure in-process work: no Cancellation or Partial Completion/status is introduced, and no trait, adapter, callback, public validator stage, or general-schema library abstraction is added.

## Dependency and deletion decision

Raw schema documents, Effective Profile merge, compiled contracts, regex/URI parsing, and Diagnostic translation are in-process. Compiler and Detection use the real implementation. Frontend introspection remains a separate authoring aid; SQLite and HTTP/browser clients are outside validation, with probes gated after definition eligibility.

**Deletion test:** Without this shared validator, Profile Compiler, Source validation/Live Check, Detection/proposal construction, registry validation, Source-owned compilation, and tests would each need to know supported keywords, applicability, specialization/composition, regex/enum/URI/minimum semantics, deterministic ordering, and category mapping.

## Examples

1. **Recursive specialization:** a base `feedUrl` URI property keeps its inherited `title`; a Source fragment narrows `region.enum`, adds complete `pageSize: { type: "integer", minimum: 1 }`, replaces `required`, and flips `additionalProperties`. Inputs remain unchanged.
2. **Invalid definition:** `{ "type": "string", "minimum": 1 }` emits `compiler/invalid_source_config_schema_minimum` at the keyword path. No saved-value validation, plan, Detection probe, or proposal follows.
3. **Context mapping:** with `region.enum = ["eu"]`, value `"us"` yields `source_validation/invalid_source_config_property_enum` at `/sourceConfig/region` for a saved Source and `detection/invalid_source_config_property_enum` at the proposal path for Detection.
4. **Profile-only title:** inherited `"title": "Greenhouse board slug"` remains inspectable and usable by the UI. Any direct Source replacement/addition or Source-owned title is rejected.
5. **Required cardinality:** `required: ["feedUrl", "feedUrl"]` reports the second entry once; `required: ["feedUrl", "tenant"]` reports `tenant` unless exactly one composed declaration supplies it.

## Scope

- Add Source Config Schema fragments to T3a’s typed direct Source-fragment model and matching Source JSON Schema `$def`.
- Implement the exact definition language, deterministic recursive specialization/composition, immutable inherited titles, typed compiled contract, and neutral definition/value violations.
- Preserve unsupported raw members through Serde/merge until semantic validation; never silently drop them.
- Compile profile-level and every effective path-local/composed contract before saved-value validation or Detection I/O, and privately cache contracts by effective path.
- Move Compiler, saved Source validation, Source Live Check compilation, Detection proposal validation, registry eligibility, and Source-owned Access Path validation to the shared implementation.
- Preserve frontend inherited-title/enum/URI hints without making frontend validation authoritative or exposing direct Source title authoring.
- Delete compiler-local and Detection-local schema interpretation, regex fallback, silent fallbacks, compatibility helpers, and superseded tests after callers move.

## Adjacent non-goals

- Provenance/origin maps and fingerprints: T4a/#171 and T4b/#175.
- Strategy Policies/cumulative budgets and schema-v3 phase naming: T5/#172 and T7/#174.
- Detection acquisition, scoring, captures, evidence policy, browser/HTTP behavior, or Source Config authoring UI redesign.
- Full JSON Schema, nested properties/array items, additional keywords, deletion/disabling/reordering, provider-specific validators, or a public validator/port.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Recognized type | Matching saved/proposed values pass identically | Shared caller-facing compiler/Detection tests |
| Pattern | Valid Rust regex enforces `is_match`; invalid/inapplicable pattern is a compiler definition error | Definition plus compiler/Detection tests |
| Enum | Non-empty unique compatible scalars enforce membership; empty/duplicate/non-scalar/incompatible enum rejects definition | Table-driven definition/value tests |
| URI | Absolute URI passes; relative/malformed value gets context category; invalid declaration is compiler error | Shared validator plus caller tests |
| Minimum | Inclusive equal/above passes, below fails; nonnumeric/inapplicable declaration rejects | Shared validator plus caller tests |
| Profile title | Non-empty reusable title survives merge and affects authoring only; empty/non-string rejects | Compiler round-trip plus UI contract |
| Direct/Source-owned title | Schema/Serde rejects authored title; programmatic invalid input gets compiler error; inherited title is unchanged | Schema/Serde parity plus compiler test |
| Unsupported keyword/shape | One deterministic compiler diagnostic; value is never ignored | Table-driven test and repository search |
| Property/new-property merge | Existing members merge recursively; complete new property is added; incomplete new property rejects; inputs unchanged | External compiler tests |
| Required/enum replacement | Exact fragment array, including `required: []`, replaces inherited location array | External compiler tests |
| Duplicate/undeclared required | One indexed deterministic diagnostic per specified occurrence | Definition/compiler/Detection tests |
| Cross-level requirements | Profile- and Access-Path-level requirement sets combine; properties required by either level remain required | Composition tests |
| Cross-level declaration/collision | A required name declared exactly once across levels passes; duplicate property declaration retains redefinition error | Composition tests |
| Additional-properties replacement | Effective location scalar equals fragment; composed false at either level closes contract | External compiler test |
| Saved value violation | Required/unknown/type/pattern/enum/URI/minimum failure is `source_validation`; no plan | External compiler test |
| Detection value violation | Same failure is `detection`; no proposal | Detection test |
| Invalid Detection profile | Any profile/recommended/non-recommended-path definition/composition error emits fixed-cardinality compiler diagnostics and causes zero probes/proposals | Deterministic fake-client tests |
| Invalid unselected compiler path | Complete Effective Source Profile rejects before saved-value validation, selection, or plan | External compiler ordering test |
| Cardinality/order | Profile error once; each path-local/composition error once in effective order; map insertion changes nothing | Determinism tests |
| Selected Source-added path schema | A complete added path introduces executable keys, is selected only after whole-profile/schema and concrete Source Config validation, and then uses those keys in plan compilation | External compiler ordering test |
| Source-owned path schema | Complete executable local schema uses the same definition/value behavior and cannot author `title` | External compiler test |
| Greenhouse/Workday/SuccessFactors | Existing pattern/title, pattern/URI/title, and URI/minimum/title contracts compile and are enforced generically | Existing profile regressions |
| Frontend | Inherited titles, enum choices, and URI controls remain; backend-only validation remains authoritative | Source UI contract tests and build |
| Runtime/Cancellation | Only typed plan crosses runtime; no Cancellation Partial Completion/status exists | Import/call-graph review |

Primary backend tests cross `compile_source` and public Profile Detection operations. Deterministic HTTP/browser clients additionally assert zero probes for invalid definitions; direct private tests are limited to definition edges that cannot be isolated economically through callers.

### Focused commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml profile_dsl::documents::serde_tests
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run test:source-ui
npm run build
```

Also run T3a’s landed focused Effective Profile Compiler target.

## Ticket-specific migration items

- [ ] Re-baseline exact T3a fragment, Effective Source Profile, compiler-stage, Diagnostic, and test names after #169 lands.
- [ ] Add profile/path Source Config Schema fragments and schema/Serde parity fixtures, including forbidden direct/Source-owned `title`.
- [ ] Introduce one private compiled contract and neutral violations; enforce exact keyword, applicability, required-cardinality, composition, path, and ordering contracts.
- [ ] Migrate Compiler, Source validation/Live Check, Detection, registry eligibility, and Source-owned validation; cache every effective path contract and gate Detection before I/O.
- [ ] Preserve Built-in constraints and frontend authoring hints without provider branches or frontend validation authority.
- [ ] Delete property/type/required/additional-properties parsing from `profile_dsl/compiler/source_config.rs` and type/enum/pattern/required/additional-properties validation from `source_profile/detection/proposal.rs` after migration.
- [ ] Delete duplicate regex/schema fallback logic and superseded tests; keep `compile_source` as the sole production compiler entry point.
- [ ] Classify all remaining backend schema-interpreter and title-authoring hits, including:

```bash
rg -n 'validate_source_config_for_detection|validate_property_schema|json_value_matches_schema_type|required_schema_keys|schema_forbids_additional_properties|property_type' src-tauri/src src-tauri/tests
rg -n 'sourceConfigSchema|source_config_schema|"(pattern|enum|format|minimum|title)"' src-tauri/src src-tauri/tests src-tauri/resources/profiles src/features/sources
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
