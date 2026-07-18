# T14b — Add conflict-safe Detection evidence reduction and Source Proposal provenance

## Result

Profile Detection reduces accepted URL, HTTP, proposal-preparation, and transitional browser contributions in deterministic order without last-write-wins behavior. Equal contributions retain all ordered origins; unequal capture or Access Path values and unequal or overlapping atomic Source Config responsibilities fail that profile before dependent work. Every successful `SourceProposal` exposes complete, bounded provenance for retained captures, atomic Source Config values, the recommended Access Path, and evidence.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#205 / T14a](https://github.com/timjonaswechler/job-radar2/issues/205).
- Blocking: [#207 / T14c](https://github.com/timjonaswechler/job-radar2/issues/207).
- Readiness: **Blocked**. #205 is open, #206 has no readiness label, and the landed Detection contracts must be re-inspected after #205 completes.
- Open decision: none. The authored contribution boundary and serialized provenance boundary below are approved and binding.

T13b/#203 or T13c/#204 becomes a direct blocker only if the #205-landed Detection profiles actually require `at_least` or `collect_all`; do not add either dependency or policy speculatively.

## Consumed contracts

- #166 / PRD Decisions 8, 24–28: typed Detection outputs, deterministic reduction, conflict rejection, cumulative budgets, and immutable ceilings.
- #166 / PRD Strategy Set Runtime decision: Detection uses its typed phase adapter over the private shared kernel; policy attempts, budgets, Cancellation, and phase reduction stay hidden.
- #195/T12b supplies the conflict-safe reducer principles and vocabulary, but Detection uses distinct contribution and provenance types rather than posting-occurrence or Detail-patch types.
- #205/T14a must supply one compiled Detection operation, ordered accepted URL/HTTP contributions, `all_required` execution, cumulative budget and typed Cancellation behavior, the transitional browser handoff, and `source_profile/detection/proposal.rs` as the sole proposal constructor.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, testing, migration, deletion, and PR-evidence rules.

## Current gap

The current repository is still pre-#205, so exact target names and paths are provisional and must be re-baselined at readiness review.

- `src-tauri/src/source_profile/detection/mod.rs::evaluate_profile` imperatively sequences URL matching, HTTP checks, optional browser probes, and `proposal::build_source_proposal`.
- `match_input_url_patterns`, `detection/http.rs::evaluate_http_checks`, and `detection/browser.rs::evaluate_browser_probes` mutate shared capture/evidence collections. Later `BTreeMap::insert` calls replace an earlier capture with the same key.
- `detection/proposal.rs::build_source_proposal` both derives Source Config/recommended Access Path values and constructs the final proposal. Templates, schema-driven capture copying, `startUrl` defaulting, and sole-Access-Path inference do not cross a common conflict/provenance reducer.
- Rust `SourceProposal` and TypeScript `SourceProposal` in `src/lib/api/sources.ts` expose captures, Source Config, recommendation, and evidence without a typed relation to their origins.
- `src-tauri/tests/source_profile_detection.rs` covers URL/HTTP/browser ordering, captures, proposals, aggregation, diagnostics, and support behavior; the built-in profile targets cover Greenhouse, Workday, and SuccessFactors. They do not prove equal corroboration, conflict-safe responsibility ownership, or complete serialized proposal provenance.
- `src-tauri/src/app/commands.rs::detect_source_proposal_from_url` is the production boundary; Source creation code consumes the TypeScript proposal shape.

#205 is expected to replace the URL/HTTP path with one compiled Strategy Set operation while retaining a transitional contribution accumulator and current proposal DTO. This ticket replaces only that remaining reduction/provenance gap; it does not add another runtime, transport seam, policy, or constructor.

## Target delta

### One private Detection reducer

The Detection phase adapter owns one concrete, in-process reducer. Exact landed names may adapt, but responsibilities may not:

```rust
struct DetectionContribution {
    captures: BTreeMap<String, OriginValue<String>>,
    source_config: Vec<AtomicSourceConfigContribution>,
    recommended_access_path_key: Option<OriginValue<String>>,
    evidence: Vec<OriginEvidence>,
}

struct AtomicSourceConfigContribution {
    path: JsonPointer,
    value: JsonValue,
    origins: Vec<DetectionOrigin>,
}

struct DetectionOrigin {
    strategy_key: Option<String>,
    schema_path: Option<JsonPointer>,
}
```

The reducer produces retained captures, atomic Source Config pointer/value pairs, one recommendation, ordered evidence, and proposal provenance. It is private: no reducer trait, callback, plugin point, public per-contribution API, or parallel Primitive/value model is introduced.

Contributions are observed in profile Strategy order, then deterministic compiled output order within a Strategy. After accepted Strategy contributions, proposal-preparation contributions use this fixed order:

1. authored Source Config templates;
2. schema-driven capture copies in effective schema/property order;
3. input-derived `startUrl`;
4. explicit recommended Access Path;
5. sole-Access-Path inference, when applicable.

After each accepted contribution, later Strategies can read only retained, unambiguous captures. A conflict terminates that profile before dependent HTTP/browser work or proposal construction.

### Atomic Source Config ownership

- One contribution owns one canonical, non-root JSON Pointer and the complete typed JSON value at that pointer. Objects and arrays remain atomic at that pointer; the reducer does not create child responsibilities.
- A successful expression result is present even when it is JSON `null`, `""`, `{}`, or `[]`. Only the blocker-landed typed absence outcome emits no contribution.
- At the same pointer, an equal typed value is retained once and adds previously unseen origins in encounter order. An unequal value is a conflict.
- Any ancestor/descendant overlap is a conflict regardless of value or arrival order: `/api` conflicts with `/api/base`, including when `/api` contains `null`, `{}`, or an object with `base`.
- Equality and ancestry use decoded canonical JSON Pointer segments, never string-prefix comparison. Invalid, root, or non-canonical pointers are rejected before reduction.
- A conflict emits safe, deterministic `detection` Diagnostics naming responsibility paths/origins, exposes no conflicting raw value, and returns no partial proposal for that profile. Diagnostic order is profile, Strategy, then contribution order, followed by at most one terminal conflict summary when the landed convention requires it; do not duplicate one conflict as per-origin and terminal errors. Reuse #205's Diagnostic vocabulary or one approved canonical addition rather than inventing codes before re-baseline. Existing multi-profile aggregation continues: another profile may still produce `Matched` or `Ambiguous`; if none succeeds and any profile has a reduction conflict, the result is `Failed`.

### Captures, recommendation, evidence, and origins

- Captures and the recommended Access Path retain the first produced value. An equal later value adds origins; an unequal later value conflicts. Absence is not a contribution. A produced Access Path key must satisfy the compiled non-empty-string contract.
- Evidence identity is `(kind, canonical authored descriptor path)`. Re-emission retains one evidence item and adds unique origins. Different descriptor paths remain distinct even when messages match; message text and map iteration order are never identity inputs. If #205 does not provide a canonical descriptor path, this ticket compiles one from the canonical authored schema location rather than falling back to message identity.
- Profile-authored evidence is seeded once with a profile-metadata origin, not attributed to a Strategy that did not produce it.
- Every origin has at least one locator. Strategy output has `strategy_key = Some(stable_key)` and may include `schema_path`; profile metadata or proposal derivation has `strategy_key = None` and a canonical `schema_path`.
- Origin identity is the exact `(strategy_key, schema_path)` pair. Equal pairs occur once; distinct pairs retain first-contribution order. Values, messages, attempt indices, and map order do not affect identity.
- Runtime attempt history, mutable reduction state, unaccepted contributions, and raw transport/provider values remain private. Provenance and conflict Diagnostics retain no body, header, cookie, query value, credential, unrestricted Source Config value, or secret.

Reduction conflict is phase failure, not Strategy acceptance, unsupported-profile matching, budget exhaustion, or Cancellation. #205's typed Cancellation and cumulative-budget terminal precedence remains unchanged: Cancellation discards accumulated proposals and never becomes Resolution Partial Completion; budget exhaustion remains the established failed-profile outcome.

### Required authored contribution boundary

A Detection Strategy may author this optional schema-v3 shape using the exact #192/#205-landed typed value-expression language:

```json
{
  "contributions": {
    "sourceConfig": [
      { "path": "/apiBase", "value": { "...": "landed typed expression" } }
    ],
    "recommendedAccessPathKey": {
      "value": { "...": "landed typed expression" }
    }
  }
}
```

`sourceConfig` is a finite ordered list of atomic path/value responsibilities. `recommendedAccessPathKey` is one typed non-empty-string expression. Authored `null` in place of an expression is invalid, while a valid expression evaluating to JSON `null` is a present Source Config value.

Schema, Serde, and compiler validation reject unknown members, duplicate exact paths within one Strategy, statically visible ancestor/descendant overlap within one Strategy, root/non-canonical pointers, missing expressions, unavailable Detection context, unbounded output, and non-string recommendation results. Direct Source fragments cannot author Detection contributions because Detection remains reusable-profile-owned and runs before a concrete Source exists. Cross-Strategy and proposal-preparation conflicts remain runtime concerns because paths/values may be context-dependent. Compiled plans carry typed expressions and canonical pointers; runtime does not parse the authored shape.

### Proposal preparation and projection

Every existing derivation becomes a typed contribution before final projection:

- each authored top-level Source Config template contributes its complete rendered value at `/<escaped-property>`, with its canonical template path and origins of every retained input/capture it read;
- each eligible schema/capture default contributes the capture string at the canonical property pointer; profile-schema and Access-Path-schema derivations retain separate origins and merge only when equal;
- the selected submitted absolute URL contributes `/startUrl` when the effective schema/defaulting rules require it, with an explicit input/profile-schema derivation origin;
- authored `recommendedAccessPathKey` contributes from its canonical metadata path;
- sole-Access-Path inference contributes from the canonical Access Path definition path.

`source_profile/detection/proposal.rs` remains the sole `SourceProposal` constructor. It receives the already reconciled result, runs existing Source Config validation and Search Request-field exclusion, renders key/name candidates, and projects the DTO. Validation occurs only after reduction. The constructor no longer combines values, writes defaults or recommendations directly, resolves conflicts, or reconstructs provenance. Key/name candidates remain proposal behavior and do not become Detection acceptance or Source Config responsibilities.

### Required caller-visible provenance

Every successful proposal adds the required field:

```rust
pub struct SourceProposalProvenance {
    pub captures: BTreeMap<String, Vec<DetectionOriginDto>>,
    pub source_config: BTreeMap<JsonPointer, Vec<DetectionOriginDto>>,
    pub recommended_access_path: Vec<DetectionOriginDto>,
    pub evidence: Vec<Vec<DetectionOriginDto>>,
}

pub struct DetectionOriginDto {
    pub strategy_key: Option<String>,
    pub schema_path: Option<JsonPointer>,
}
```

The Rust/Serde and TypeScript DTOs expose `provenance` with camelCase `captures`, `sourceConfig`, `recommendedAccessPath`, and `evidence`; each origin exposes optional camelCase `strategyKey` and `schemaPath`, and both may never be absent together. The field and all four subcollections always serialize; empty maps are `{}` and empty lists are `[]`. No `skip_serializing_if`, optional compatibility form, alias, or old/new serializer is retained.

Every retained capture key, atomic Source Config pointer, successful recommendation, and evidence item has a non-empty, ordered, duplicate-free origin list. `sourceConfig` contains only exact owned atomic pointers, not fabricated descendants; `evidence[i]` aligns with final evidence item `i`. `DetectionOriginDto` preserves the internal locator and exact-pair identity invariants. `UnsupportedSourceProfile` remains unchanged because it is not a Source Proposal.

Except for required `SourceProposal.provenance`, existing proposal fields and the outer Detection operation/`SourceProposalDetectionResult` status algebra remain as landed by #205. The UI need not display provenance; its proposal types/fixtures must accept the required serialized field.

## Dependency and deletion decision

Reduction, pointer comparison, origin accumulation, ordering, proposal preparation, validation, and DTO projection are in-process. Reuse #205/#178 HTTP production and deterministic clients and the existing browser production/deterministic seam; T14b changes neither transport boundary. Registry/effective schema documents are immutable input data, not ports. SQLite is not involved.

Tests cross the public typed Detection operation with real compiled profiles, the real Strategy Set adapter/reducer/proposal constructor/validator, and deterministic HTTP/browser implementations. A private test is justified only for a decoded JSON Pointer ancestry edge that cannot be represented economically through a valid profile.

**Deletion test:** Without this reducer boundary, conflict comparison, origin accumulation, deterministic ordering, safe context advancement, and Source Config/proposal provenance reconstruction would spread across URL, HTTP, browser, and proposal-construction paths. A forwarding or DTO-conversion-only module fails this test.

## Examples

1. **Equal capture:** URL and HTTP Strategies both produce `tenant = "acme"`. The proposal retains one value with both ordered origins, and a later Strategy may safely read it.
2. **Capture conflict:** URL produces `tenant = "acme"`; HTTP produces `tenant = "other"`. Neither wins, no dependent work or proposal runs for that profile, and other profiles continue unless Cancellation occurred.
3. **Atomic value:** a template and Strategy both produce `{ "base": "https://jobs.example" }` at `/api`. One atomic value retains all origins; no `/api/base` provenance is created. A different `/api` value or any `/api/base` responsibility fails the profile.
4. **Presence:** successful values `null`, `""`, `{}`, and `[]` are retained with provenance; typed absence emits no responsibility or origin.
5. **Recommendation:** metadata and Strategy both recommend `api`, so origins merge. `api` versus `feed` fails the profile.
6. **Browser transition:** the #205 transitional browser output is translated once into the reducer input. T14c later replaces its execution without changing reduction/proposal semantics.
7. **Budget/Cancellation:** a budget denial remains #205's budget terminal, not a conflict. Cancellation returns typed Cancellation and no proposal or Resolution Partial Completion.

## Scope

- Re-baseline against the landed #205 Detection operation, contribution accumulator, context advancement, budget/Cancellation behavior, browser handoff, proposal constructor, Diagnostics, DTOs, callers, and tests.
- Add strict authored/typed/compiled support for `contributions.sourceConfig` and `contributions.recommendedAccessPathKey`, including direct-Source-fragment rejection.
- Replace the transitional replacement accumulator with the one private conflict-safe reducer and Detection-specific origin model.
- Route URL, HTTP, proposal-preparation, and transitional browser output through that reducer; preserve profile/policy order and advance context only from unambiguous captures.
- Route every listed Source Config and recommendation derivation through proposal-preparation contributions in the fixed order.
- Keep `proposal.rs` as sole constructor while removing direct final-value writes, merge logic, and inferred-origin reconstruction from it.
- Add required Rust/Serde/TypeScript proposal provenance and update frontend fixtures/consumers only enough to preserve compilation and proposal use.
- Preserve #205's outer status aggregation, absolute-URL boundary, `all_required` behavior, budgets, Cancellation, optional HTTP-status behavior, and browser bounds.
- Delete the transitional last-write-wins accumulator/replacement branch, duplicate merge helpers, inferred provenance reconstruction, DTO compatibility code, and superseded implementation-detail tests.
- Update active canonical Detection documentation for equal merge, conflict, provenance, and the transitional browser handoff.

## Adjacent non-goals

- Browser Strategy execution or browser ceiling changes: T14c/#207.
- Final deletion of all replaced Detection execution residue: T14d.
- New Strategy Policies or speculative `at_least`/`collect_all` use.
- Detection scoring, confidence weighting, ranking, or source-specializable Detection.
- A generic reducer framework, public contribution API, new transport seam, or second proposal constructor.
- Candidate Resolution, posting identity, Search Request matching, persistence, statuses, parallel execution, or resumability.
- Broad provenance UI redesign; provenance is a required serialized machine-readable boundary in this ticket.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Valid authored contributions | Canonical pointer/value and recommendation expressions compile to typed immutable contributions | Schema/Serde/compiler test through profile loading |
| Invalid authored shape | Root/non-canonical/duplicate/known-overlapping pointer, missing or authored-null expression, unknown field, unavailable context, unbounded output, or non-string recommendation rejects before execution | Schema/Serde/compiler parity table |
| Source specialization | Detection contribution authoring in a direct Source fragment is rejected | Source-fragment parity test |
| Distinct responsibilities | One proposal preserves values, contribution order, and non-empty origins | Public Detection integration test |
| Equal/unequal capture | Equal values merge origins and remain readable; unequal values stop the profile before later work | Scripted HTTP call-log test |
| Equal/unequal Source Config | Equal complete values at one pointer merge; unequal values fail before proposal validation | Public Detection table |
| Pointer overlap | `/api` and `/api/base` fail in either order for scalar/null/object values | Public Detection order table; narrow pointer unit test only if needed |
| Present versus absent | `null`, empty string/object/array remain values; typed absence emits nothing | Public Detection table |
| Equal/conflicting recommendation | Equal keys merge origins; unequal valid keys produce no proposal | Public Detection test |
| Evidence identity | Same kind/descriptor path deduplicates and adds origins; equal messages at different paths remain distinct | Public Detection test |
| Profile evidence | Authored evidence appears once with metadata origin, never a fake Strategy origin | Fixture assertion |
| Proposal derivations | Templates, schema copies, `startUrl`, explicit recommendation, and sole-path inference enter the reducer in fixed order with dependency origins; `/startUrl` retains its explicit input/profile-schema derivation origin | Public Detection table |
| Browser handoff | Equal/conflicting transitional browser output follows the same reducer | Deterministic browser test |
| Unsupported profile | Existing `UnsupportedSourceProfile` result is unchanged and gains no provenance DTO | Public Detection aggregation test |
| Multi-profile conflict | A conflicting profile contributes ordered Diagnostics while another can still match; no success plus a conflict is `Failed` | Public Detection aggregation test |
| Budget/Cancellation | Existing terminal precedence remains; no conflict or partial proposal is fabricated | Detection budget/Cancellation tests |
| Provenance serialization | `provenance` and all four camelCase collections always serialize, empties remain `{}`/`[]`, evidence indices align, every retained responsibility has origins, and each origin exposes `strategyKey`/`schemaPath` with at least one present | Rust serialization plus frontend contract test |
| Origin/order stability | Exact origin pairs deduplicate; distinct pairs and results remain stable across map insertion order | Table-driven public/serialization test |
| Data minimization | Secret sentinels from response/header/query do not appear in provenance, Diagnostics, or logs | Sanitization test/static review |
| Regression | Greenhouse, Workday, and SuccessFactors retain generic proposal behavior except for required provenance | Existing three deterministic fixture targets |
| Policy inventory | Landed Detection profiles require no policy beyond available prerequisites | Authored-profile inventory/static check |
| Deletion | No transitional replacement accumulator, duplicate reducer, inference reconstruction, or compatibility DTO remains | Repository searches and call-graph review |

### Focused commands

Re-inspect #205-landed target names and substitute exact equivalents without dropping coverage:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_registry
cargo test --manifest-path src-tauri/Cargo.toml --test source_profile_detection
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
npm run build

rg -n 'SourceProposal|SourceProposalEvidence|provenance|strategyKey|schemaPath' \
  src-tauri/src/source_profile/detection src-tauri/src/app/commands.rs \
  src/lib/api/sources.ts src/features/sources
rg -n 'captures\.insert|source_config.*insert|recommended.*access.*path' \
  src-tauri/src/source_profile/detection
rg -n 'legacy|compat|forward|last.?write|overwrite|replace' \
  src-tauri/src/source_profile/detection src/lib/api/sources.ts
```

Also run the #205-landed focused Strategy Set/`all_required` and Detection budget/Cancellation targets when present. Classify every search hit; initialization and equal-origin accumulation are permitted, replacement at a Detection responsibility boundary is not.

## Ticket-specific migration items

- [ ] Replace the #205-landed transitional contribution accumulator with the private conflict-safe Detection reducer; migrate all URL/HTTP/proposal-preparation/browser inputs directly.
- [ ] Add the approved strict Strategy contribution documents/schema/compiler output using landed typed expressions and canonical pointers; reject Detection contributions in direct Source fragments.
- [ ] Move authored templates, schema capture copying, `startUrl`, explicit recommendation, and sole-path inference to typed proposal-preparation contributions.
- [ ] Reduce complete atomic Source Config values with exact-pointer equality and decoded-segment overlap checks; retain present null/empty/object/array values.
- [ ] Enforce origin locator, exact-pair identity, deterministic order, and safe data retention in the internal and DTO models.
- [ ] Add required `SourceProposal.provenance` to Rust, Serde, TypeScript, and frontend fixtures with no compatibility serializer or optional old shape.
- [ ] Keep `proposal.rs` as sole constructor but delete its direct merge/default/recommendation/provenance-inference responsibilities.
- [ ] Delete the last-write-wins branch, duplicate reducers/helpers, forwarding modules, compatibility DTOs, and superseded helper tests.
- [ ] Review all remaining mutation/search hits and prove they are initialization, equal-origin accumulation, or unrelated behavior.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
