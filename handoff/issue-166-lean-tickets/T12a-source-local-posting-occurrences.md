# T12a — Define Source-local posting occurrences and Discovery value semantics

## Result

Discovery returns one typed `PostingOccurrence` model whose Source-local identity is deterministic and whose provider reference, provider values, noncanonical hints, and `postingMeta` are structurally disjoint. Discovery and Detail share this occurrence type, while the existing pre-T16 Search Run path admits only occurrences with provider title and company and cannot treat hints as canonical posting data.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#192/T11c](https://github.com/timjonaswechler/job-radar2/issues/192).
- Blocking: [#195/T12b](https://github.com/timjonaswechler/job-radar2/issues/195).
- Readiness: **Blocked** while #192 is open; the issue has no `ready-for-agent` label.
- Open decision: none.

At readiness review, re-baseline the provisional paths and exact type names below against #192's landed schema-v3 documents, immutable value plans, typed Discovery/Detail operations, diagnostics, tests, and production callers. Stop and perform Design It Twice rather than implementing if the landed design cannot preserve this contract without a second value evaluator, raw authored-JSON runtime, compatibility wrapper, or moving T12b/T15/T16 responsibilities into this ticket.

## Consumed contracts

- #166 / PRD Decisions 9–11, 24, 31–32, and 46: typed Discovery occurrences, Source-local identity, provider-value/hint separation, backend-owned normalization, and separation from cross-Source Job Posting deduplication.
- #166 / PRD Strategy Set module decision: callers use typed phase operations; runtime executes immutable plans and preserves deterministic budgets, attempts, diagnostics, and Cancellation.
- #192 provides schema-v3 authored value expressions, one canonical implementation per value Primitive, placement-aware immutable compiled value plans, typed Discovery/Detail value contexts, strict `first_non_empty`, scalar `const` parity, bounds, and the rule that raw authored expressions do not reach runtime.
- The concrete Source passed to compiled Discovery is authoritative. T12a reuses #192's evaluator and value outcomes; it adds no second evaluator, raw-value context map, or Source/profile-authored identity.
- `handoff/issue-166-delivery.md` supplies the shared readiness, hard-cut, testing, migration, Definition-of-Done, and PR-evidence contract.

## Current gap

The current repository is still pre-#192, so this baseline is provisional:

- `src-tauri/src/profile_dsl/documents/posting_discovery.rs` and `src-tauri/src/schema/profile-dsl/extract.schema.json` model required title, company, and URL plus optional locations, `postingMeta`, and description; there is no typed `reference`, `providerValues`, `hints`, `hintUse`, or provider posting ID.
- `src-tauri/src/profile_dsl/execution_plan/posting_discovery.rs` clones authored `FieldExpression` values into `ExecutionPlanPostingDiscoveryFields`; #192 must first replace this with compiled value plans.
- `src-tauri/src/profile_dsl/runtime/posting_discovery.rs` exposes `PostingDiscoveryCandidate`, while `src-tauri/src/profile_dsl/runtime/posting_detail.rs` separately exposes `PostingDetailPostingOccurrence`. Neither carries concrete Source identity, typed occurrence identity, hint authorization, or separate provider and normalized identity URLs.
- `src-tauri/src/profile_dsl/runtime/posting_discovery/extract.rs` emits only when title, company, and URL resolve, and it trims, normalizes, and deduplicates locations inside Discovery.
- `src-tauri/src/search/run/execution.rs` converts every Discovery candidate directly to the complete `SourceCandidate` in `src-tauri/src/search/run/types.rs`; backend normalization and cross-Source merging remain in `src-tauri/src/search/normalization.rs` and `src-tauri/src/search/run/service/merging.rs`.
- Current Discovery/Detail, Source Live Check, Search Run, and Greenhouse/Workday/SuccessFactors tests encode the two old DTOs and complete-candidate behavior. No test proves provider-ID precedence, conservative URL fallback, mixed-kind non-correlation, hint noncanonicality, or lossless location handoff.
- `src-tauri/Cargo.toml` already has a direct `url = "2"` dependency, but no occurrence-identity normalizer exists in `src-tauri/src/`.

The gap is one typed Discovery occurrence/value contract and one pure Source-local identity constructor—not reduction, Candidate Resolution, cross-Source deduplication, or persistence.

## Target delta

### Authored and compiled Discovery output

Replace the complete-candidate extraction shape with four disjoint typed sections:

```json
{
  "extract": {
    "reference": {
      "url": { "type": "json_path", "jsonPath": "$.absoluteUrl", "cardinality": "one" },
      "providerPostingId": { "type": "json_path", "jsonPath": "$.jobId", "cardinality": "optional" }
    },
    "providerValues": {
      "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "optional" },
      "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "optional" },
      "locations": { "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" },
      "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "optional" }
    },
    "hints": {
      "title_from_url": {
        "value": { "type": "item_field", "key": "urlSlug", "cardinality": "optional" },
        "hintUse": "search_prefilter"
      }
    },
    "postingMeta": {
      "detailApiId": { "type": "json_path", "jsonPath": "$.detailApiId", "cardinality": "optional" }
    }
  }
}
```

- `reference.url` is authored-required and must evaluate per item to one non-empty absolute HTTP(S) provider URL. `providerPostingId` is optional; a present value must be non-empty, opaque, and case-sensitive.
- `providerValues` admits only optional title, company, locations, and description text. URL exists only in `reference`. Locations preserve #192 evaluator output order and duplicates; Discovery performs no implicit splitting, geographic interpretation, case folding, sorting, or deduplication.
- `hints` is a finite stable-keyed scalar object. A technical key is opaque and remains noncanonical even when spelled `title`, `company`, or `locations`; there is no canonical-key denylist. The only admitted `hintUse` is `search_prefilter`. An omitted use leaves observable technical data that cannot affect Search Request evaluation.
- `postingMeta` remains separate hidden Source-local technical data for re-identification/detail loading. It cannot satisfy a provider field, affect identity, or participate in matching; preserve #192's union-based Detail key-admission contract.
- Schema and direct Serde reject unknown sections/fields, `null`, alternate `kind` tags, generic contribution arrays, and unknown hint uses. Source specialization uses the same schema-v3 nested vocabulary and merge rules.
- Compile every expression through #192's typed value family and phase placement. Runtime receives immutable compiled plans only. Empty optional outputs are omitted according to #192's outcome contract.

### Typed occurrence and identity

Responsibility-level interface; names may adapt to #192 without changing caller-visible meaning:

```rust
pub enum PostingOccurrenceIdentity {
    ProviderPostingId { source_key: SourceKey, provider_posting_id: ProviderPostingId },
    NormalizedUrl { source_key: SourceKey, normalized_url: NormalizedPostingUrl },
}

pub struct PostingOccurrence {
    pub identity: PostingOccurrenceIdentity,
    pub reference: PostingReference,
    pub provider_values: ProviderValues,
    pub hints: BTreeMap<HintKey, DiscoveryHint>,
    pub posting_meta: PostingMeta,
}

pub struct PostingReference {
    pub provider_url: AbsolutePostingUrl,
    pub provider_posting_id: Option<ProviderPostingId>,
}

pub struct ProviderValues {
    pub title: Option<String>,
    pub company: Option<String>,
    pub locations: Vec<String>,
    pub description_text: Option<String>,
}

pub struct DiscoveryHint { pub value: String, pub hint_use: Option<HintUse> }
pub enum HintUse { SearchPrefilter }
```

Validated newtypes are justified only when they enforce or communicate an invariant; do not wrap `String` merely to rename it.

Identity invariants:

1. The concrete Source key comes from typed Discovery input and cannot be authored or spoofed by a Profile expression.
2. A provider ID makes identity exactly `(source_key, provider_posting_id)`; URL remains available but does not alter identity.
3. Without an ID, identity is exactly `(source_key, normalized absolute provider URL)`.
4. Title, company, locations, description, hints, `postingMeta`, Strategy/profile key, host, and company never participate.
5. Different Source keys remain distinct. Same-Source equal IDs compare equal despite different URLs; same-Source equal normalized fallback URLs compare equal.
6. ID and URL-fallback variants never compare equal merely because their URLs match. If one Strategy supplies an ID and another does not, preserve both occurrences; do not guess correlation.
7. Discovery preserves item order. Location order is preserved; hint/`postingMeta` keys serialize and diagnose in stable order; map insertion order cannot affect identity equality/hash/order.
8. `PostingOccurrence` becomes the one occurrence passed from Discovery to Detail and later Candidate Resolution. Delete the second Detail occurrence DTO and conversion shape.
9. Provider values remain unnormalized backend inputs. Hints have no conversion into provider/canonical fields; even an authorized hint may later reject but never finalize or populate persisted data.
10. A valid reference emits an occurrence even with no provider values, hints, or `postingMeta`. An invalid required reference suppresses only that item and emits an item-scoped Structured Diagnostic.
11. The occurrence contains no Search Request, Match/Exclusion result, normalized geolocation, Source Status, persistence status, or cross-Source Job Posting identity.

### Provider URL and URL-fallback policy

Use one pure in-process standards URL parser for every provider URL:

- trim surrounding ASCII whitespace only; require absolute `http` or `https` plus host;
- reject username/password/userinfo for both identity variants; credentials and full secret-bearing URLs must not appear in diagnostics, logs, or derived artifacts;
- retain the validated parsed provider URL for navigation and downstream use;
- permit a fragment only when a valid provider ID owns identity; reject fragment-bearing URL fallback;
- for fallback identity, use standards-parser serialization to canonicalize scheme/host and IDNA, remove default ports, and resolve dot segments;
- preserve path semantics and meaningful trailing slash, parser-preserved percent encoding, and query name/value/multiplicity/order;
- do not sort or strip query parameters, add/remove trailing slashes, apply provider heuristics, follow redirects, correlate variants, or perform I/O.

Malformed/unsupported URLs, userinfo, fragment-only fallback, empty URLs, and empty provider IDs produce stable runtime Diagnostics at the concrete `reference` expression path with Strategy key and item index/provider position when available. Reuse landed codes or responsibility-level equivalents such as `occurrence_reference_invalid`, `occurrence_provider_id_empty`, and `occurrence_url_identity_unsupported`; details may report scheme/host presence but never credentials or the full secret-bearing URL.

### Failure, bounds, Cancellation, and pre-T16 Search Run transition

- Optional provider-value/hint failures follow #192's item-scoped placement outcome and cannot cross structural sections. Existing Strategy Set attempt order, budgets, diagnostics, and terminal behavior remain unchanged.
- Hints and provider fields remain under #192's complete effective expression node/depth limits. Use the landed finite keyed-output/Strategy-expression ceiling rather than inventing an unbounded runtime collection or independent limit.
- URL normalization is finite pure work. T12a adds no network/browser work, retries, pagination, batching, fan-out, completion/count/status type, or persistence.
- Cancellation follows the existing typed Discovery/Search Run path, prevents later work, and creates no persistable `ResolutionCompletion::Partial`.
- Migrate Source execution directly from the old candidate DTO to `Vec<PostingOccurrence>` (or the landed typed collection), with no dual output or compatibility adapter.
- At the existing private Search Run edge, admit an occurrence to `SourceCandidate` only when provider title and company are present. Use the validated provider URL; locations remain optional and description is not required by the current matcher.
- Only admitted occurrences enter backend normalization, Match/Exclusion rules, cross-Source merging, Job Posting/Match persistence, and existing `candidate_count`. An incomplete occurrence enters none of these, emits no new warning/count/state/status, and does not stop later complete occurrences.
- Search Run does not call Detail or evaluate hints in this ticket. Hints, `postingMeta`, and URL-derived text never satisfy admission. T16 solely owns incomplete-candidate visibility, Candidate states, hint prefilter execution, batching, discovered/processed counts, completion, and Source-scoped Candidate Resolution.

## Dependency and deletion decision

Occurrence/value/identity types and provider-ID/URL logic are concrete in-process domain data and pure logic. Reuse #192's compiled plans/evaluator directly. The existing typed Discovery operation remains caller-facing; existing HTTP/browser seams remain unchanged. `url` is an in-process library, not a normalizer port. Search Request normalization/rules, cross-Source Job Posting deduplication, and SQLite remain separate backend responsibilities.

**Deletion test:** Removing this typed occurrence boundary would force Discovery, Detail, T12b reducers, T13c collection, T15 requested fields, and T16 Candidate Resolution to duplicate Source-local identity precedence, URL validation, provider/hint separation, `postingMeta` handling, and the prohibition on hint finalization. A Discovery-only DTO or forwarding wrapper fails this test.

## Examples

1. **Provider ID:** Source `mainz_careers`, ID `REQ-1042`, and any valid provider URL yields `ProviderPostingId(mainz_careers, "REQ-1042")`; changing the URL does not change identity.
2. **Equivalent fallback:** `HTTPS://EXAMPLE.TEST:443/jobs/engineering/../1042?lang=en` and `https://example.test/jobs/1042?lang=en` have equal fallback identities for the same Source. Query reordering/content and `/1042` versus `/1042/` remain distinct.
3. **Hint-only occurrence:** a valid URL plus `hints.title = { value: "senior-rust-engineer", hintUse: search_prefilter }` emits an occurrence without provider title/company. It is not admitted to the pre-T16 Search Run candidate path.
4. **Fragment/userinfo:** `https://example.test/jobs#REQ-1042` fails without an ID and succeeds as an ID-based reference with ID `REQ-1042`; `https://user:secret@example.test/jobs/1042` always fails with a sanitized Diagnostic.
5. **Mixed kinds:** the same Source/URL once with ID `1042` and once without produces two different identity variants; T12a does not correlate or reduce them.

## Scope

- Add the disjoint authored/Serde/schema/Source-fragment shape and matching immutable compiled output plans using #192's value family.
- Add typed references, provider values, hints, `postingMeta`, one shared occurrence, and provider-ID-first identity with conservative URL fallback.
- Preserve provider locations losslessly after explicit authored transforms; remove phase-local implicit location cleanup.
- Emit stable sanitized item-scoped reference Diagnostics and preserve deterministic ordering, budgets, and Cancellation.
- Migrate Discovery, Detail (including UI lazy Detail loading), Source Live Check, Search Run, and deterministic provider-shaped callers/tests directly; implement only the narrow Search Run admission edge above. Update Source Live Check summaries only as needed for the new shape—do not redesign report persistence or freshness.
- Delete `PostingDiscoveryCandidate`, `PostingDetailPostingOccurrence`, old required-title/company fields, duplicate occurrence conversions, old Source-execution output variants, compatibility shapes/wrappers/aliases, and superseded tests after equivalent interface coverage exists.
- Update `CONTEXT.md` with Posting Occurrence, Provider Value, Discovery Hint, Hint Use, and Source-local identity; update the strategy-algebra and landed schema-v3 DSL docs to the implemented shape, identity/URL policy, valid hint-key rule, and pre-T16 transition without dual vocabulary.

## Adjacent non-goals

- Occurrence/Detail patch reducers and conflict diagnostics: T12b/#195.
- `collect_all` occurrence union: T13c/#204.
- Requested multi-field Detail and Candidate Resolution: T15/#219 and T16/#233.
- Hint prefilter execution, normalization/final matching, batches, counts/completion, incomplete-candidate diagnostics, and finalized-only persistence.
- Cross-Source Job Posting deduplication/persistence identity or structured Location semantics (#57).
- ID-only/URL-less occurrences, aggressive URL heuristics, redirects, tracking removal, query sorting, trailing-slash rewriting, provider-ID parsing, or mixed-kind correlation.
- Generic contributions, dynamic canonical fields, arbitrary hint uses, hint-to-canonical conversion, provider-specific schemas/branches, or new ports/traits.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| Same ID / different ID / different Source | Same Source+ID compares equal despite URL; different IDs or Sources differ | External typed Discovery/identity table |
| URL equivalent forms | Scheme/host case, default port, and dot segments canonicalize equally | External Discovery URL table |
| Preserved URL semantics | Query order/content and trailing slash remain distinct | External Discovery URL table |
| Mixed identity kinds | ID-based and fallback identities sharing URL remain different | External identity test |
| Minimal occurrence | Valid URL with no optional values emits one occurrence | External typed Discovery test |
| Missing/empty URL | Authored absence is schema/Serde rejection; runtime empty emits Diagnostic and no item | Schema parity + Discovery test |
| Relative/non-HTTP/missing-host URL | Sanitized item Diagnostic; no occurrence | External Discovery table |
| Userinfo | Rejected for all identities; no credential/full-URL leakage | Discovery Diagnostic/log test |
| Fragment fallback / fragment with ID | Fallback rejected; valid ID-based occurrence accepted | External Discovery test |
| Empty provider ID | Stable reference-path Diagnostic; no occurrence | External Discovery test |
| Provider values | Optional fields remain only under `provider_values`; no canonical/final claim | Typed serialization test |
| Provider locations | Evaluator order, duplicates, whitespace, and provider-specific strings survive Discovery | External Discovery test |
| Authorized/unmarked hint | `search_prefilter` is typed; omitted use is observable but unauthorized | Compile + Discovery serialization test |
| Unknown hint use/alternate shape/null | Schema and direct Serde reject identically | Schema/Serde parity fixtures |
| Unavailable value expression | Compiler rejects it at the deterministic output path; no runtime plan | External compiler semantic test |
| Canonical-looking hint key | `title`, `company`, or `locations` is admitted but remains noncanonical | Schema/Serde + Discovery test |
| Hint cannot finalize | URL plus hint keyed `title` leaves provider title absent and is not a Search Run candidate | Discovery/Search Run boundary test |
| `postingMeta` separation | Retained for Detail; cannot affect identity/provider fields | Discovery/Detail boundary test |
| Deterministic maps/order | Stable serialization/Diagnostic order; identity ignores map insertion order | Deterministic rerun test |
| Equal duplicate-ready identity | Equal identity is exposed; no T12b/T13c reducer runs here | Identity test + static scope review |
| Shared Detail occurrence | Detail consumes `PostingOccurrence` directly; provider URL/`postingMeta` remain available | External Detail test + deletion search |
| Search Run complete occurrence | Existing candidate count, normalization, rules, matching, and persistence continue | Search Run regression |
| Search Run incomplete occurrence | No admission/count/diagnostic/state/normalization/match/merge/persistence; later complete items continue | Search Run test with temporary SQLite |
| Cancellation/budget | Existing stop/terminal behavior remains; no occurrence after stop and no Resolution Partial | Discovery/Search Run regressions |
| Acceptance profiles/Live Check | Greenhouse, Workday, SuccessFactors and Source Live Check consume generic occurrences | Existing deterministic regressions |
| Cross-Source boundary | Occurrence identity is absent from Job Posting merge/persistence matching | Static review + dedup regression |
| Deletion | One occurrence model; no old DTO, dual output, tagged contribution, or incomplete-candidate policy remains | Reviewed repository searches |

Primary tests cross the #192-landed compiler and typed Discovery/Detail operations. Search Run assertions cross the existing service and real temporary SQLite where persistence absence matters. Private tests are limited to narrow URL/newtype parser edges not economically visible through Discovery.

### Focused commands

Reconcile target names with #192's landed tree, then run the focused equivalents:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_semantic_validation
cargo test --manifest-path src-tauri/Cargo.toml --test primitive_registry
cargo test --manifest-path src-tauri/Cargo.toml --test strategy_set_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test posting_discovery_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test posting_detail_runtime
cargo test --manifest-path src-tauri/Cargo.toml --test source_live_check
cargo test --manifest-path src-tauri/Cargo.toml --test greenhouse_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test workday_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml --test successfactors_profile_dsl
cargo test --manifest-path src-tauri/Cargo.toml search::run
cargo test --manifest-path src-tauri/Cargo.toml search::posting
```

Run and classify the ticket-specific inventory/deletion searches:

```bash
rg -n 'PostingOccurrence|PostingOccurrenceIdentity|providerPostingId|providerValues|hintUse|NormalizedPostingUrl|ExecutionPlanPostingDiscoveryFields|PostingDiscoveryFields' src-tauri/src src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'normalize.*url|Url::parse|username\(|password\(|fragment|set_fragment|query_pairs|trailing|normalize_locations|collapse_whitespace|dedup' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'SourceCandidate|candidate_count|same_job_posting|merge_postings|find_posting_by_source_url' src-tauri/src/search src-tauri/tests --glob '*.rs'
rg -n 'DiscoveryHint.*(into|as_)|hint.*(into|as_).*(provider|canonical)|provider_values.*hint|profile_key|source_key.*(match|==)|incomplete|requires_resolution|ResolutionCount|candidate.*(discovered|processed)|diagnostic.*occurrence' src-tauri/src src-tauri/tests --glob '*.rs'
rg -n 'Posting Occurrence|Provider Value|Discovery Hint|Hint Use|providerValues|search_prefilter' CONTEXT.md docs/prd --glob '*.md'
if rg -n 'PostingDiscoveryCandidate|PostingDetailPostingOccurrence|ExecutionPlanPostingDiscoveryFields|PostingDiscoveryFields' src-tauri/src src-tauri/tests --glob '*.rs'; then exit 1; fi
if rg -n 'Legacy.*Occurrence|OccurrenceCompat|compat.*occurrence|forward.*occurrence|old.*discovery.*output' src-tauri/src src-tauri/tests --glob '*.rs'; then exit 1; fi
if rg -n 'discovery_occurrence_requires_resolution|incomplete_occurrence_(warning|count)|occurrence.*(discovered|processed).*count' src-tauri/src/search src-tauri/tests --glob '*.rs'; then exit 1; fi
if rg -n '"kind"\s*:\s*"(provider_value|hint)"|"contributions"\s*:' src-tauri/src/schema src-tauri/tests --glob '*.json'; then exit 1; fi
if rg -n 'postingDiscovery|postingDetail' src-tauri/src/profile_dsl src-tauri/src/schema src-tauri/tests --glob '*.rs' --glob '*.json'; then exit 1; fi
```

The deletion checks must have no active old/compatibility hits; classify any inventory hit and adapt names only to #192's exact landed equivalents. Do not reject canonical-looking keys under `hints`.

## Ticket-specific migration items

- [ ] Re-baseline against #192 and compile all four output sections through its typed immutable value plans.
- [ ] Add schema/Serde parity for valid disjoint output, required URL, unknown/null/tagged/contribution shapes, hint uses, and canonical-looking technical hint keys.
- [ ] Add one shared occurrence and direct Discovery/Detail/Source Live Check/Search Run caller migration.
- [ ] Add provider-ID precedence, conservative URL fallback, sanitized Diagnostics, and the full same/different URL/Source/kind matrix.
- [ ] Remove phase-local implicit location normalization while preserving authored transforms and evaluator order/duplicates.
- [ ] Preserve Search Run `candidate_count` as admitted complete candidates and prove incomplete occurrences are silent and nonpersistable while later complete items continue.
- [ ] Delete `PostingDiscoveryCandidate`, `PostingDetailPostingOccurrence`, `ExecutionPlanPostingDiscoveryFields`, `PostingDiscoveryFields`, duplicate conversion-only DTOs/functions, dual Source-execution outputs, compatibility tags/wrappers/aliases, and superseded tests.
- [ ] Verify no hint conversion or incomplete-candidate visibility was added and no occurrence identity entered cross-Source merge/persistence matching.
- [ ] Replace obsolete required-title/company and old phase/output examples in the canonical docs; add no migration vocabulary.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
