# T7 — Perform the schema-v3 authored hard cut

## Result

Every active authored Source and Source Profile contract uses schema version 3, the canonical phase fields `detection`, `discovery`, and `detail`, explicit authored `{ "type": "first_accepted" }` policy on complete Discovery and Detail Strategy Sets, and only the typed direct Source-fragment specialization model. The compiler and runtime preserve the behavior landed through T6; schema-v2 documents and old phase/override names are rejected rather than migrated.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#173 — T6 — Move internal phase modules directly to `detection`, `discovery`, and `detail`](https://github.com/timjonaswechler/job-radar2/issues/173).
- Blocking: [#175 — T4b — Fingerprint canonical schema-v3 Source behavior](https://github.com/timjonaswechler/job-radar2/issues/175) and [#176 — T8 — Route Discovery and Detail through one typed Strategy Set runtime](https://github.com/timjonaswechler/job-radar2/issues/176).
- Readiness: **Blocked** by #173; re-baseline against the exact landed T6 tree before assignment.
- Open decision: none. The authored policy representation is settled as the internally tagged object `{ "type": "first_accepted" }`.

## Consumed contracts

- #166 / PRD Decisions 1, 12–22, and 36–38: canonical schema-v3 names, typed direct Source specialization, compiler ordering, immutable-plan runtime boundary, and one pre-production hard cut without compatibility runtime.
- #166 / PRD Strategy Set and Effective Profile Compiler decisions: authored policy compiles to the mandatory typed plan policy; the directly passed Source remains authoritative and Source-owned access remains distinct.
- #173/T6 must have moved internal modules, public phase operations, compiled types, callers, tests, and Diagnostics to `detection`, `discovery`, and `detail`, while deliberately leaving only the schema-v2 authored boundary for this ticket. Its Discovery and Detail plans already carry mandatory typed `FirstAccepted` with behavior-preserving phase-local execution.
- The landed T1–T3 contracts remain intact: profile-based compilation exposes an inspectable Effective Source Profile; typed partial direct Source fragments merge before complete-profile and Source Config validation; keyed order, whole-array replacement, completeness, Source Config Schema specialization, and profile-only inherited `title` semantics do not change here.

Issue #171/T4a is not a blocker. If its provenance implementation has landed at readiness review, T7 renames affected typed phase field segments in place; otherwise T7 adds no provenance placeholder.

## Current gap

The ticket is blocked, so this baseline is provisional and must be replaced with the exact #173-landed paths and symbols during readiness review.

The current repository is still pre-T1/pre-T6. `src-tauri/src/schema/source-profile.schema.json`, `source.schema.json`, and `profile-dsl/strategy.schema.json` require schema version 2 and expose `detect`, `postingDiscovery`, and `postingDetail`. `profile-dsl/overrides.schema.json`, `profile_dsl/documents/overrides.rs`, `profile_dsl/compiler/overrides.rs`, and `source/documents.rs` implement `sourceOverrides.strategyOverrides[]`. `source_profile/documents.rs` and `source_profile/detection/**` use the `detect` field and `/detect` paths. The compiler still exposes `compile_source_execution_plan` and old phase-path Diagnostics.

The three Built-in Source Profiles in `src-tauri/resources/profiles/`, positive and negative fixtures under `src-tauri/tests/fixtures/source-profile-dsl/`, and authored JSON in compiler, registry, Detection, phase-runtime, Source Live Check, and acceptance-profile tests all use schema-v2 vocabulary. Current runtime targets remain `posting_discovery_runtime` and `posting_detail_runtime`; T6 is expected to replace these with final phase names before T7 starts.

Active domain and authoring guidance, including `CONTEXT.md`, the earlier DSL PRD/ADRs, production-agent guidance, and smoke documentation, still describes Source Overrides and old phase fields. The single T7 gap after T6 must be the remaining authored schema/Serde/resource/fixture/Diagnostic/documentation boundary, not another internal runtime rename.

## Target delta

### Canonical authored model

Source and Source Profile roots require `schemaVersion: 3`. Complete documents and Source-owned Access Paths use only `detection`, `discovery`, and `detail`; admitted direct Source fragments use the same canonical nested phase members. Posting-prefixed `$defs`, `$ref` targets, serialized fields, and fixture/helper names are replaced in place.

`schemaVersion: 2`, `detect`, `postingDiscovery`, `postingDetail`, `sourceOverrides`, `strategyOverrides`, and operation/path/value override representations are invalid at active JSON Schema and Serde boundaries. Strict ordinary rejection is sufficient: there is no version dispatcher, alias, warning, migration command, on-load transformer, dual fixture, or compatibility result. Unrelated Check Report, browser manifest, database, and other document schema versions do not change.

### Authored Strategy Policy

Discovery and Detail use one closed, internally tagged authored representation:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StrategyPolicyDocument {
    FirstAccepted,
}
```

Exact type placement follows T6. The contract is:

1. Every complete reusable-profile or Source-owned Discovery/Detail Strategy Set requires `policy: { "type": "first_accepted" }`.
2. A complete Source-added Access Path or newly supplied complete phase step also requires policy.
3. A partial direct Source fragment targeting an existing Strategy Set may omit policy and inherit the base value; it may explicitly supply the approved typed value.
4. A fragment introducing a complete new Access Path or phase without policy fails the existing compiler completeness gate before effective-profile validation. Deterministically sorted `missingFields` includes `policy`; no unrelated phase/path supplies it.
5. Scalar strings, externally tagged objects, raw objects, unknown members/types, and `all_required`, `collect_all`, or `at_least` are rejected.
6. Authored policy compiles directly to T6’s mandatory compiled `FirstAccepted`. There is no optional compiled field, second behavior enum, Serde default, or runtime inference from Strategy order.
7. Detection is renamed at its authored root but is not converted to the later shared Strategy Set runtime.

### Preserved compiler and runtime behavior

Mechanically migrating a T6 fixture—version, phase fields, direct-fragment representation, and required policy only—must preserve:

- authoritative direct Source selection; Effective Source Profile values, provenance if present, and deterministic keyed/array order;
- complete-profile then Source Config validation, selected Access Path resolution, Source-owned distinction, and Diagnostic cardinality/order;
- immutable plan shape and serialized compiled `first_accepted` policy;
- accepted-first, rejected/failed recovery, exhaustion, and transport-success-does-not-imply-acceptance behavior;
- Detection proposals, Discovery candidates, lazy Detail output, Source Live Check/Search Run output, persistence, and production call order;
- existing per-Strategy fetch, pagination, browser, response-size, timeout, retry, and caller-owned Discovery bounds;
- Cancellation before/during execution: earlier Diagnostics remain, later Strategies and exhaustion are suppressed, one cancellation Diagnostic is emitted, and no persistable `ResolutionCompletion::Partial` or new status is created.

T7 adds no cumulative Strategy Set budget or changed accounting.

### Direct Source specialization and validation order

The exact landed typed direct Source-fragment shape is authoritative. It remains the only specialization model: no wrapper, arbitrary map, JSON Pointer, target/value pair, or generic patch operation returns. Existing keyed entries merge recursively, complete new entries append, and non-keyed arrays replace wholly. `null`, deletion, disabling, and authored placement/reordering remain invalid. Profile identity, `detection`, Search Request/persistence fields, and Source Config Schema `title` remain unrepresentable; inherited reusable-profile `title` survives. Effective-profile validation, Source Config validation, selected-path resolution, and plan compilation retain their landed order.

### Diagnostics and provenance

Schema/compiler/runtime Diagnostic paths and phase-specific codes/messages move directly to canonical vocabulary, for example:

- `/profiles/0/detect/...` → `/profiles/0/detection/...`;
- `/accessPaths/0/postingDiscovery/...` → `/accessPaths/0/discovery/...`;
- `/selectedAccessPath/postingDetail` → `/selectedAccessPath/detail`;
- `posting_discovery_request_budget_reached` → `discovery_request_budget_reached`;
- the phase-plan absence code becomes `detail_missing` rather than `posting_detail_missing`.

Inventory landed values before editing. Rename only DSL-phase terminology, not unrelated product-level Job Posting operations. Category, severity, trigger, Strategy key, details, sampling, cardinality, and deterministic order do not change; no duplicate emission or old-code translation remains. Direct-fragment Diagnostics point to their actual schema-v3 Source location.

If #171 landed, affected `ProvenancePathSegment::Field` values use canonical phase names and old values are deleted from serializers/fixtures. Detection remains outside T4a’s provenance surface; no symmetric entry is invented. There is no versioned provenance type or second path language.

### Documentation

After the cut, active guidance must not instruct contributors to author schema v2, Source Overrides, `detect`, `postingDiscovery`, or `postingDetail`.

- Update `CONTEXT.md` to define Direct Source Specialization, Detection, Discovery, Detail, Strategy Set, and Strategy Policy, and correct affected Source/Profile/Compiler/Diagnostic/Live Check terminology without adding implementation detail.
- Update or explicitly supersede stale normative passages in `docs/prd/declarative-source-profile-dsl.md` and ADRs 0001/0009.
- Correct stale present-tense guidance in the canonical #166 PRD while preserving clearly historical comparisons.
- Update active examples/instructions in `README.md`, `docs/source-profile-production-agent.md`, `docs/dev-search-run-smoke.md`, and other discovered current guidance.
- Leave published ticket snapshots and the historical `docs/profil source algebra refactor.md` unchanged; residual old terms are allowed only when unmistakably historical.

## Dependency and deletion decision

Authored documents, policy parsing, merge/validation, plan compilation, Diagnostics, and provenance are in-process typed data/computation and use their real implementation in tests. Registry/resource loading remains an application/filesystem concern. Existing HTTP/browser seams and temporary SQLite stand-ins are reused only for parity tests; T7 introduces no trait or port.

**Deletion test:** Without one strict schema-v3 authored/compiler boundary, registry loading, Source validation, Source Live Check, Search Run and lazy Detail preparation, app-command examples, and deterministic profile fixtures would each need to understand document shape and policy/default semantics. A v2 adapter or naming wrapper hides no current complexity and can be removed without spreading behavior, so none is retained.

The Effective Profile Compiler and Strategy Set Runtime remain Deepening Candidates; this naming/migration slice does not establish accepted module depth.

## Examples

### Reusable profile

```json
{
  "schemaVersion": 3,
  "detection": { "inputUrlPatterns": [{ "pattern": "example" }] },
  "accessPaths": [{
    "key": "api",
    "name": "API",
    "discovery": {
      "policy": { "type": "first_accepted" },
      "strategies": [{ "key": "primary" }]
    },
    "detail": {
      "policy": { "type": "first_accepted" },
      "strategies": [{ "key": "detail" }]
    }
  }]
}
```

The equivalent v2 document is rejected, but the migrated document produces the same T6 plan behavior.

### Policy completeness

An existing-set fragment may omit policy and inherit `first_accepted`. A new complete phase that omits policy is rejected with `missingFields: ["policy", ...]` in deterministic sorted order. `"policy": "first_accepted"` and `{ "type": "first_accepted", "extra": true }` are schema-/Serde-invalid.

### Recovery and Cancellation parity

```text
primary: transport succeeds, acceptance rejects
fallback: accepts
result: fallback output; ordered primary then fallback Diagnostics; no exhaustion

first: rejects
second: cancelled
third: not called
result: empty; prior Diagnostics plus one cancellation Diagnostic; no exhaustion or Partial Completion
```

## Scope

- Re-baseline against #173, then hard-cut Source/Profile/direct-fragment schemas, Serde documents, schema registry, resources, fixtures, builders, assertions, and schema-facing Diagnostics to v3 names.
- Add the strict authored policy and its complete-set/partial-fragment completeness behavior, compiling to existing mandatory `FirstAccepted` without defaults.
- Delete the final v2 boundary, old Source Override schema/types/compiler paths, compatibility artifacts, duplicate fixtures/snapshots, and old schema-facing/provenance values.
- Migrate Built-ins and deterministic compiler/runtime/production-caller fixtures while preserving Greenhouse, Workday, and SuccessFactors behavior.
- Update active glossary, PRD/ADR, authoring, and operational documentation; classify historical residuals.

## Adjacent non-goals

- T8’s shared Strategy Set kernel, attempt history, reducers, cumulative budgets, or additional policies.
- Detection Strategy Set convergence or new Detection behavior; Primitive extraction/consolidation.
- T4b fingerprints/freshness; provenance implementation when T4a is absent.
- Candidate Resolution, batching, requested fields, matching, persistence changes, or new statuses.
- Source Config Schema expansion; placement/reordering/deletion/disabling/`null`; parallelism or resumability.
- Rewriting historical issue/handoff records or adding provider-specific behavior.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Complete Source Profile v3 | Schema/Serde/registry accept and round-trip only canonical names | Schema/Serde + registry integration |
| Profile-selecting Source v3 | Direct fragments compile to expected Effective Source Profile and plan | Compiler integration |
| Source-owned v3 | Canonical phases and policies compile without fabricating an Effective Source Profile | Compiler integration |
| Version 2 or old phase field | Strict rejection; no migration, alias, or compatibility result | Focused negative fixtures |
| Complete reusable/Source-owned set omits policy | Schema/Serde rejection; no inference | Negative fixture |
| Source-added complete set omits policy | Completeness rejection before effective validation; sorted `missingFields` includes `policy` | Merge/compiler integration |
| Existing-set fragment omits policy | Base policy is inherited; behavior unchanged | Merge/compiler integration |
| Explicit approved fragment policy | Typed merge succeeds and compiled policy is `FirstAccepted` | Compiler integration |
| Invalid policy shape/value/member | Strict rejection without fallback/default | Table-driven schema/Serde test |
| Old Source Override/patch surface | Strict rejection and no active schema/document/compiler implementation | Negative fixture + scoped search |
| Direct specialization regression | Keyed merge/add/order, whole-array replacement, schema/title rules, and validation order remain unchanged | Migrated T2/T3 tests |
| Source authority/profile access | Direct Source wins; profile and Source-owned results remain distinct | Compiler regression |
| First accepted/recovery/exhaustion | T6 calls, output, ordered Diagnostics, and one terminal exhaustion result remain unchanged | Discovery/Detail runtime tests |
| Existing bounds | Same stop behavior and canonical Diagnostic vocabulary; no cumulative budget | Boundedness/runtime regressions |
| Pre-/mid-attempt Cancellation | Later work/exhaustion suppressed; prior Diagnostics retained; no Partial Completion | Phase runtime + Search Run regression |
| Detection | Same proposal, captures, evidence, recommended path, config, and semantics under `detection` | `source_profile_detection` |
| Production callers | Source Live Check, Search Run, activation, lazy Detail, commands, and persistence preserve behavior/call order | Existing caller regressions |
| Built-in profiles | Greenhouse, Workday, and SuccessFactors load/compile/execute unchanged | Three deterministic profile targets |
| Diagnostic migration | Canonical path/code/message with unchanged category/severity/details/key/cardinality/order | Focused assertions/snapshots |
| Provenance present/absent | Landed segments rename in place, or no provenance code is added | Conditional provenance test + search |
| Active documentation | Current guidance is v3-only; every old term is corrected or explicit history | Reviewed docs inventory |
| Runtime boundary | Only immutable typed plans cross into phase runtime | Compiler-plus-runtime test + import search |

### Focused commands

Confirm exact T6-landed target names at readiness review:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_v3_authored_hard_cut
cargo test --manifest-path src-tauri/Cargo.toml --test phase_module_naming
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_policy_first_accepted
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_security_boundedness
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

If T4a landed, also run `cargo test --manifest-path src-tauri/Cargo.toml --test effective_profile_provenance`. Add exact focused T1–T6 targets discovered during re-baseline rather than retaining obsolete posting-prefixed targets.

## Ticket-specific migration items

- [ ] Inventory and move all active Source/Profile schema-version literals/constants/comparisons, canonical phase fields, `$defs`/refs, Serde members, resources, fixtures, builders, and Diagnostic locations to v3; classify unrelated independently versioned documents.
- [ ] Add one closed authored `StrategyPolicyDocument`; require it on complete sets, permit omission only for inherited existing-set fragments, and remove policy aliases/defaults/unwrap fallbacks/versioned compatibility types.
- [ ] Record the exact landed direct-fragment/old-override document/compiler/schema surface; delete `sourceOverrides`, `strategyOverrides`, `SourceOverrides`, `StrategyOverride`, `OverridableStep`, and any patch operation/path/value representation only within that reviewed responsibility surface.
- [ ] Rename landed phase-facing Diagnostic and, conditionally, provenance values without renaming unrelated Job Posting operations.
- [ ] Migrate Built-ins, all production- and test-authored JSON—including app, Search Run, posting, and smoke-support JSON—positive/negative fixtures, compiler/runtime callers, and helper/target filenames without keeping v2/v3 pairs.
- [ ] Update `CONTEXT.md`, normative PRD/ADRs, README, production-agent/smoke, and discovered active guidance; leave historical records unchanged and classify residuals.
- [ ] Run and classify the ticket-specific hard-cut proof below. Positive authored surfaces must have no old-version/name/override hit; negative inputs may retain old literals only to prove strict rejection; independently versioned documents and product-level Job Posting operations are classified separately.

```bash
# Active authored surfaces: expected no old contract.
rg -n '"schemaVersion"\s*:\s*2|"const"\s*:\s*2|"(detect|postingDiscovery|postingDetail)"|sourceOverrides|strategyOverrides' \
  src-tauri/src/schema src-tauri/resources/profiles \
  src-tauri/tests/fixtures/source-profile-dsl/valid

# Source/Profile Rust version forms; classify broad hits by document family.
SOURCE_PROFILE_V2_RUST_RE='\bschema_version\b\s*(?::|==|!=|<=|>=|=)\s*2\b|\bschema_version\b[^\n,]*,\s*2\b|\b2\s*,[^\n,]*\bschema_version\b|"?expected(?:SchemaVersion|_schema_version)"?\s*:\s*2\b|\b[A-Z0-9_]*(?:SOURCE_PROFILE|SOURCE|PROFILE)[A-Z0-9_]*SCHEMA_VERSION[A-Z0-9_]*\b[^=\n]*=\s*2\b'
rg -n "$SOURCE_PROFILE_V2_RUST_RE" \
  src-tauri/src/source src-tauri/src/source_profile src-tauri/src/profile_dsl \
  src-tauri/src/checks/source_live src-tauri/src/app/commands.rs src-tauri/src/search \
  src-tauri/tests --glob '*.rs'
rg -n "$SOURCE_PROFILE_V2_RUST_RE|\"schemaVersion\"\s*:\s*2" \
  src-tauri/src src-tauri/tests --glob '*.rs'

# Negative inputs and old active phase/Diagnostic values: review every hit.
rg -n '"schemaVersion"\s*:\s*2|"(detect|postingDiscovery|postingDetail|sourceOverrides|strategyOverrides)"' \
  src-tauri/tests/fixtures/source-profile-dsl/invalid src-tauri/tests \
  src-tauri/src/profile_dsl/documents --glob '*.{rs,json}'
rg -n '\bdetect\b|postingDiscovery|postingDetail|posting_discovery|posting_detail|/detect(/|"|$)' \
  src-tauri/src src-tauri/tests --glob '*.rs'

# Old active override vocabulary: expected no production hit.
rg -n 'sourceOverrides|strategyOverrides|SourceOverrides|StrategyOverride|OverridableStep' \
  src-tauri/src/schema src-tauri/resources/profiles src-tauri/src/profile_dsl \
  src-tauri/src/source src-tauri/src/source_profile --glob '*.{rs,json}'

# Build a reviewed fragment/old-override responsibility surface before searching generic patch terms.
rg -l 'DirectSource|SourceFragment|ProfileFragment|source_fragment|profile_fragment|sourceOverrides|strategyOverrides|SourceOverrides|StrategyOverride|OverridableStep' \
  src-tauri/src/profile_dsl/documents src-tauri/src/profile_dsl/compiler \
  src-tauri/src/source src-tauri/src/schema/source.schema.json \
  src-tauri/src/schema/profile-dsl --glob '*.{rs,json}' | sort \
  > /tmp/t7-override-surface-candidates.txt
: > /tmp/t7-override-surface-reviewed.txt # manually populate from classified candidates
test -s /tmp/t7-override-surface-reviewed.txt
xargs -r rg -n '"(operation|path|target|value)"\s*:|serde\s*\([^)]*rename\s*=\s*"(operation|path|target|value)"|\bpub\s+(operation|path|target|value)\s*:|\b(Operation|PatchOperation|OverridePath|OverrideValue)\b' \
  < /tmp/t7-override-surface-reviewed.txt
find src-tauri/tests/fixtures/source-profile-dsl -iname '*override*' -print

# Old schema/fixture/internal phase families: expected no active implementation hit.
rg -n 'posting(Discovery|Detail)(Step|Strategy|Extraction)|posting[-_](discovery|detail)' \
  src-tauri/src/schema src-tauri/resources/profiles \
  src-tauri/tests/fixtures/source-profile-dsl/valid
find src-tauri/src/schema src-tauri/tests/fixtures/source-profile-dsl \
  \( -iname '*posting*discovery*' -o -iname '*posting*detail*' \) -print
rg -n '\bPosting(Discovery|Detail)[A-Za-z0-9_]*\b|\bexecute_posting_(discovery|detail)[A-Za-z0-9_]*\b|(^|::)posting_(discovery|detail)(::|;|\b)' \
  src-tauri/src src-tauri/tests --glob '*.rs'
find src-tauri/src src-tauri/tests \
  \( -name '*posting_discovery*.rs' -o -name '*posting_detail*.rs' -o \
     -type d \( -name '*posting_discovery*' -o -name '*posting_detail*' \) \) -print

# Policy must be explicit and visible; compatibility helpers are forbidden.
rg -n 'policy|first_accepted|StrategyPolicyDocument|FirstAccepted' \
  src-tauri/src/schema src-tauri/src src-tauri/resources/profiles src-tauri/tests
rg -n '\bdefault_[A-Za-z0-9_]*policy[A-Za-z0-9_]*\b|\bpolicy\.unwrap_or[A-Za-z0-9_]*\b|\.policy\.unwrap_or[A-Za-z0-9_]*\b|\b(?:Legacy|Compat(?:ibility)?)[A-Za-z0-9_]*Policy[A-Za-z0-9_]*\b|\b[A-Za-z0-9_]*Policy[A-Za-z0-9_]*(?:Legacy|Compat(?:ibility)?)\b|\bPolicyV[23]\b' \
  src-tauri/src src-tauri/tests --glob '*.rs'
rg -n -U '(?s)#\[serde\([^\]]*\b(alias|default)\b[^\]]*\)\]\s*(?:(?:pub\s+)?(?:enum|struct)\s+[A-Za-z0-9_]*Policy[A-Za-z0-9_]*\b|(?:pub\s+)?policy\s*:|FirstAccepted\b)' \
  src-tauri/src src-tauri/tests --glob '*.rs'

# Provenance/compatibility and active documentation: review and classify every residual.
rg -n 'postingDiscovery|postingDetail|Provenance.*V[23]|V[23].*Provenance|legacy.*provenance|compat.*provenance' \
  src-tauri/src src-tauri/tests --glob '*.rs'
rg -n '(schema.?v?2|v2.*(source|profile|detection|discovery|detail)|legacy|compat(ibility)?|migrat(e|ion)|serde.*alias|forward)' \
  src-tauri/src/profile_dsl src-tauri/src/source src-tauri/src/source_profile \
  src-tauri/src/schema src-tauri/resources/profiles src-tauri/tests --glob '*.{rs,json}'
rg -n -i '"schemaVersion"\s*:\s*2|\bschema(?:\s+version\s+2|\s+v2|-v2)\b|"detect"|`detect`|postingDiscovery|postingDetail|\bSource Overrides?\b|sourceOverrides|strategyOverrides' \
  CONTEXT.md README.md AGENTS.md docs --glob '*.md'

# Canonical surface must be directly visible.
rg -n 'schemaVersion.*3|"detection"|"discovery"|"detail"|first_accepted|compile_source|execute_(discovery|detail)' \
  src-tauri/src/schema src-tauri/resources/profiles src-tauri/src src-tauri/tests
```

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
