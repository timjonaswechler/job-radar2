# T11b — Share evidence-backed select Primitives

## Result

Discovery, Detail, and Discovery sitemap pagination execute the six currently authored Select behaviors through one canonical select-family implementation. The Profile Compiler produces typed selector plans, rejects evidenced syntax and static context errors before execution, and leaves phase-specific cardinality, matching, output, budgets, and Cancellation behavior with the typed phase operations.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#179 — T11a — Establish the Primitive registry and shared parse Primitives](https://github.com/timjonaswechler/job-radar2/issues/179).
- Blocking: [#192 — T11c — Share evidence-backed value Primitives and compile typed value contexts](https://github.com/timjonaswechler/job-radar2/issues/192).
- Readiness: **Blocked** while #179 remains open; re-baseline this draft against its landed registry, parsed-document types, compiler, typed phase operations, Diagnostics, tests, and callers before assignment.
- Open decision: none. The selected scope is the evidence-only six-key Select family; URL-component selection remains deferred.

## Consumed contracts

- #166 / PRD Decisions 7, 22–23, 30, and 39–40: runtime consumes immutable typed plans; shared phase-neutral Primitives have one canonical implementation and family-qualified registry entry.
- #166 / PRD “Strategy Set Runtime” module decision: public callers cross typed Discovery and Detail operations; phase adapters retain typed inputs, outputs, acceptance, and reducers.
- #179 provides the family-qualified Primitive registry and validator, shared parsed-document/item representations, canonical JSON/XML/HTML parsing, typed compiled plans and phase operations, distinct bounded HTTP-decoded/browser-rendered inputs, and compiler rejection of unsupported Discovery/Detail `parse.type: "text"`.
- Compilation continues to treat the directly supplied Source as authoritative, exposes profile-based `EffectiveSourceProfile`/`effective_profile`, keeps Source-owned access distinct, and sends no raw authored JSON to runtime.
- `handoff/issue-166-delivery.md` owns shared readiness, hard-cut, test-seam, migration, deletion, Definition-of-Done, and PR-evidence rules.

## Current gap

This section describes the repository while #179 is still open and is provisional until readiness review.

- `src-tauri/src/profile_dsl/documents/select.rs` and `src-tauri/src/schema/profile-dsl/select.schema.json` both admit exactly `document`, `json_path`, `xml_element`, `xml_text`, `css`, and `sitemap_urls`; no URL-component variant exists.
- `compiler/capabilities.rs` checks broad parse/select compatibility but admits `sitemap_urls` for any XML Strategy placement. `execution_plan/posting_discovery.rs`, `posting_detail.rs`, and `capabilities.rs::clone_select` copy authored Select values instead of compiling syntax and placement into typed selector plans.
- `runtime/posting_discovery/document.rs::select_items` and `runtime/posting_detail/document.rs::select_detail_document` duplicate Select dispatch. Discovery requires JSON collections and preserves ordered XML/CSS collections; Detail accepts one selected value unless its explicit collection-match path authorizes a collection.
- `src-tauri/src/simple_json_path.rs`, both phase-local `values.rs` files, and both `extract/fields.rs` paths split or duplicate constrained JSONPath, CSS, and XML traversal used by Select and Field Expressions.
- `posting_discovery/document.rs::select_sitemap_url_items` implements ordered `<loc>` extraction, whitespace normalization, optional regex filtering, and text-item creation. Only `posting_discovery/pagination.rs` calls it, while unsupported placements fail late at runtime.
- Current evidence is concentrated in `compiler_semantic_validation.rs`, `schema_validation.rs`, `posting_discovery_runtime.rs` and its pagination/cancellation modules, `posting_detail_runtime.rs`, Source Live Check/Search Run/lazy Detail tests, and the Greenhouse, Workday, and SuccessFactors regressions.

The gap is duplicate selector compilation/traversal and late syntax/context failure—not a redesign of phase outputs, value expressions, matching, sitemap budgets, or URL components.

## Target delta

### Canonical family and compiled plans

Extend #179's registry with the `select` family only. Each authored key has one behavior owner under `src-tauri/src/profile_dsl/primitives/select/`: `document.rs`, `json_path.rs`, `xml_element.rs`, `xml_text.rs`, `css.rs`, and `sitemap_urls.rs`. `select/mod.rs`, `primitives/mod.rs`, and `primitives/registry.rs` contain registration/dispatch metadata only.

Responsibility sketch; private names may adapt to #179:

```rust
enum CompiledSelect {
    Document(select::document::Plan),
    JsonPath(select::json_path::Plan),
    XmlElement(select::xml_element::Plan),
    XmlText(select::xml_text::Plan),
    Css(select::css::Plan),
}

struct CompiledSitemapUrlSelect(select::sitemap_urls::Plan);
```

The Profile Compiler validates syntax and parsed-document/phase/placement compatibility, then stores immutable typed plans. JSONPath uses the existing constrained grammar, CSS uses the existing selector parser, and `sitemap_urls.urlPattern` uses Rust-regex syntax. An error-severity compiler Diagnostic produces no plan and therefore no HTTP/browser request.

Shared selectors return ordered selected values/items and Structured Diagnostics. Discovery retains collection shape and item order. Detail retains exactly-one behavior and its existing explicit collection-match authorization. Phase acceptance, extraction, reducers, output validation, and runtime no-match/result-shape behavior remain phase-owned.

### Sitemap placement

`sitemap_urls` is a distinct XML Discovery sitemap-pagination plan:

- a present `childSitemapSelector` or `postingUrlSelector` accepts only `sitemap_urls`; every other Select type is a compiler error;
- omitted `postingUrlSelector` selects every non-empty normalized `<loc>` in document order;
- omitted `childSitemapSelector` performs no child-sitemap traversal;
- `sitemap_urls` is invalid as an ordinary Discovery Strategy selector and in every Detail placement;
- order, whitespace normalization, optional regex filtering, and cumulative `maxRequests`, `maxItems`, and `maxDepth` accounting remain unchanged. Selection owns neither I/O nor a separate traversal budget.

### Evidence-preserving XML grammar

T11b does not invent XPath or a new XML syntax-error boundary.

- `xml_element.element` is the schema-admitted non-empty string. The complete string is compared literally and case-sensitively with `tag_name().name()` for the current node and descendants in `node.descendants()` order. Prefixes and XPath-looking characters have no operator meaning.
- `xml_text.textPath` is schema-admitted non-empty text. Runtime trims it; `"."` or a value empty after trimming selects the current node. Otherwise it splits on `/`, discards empty segments, and treats each remaining segment as a literal, case-sensitive local element name. One segment selects matching descendants. For multiple segments, the first may identify the current element or matching direct children; later segments identify matching direct children. Selected text is the ordered join of descendant text nodes with one space.
- Wildcards, attributes, predicates, parent steps, namespaces, escaping, and functions are not added. XPath-looking input remains literal and receives no `selector_syntax_invalid` Diagnostic.
- Empty authored `element`/`textPath` values remain schema errors at the exact `.../element` or `.../textPath` suffix. A non-XML top-level XML Select is reported at `.../discovery/strategies/{strategyIndex}/select` or `.../detail/strategies/{strategyIndex}/select`. A non-XML XML Field Expression is reported at its expression root: `.../extract/fields/{field}`, `.../extract/fields/postingMeta/{key}`, `.../match/left`, `.../match/right`, or nested `.../parts/{partIndex}/value`, as applicable. Use #179's exact schema-v3 prefix and stable Strategy key; these suffix responsibilities may not drift.

Canonical XML traversal is exercised through complete compiled Sources and typed Discovery/Detail operations for literal/case-sensitive matching, one- and multi-segment paths, `.`, empty results, and XPath-looking literals. Migrated XML Field Expression calls retain existing cardinality, transforms, missing-value behavior, paths, and Diagnostics.

### Diagnostics, ordering, and retention

Reuse equivalent stable #179 codes. If none exist, use:

- `selector_syntax_invalid`: error-severity `compiler` Diagnostic for invalid JSONPath, CSS, or sitemap regex. Path: exact `.../jsonPath`, `.../selector`, or `.../urlPattern` member. Required details: `{ "selectType", "phase", "placement", "field", "error" }`; placement is `strategy_select`, `field_expression`, `sitemap_child`, or `sitemap_posting`, and `field` is exactly `jsonPath`, `selector`, or `urlPattern`. Add Strategy key when available.
- `selector_unavailable_in_context`: error-severity `compiler` Diagnostic for invalid phase/placement. Path ends at the concrete `.../select/type`, `.../pagination/childSitemapSelector/type`, or `.../pagination/postingUrlSelector/type`. Required details: `{ "selectType", "phase", "placement" }`, plus optional `parseType` only when a parsed-document context exists.
- Parse/document mismatch preserves #179's equivalent code or falls back to `incompatible_parse_select_capability` at the concrete Select/Field Expression path with `{ "parseType", "capabilityType" }` and Strategy key.

Error details are stable and sanitized: no response body or unsanitized transport value. Runtime no-match, wrong-shape, Field Expression missing/cardinality, and Detail multiplicity Diagnostics retain their phase paths and Strategy keys. Diagnostic order follows Strategy, pagination, and item order; selectors do not reorder map-backed data.

Cancellation remains the typed Strategy Set/Search Run control path. If it or a cumulative ledger stops work before a document is available, no selector runs; selection performs no I/O and releases no partial phase output after Cancellation.

### Completeness and ownership

- A production-backed parity test compares all six real schema keys, exhaustive Rust Select variants, and real compiled registrations, with exactly one registration per key and the restricted `sitemap_urls` context recorded.
- Synthetic descriptor-set tests reject missing or duplicate `(family, authored_type)` registrations without mutating or feature-gating the production registry.
- Deterministic filesystem inventory and implementation searches prove one canonical behavior owner per key; registry metadata alone is insufficient.
- Parse-family parity remains unchanged. No completeness claim or placeholder registration is added for value, extract, filter, predicate, transform, fetch, pagination, or other families.
- Existing Field Expression call sites may use canonical JSONPath/CSS/XML traversal, but their cardinality, transforms, combine/fallback, canonical-field mapping, and T11c registry ownership do not move.
- No `url_component` schema shape, enum variant, registration, alias, placeholder, or runtime rejection stub is added.

## Dependency and deletion decision

Compiled selector plans, selector libraries, parsed documents/items, and registry metadata are in-process concrete code. HTTP remains behind the production/deterministic adapters already used by typed phase operations; browser fetch remains behind #179's landed production/fake seam. Selectors consume parsed input and perform no I/O. The existing sitemap queue/ledger retains traversal and budget ownership. No selector, document-tree, registry, pagination, or policy trait/port is introduced.

**Deletion test:** Removing the select-family boundary would force selector-plan compilation, static syntax/context validation, JSONPath/CSS/XML traversal, selector Diagnostics, and sitemap URL rules back into Discovery, Detail, pagination, and Field Expression callers. A family that only forwards to behavior retained elsewhere fails this test.

## Examples

1. `{ "select": { "type": "json_path", "jsonPath": "$.jobs" } }` is compiled once. Discovery applies its collection requirement; Detail applies its single/authorized-collection contract. `$.jobs[*]` fails compilation before any provider response.
2. `{ "select": { "type": "css", "selector": "article.posting" } }` preserves Discovery document order. Detail emits its existing multiplicity Diagnostic when several nodes match.
3. `{ "select": { "type": "xml_text", "textPath": "job/title" } }` follows literal element names; `.` selects the current node. XPath-looking characters remain literal rather than gaining semantics or a new compiler rejection.
4. A sitemap `postingUrlSelector: { "type": "sitemap_urls", "urlPattern": "/jobs/" }` returns ordered normalized matching `<loc>` values to the existing bounded queue. A CSS selector in that placement, or `sitemap_urls` at Strategy-level Discovery/Detail, produces no plan/request.

## Scope

- Add six canonical select owners and select-family registrations to #179's landed registry.
- Compile selector syntax and static context into typed immutable plans while preserving the XML literal grammar.
- Route Discovery, Detail, and sitemap pagination directly through canonical selectors.
- Route existing Field Expression JSONPath/CSS/XML traversal through canonical engines where required to eliminate duplicate traversal without moving value semantics.
- Preserve phase cardinality/matching/output, Diagnostic ordering, sitemap queue/ledger bounds, and Cancellation in their existing owners.
- Add real parity, synthetic registry-invariant, filesystem-inventory, ownership, compiler, and typed phase-operation coverage.
- Migrate deterministic fixtures only where compiled selector boundaries or static failure timing change.
- Delete phase-local selector switches/helpers, copied authored Select plans, late static-context runtime branches, forwarding wrappers, aliases, and superseded implementation-detail tests after callers and behavior tests move.
- Preserve Greenhouse, Workday, and SuccessFactors through generic selector behavior.

## Adjacent non-goals

- URL-component selection; its representation, input source, component set, decoding, cardinality, and Diagnostics require a separate approved ticket.
- T11c/#192 value-expression consolidation, `first_non_empty`, combine/fallback, value-family registration, or broad phase-context expression validation.
- Changing Discovery collection semantics, Detail single/collection-match semantics, extraction, acceptance, reducers, output contracts, or canonical fields.
- Detection selection, `parse.type: "text"`, XPath, JSONPath filters/wildcards, DOM scripting, JSON-LD/microdata, arbitrary selector plugins, or new selector types.
- Moving sitemap traversal/budget ownership, registering other Primitive families, or adding provider-specific behavior.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| `document`, both phases | Existing JSON/XML/HTML root/item behavior and order | External compiled Discovery and Detail tests |
| JSONPath, both phases | One canonical traversal; Discovery collection and Detail single/match behavior preserved | External cross-phase tests |
| Invalid JSONPath | Compiler Diagnostic at `jsonPath`; no plan/fetch | External compiler test with scripted adapter |
| JSONPath missing/wrong Discovery shape | Existing ordered runtime no-match/result-shape Diagnostic; no partial output | Both phase tests |
| XML element | Literal case-sensitive local-name matching and Discovery order; Detail cardinality preserved | External cross-phase tests |
| XML text | One segment, multiple segments, `.`, and text join preserve current behavior | External cross-phase tests |
| XML boundary | Case mismatch/XPath-looking values remain literal; empty schema values and non-XML contexts use exact paths | Schema/compiler plus phase tests |
| CSS, both phases | One canonical engine; Discovery many and Detail exactly-one behavior preserved | External cross-phase tests |
| Invalid/no-match/multiple CSS | Syntax fails compilation; runtime missing/multiple Diagnostics remain stable | Compiler and Detail tests |
| Parse/select mismatch | Compiler Diagnostic at concrete Select/expression path; no plan | Table-driven compiler tests |
| Sitemap posting selector present/omitted | Ordered normalized regex matches, or all non-empty `<loc>` values; existing limits charged | Discovery pagination tests |
| Sitemap child selector present/omitted | Existing traversal order/limits, or no child request | Pagination test with scripted request log |
| Wrong child/posting selector | Each of the other five Select types rejects with unavailable-context Diagnostic and no request | Table-driven compiler + scripted adapter tests |
| Invalid sitemap regex | Syntax Diagnostic at `urlPattern`; no plan/request | Compiler + scripted adapter test |
| `sitemap_urls` ordinary Discovery/Detail | Unavailable-context Diagnostic; no plan | External compiler tests |
| JSONPath Field Expression | Existing scalar conversion, cardinality, missing behavior, transforms, paths, and Diagnostics | Compiled Discovery/Detail operations |
| CSS text/attribute Field Expressions | Existing missing-node/attribute, cardinality, transforms, paths, and Diagnostics | Compiled Discovery/Detail operations |
| XML text/element Field Expressions | Canonical literal traversal with existing value semantics | Compiled Discovery/Detail operations |
| Cancellation/budget stop | No later selector work or partial output; existing terminal/usage/Diagnostic order | Strategy Set, Search Run, and pagination regressions |
| Real parity | Exact six-key schema/Rust/registration equality and one registration per key | Production-backed registry test |
| Missing/duplicate descriptor | Deterministic registry rejection without production mutation | Synthetic registry tests |
| Canonical ownership/deletion | One owner per key; no phase-local/renamed duplicate, wrapper, or late static-context fallback | Filesystem inventory and reviewed searches |
| URL component/later families | Schema/Serde rejection and no placeholder or false completeness claim | Parity/static review |
| Acceptance profiles/callers | Greenhouse, Workday, SuccessFactors, Source Live Check, Search Run, and lazy Detail behavior remains generic | Relevant existing regressions |

Tests cross complete Source compilation and the typed Discovery/Detail operations landed by #179. Private tests are limited to narrow grammar/library edges. If #179 changes target names, record the exact landed replacements without dropping acceptance cases.

### Focused commands

```bash
# Use #179's landed target names; retain current names where still present.
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

find src-tauri/src/profile_dsl/primitives/select -maxdepth 1 -type f -name '*.rs' -print | sort
rg -n 'enum\s+Select|CompiledSelect|Select::(Document|JsonPath|XmlElement|XmlText|Css|SitemapUrls)|select_items|select_detail_document|select_sitemap_url_items|unsupported_select_type|unsupported_sitemap_url_selector' src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'
rg -n 'resolve_simple_json_path|parse_simple_json_path|SimpleJsonPath|json_path_select|jsonPath|Matcher::new|select_matcher|xml_descendant_elements|xml_path_(nodes|texts)|xml_node_text|descendants\(\)' src-tauri/src/profile_dsl src-tauri/src/simple_json_path.rs src-tauri/tests --glob '*.rs'
rg -n 'sitemap_urls|SitemapUrls|urlPattern|postingUrlSelector|childSitemapSelector|<loc>|sitemap_url_pattern' src-tauri/src/profile_dsl src-tauri/src/schema src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'url_component|urlComponent|UrlComponent' src-tauri/src/profile_dsl src-tauri/src/schema src-tauri/tests --glob '*.rs' --glob '*.json'
rg -n 'register|registration|registry|legacy|compat|placeholder|forward' src-tauri/src/profile_dsl/primitives --glob '*.rs'
rg -n 'SourceDocument|SourceProfileDocument|serde_json::Value|greenhouse|workday|successfactors|profile_key|source_key.*(match|==)' src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives --glob '*.rs'
```

Every search hit is classified. Also apply the shared full-suite requirement from `handoff/issue-166-delivery.md`; run `npm run build` if landed serialized/schema changes affect frontend consumers.

## Ticket-specific migration items

- [ ] Re-baseline against #179 and inventory every Select schema key, Rust variant, registration/plan, compatibility check, traversal entry point, Diagnostic, test, and production caller.
- [ ] Add exactly six select-family registrations/owners and preserve parse-family registrations unchanged.
- [ ] Replace copied authored Select values with typed selector plans and migrate Discovery, Detail, and sitemap pagination callers.
- [ ] Migrate JSONPath/CSS/XML Field Expression traversal without moving value cardinality, transforms, fallback/combine, output mapping, or T11c registrations.
- [ ] Delete `clone_select`, `select_items`, `select_detail_document`, `select_sitemap_url_items`, duplicate phase-local CSS/XML traversal and syntax checks, and late `unsupported_select_type`/`unsupported_sitemap_url_selector` branches after equivalent coverage exists.
- [ ] Delete forwarding wrappers, aliases, compatibility registrations, renamed duplicates, and superseded implementation-detail tests.
- [ ] Verify sitemap selectors admit only `sitemap_urls`, preserve both omission behaviors, and issue no request for invalid syntax/placement.
- [ ] Verify no URL-component shape/placeholder and no completeness claim for later Primitive families.
- [ ] Run and classify the focused ownership/deletion searches above; legitimate hits are authored representations, one compiled registration/owner per key, and thin typed callers only.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
