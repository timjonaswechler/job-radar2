# T11a — Establish the Primitive registry and shared parse Primitives

## Result

Discovery and Detail compile and execute authored `parse.type` values through one private parse-family registry and exactly one canonical owner for each of `json`, `xml`, `html`, and `text`. JSON, XML, and HTML share one parsed-document representation and one implementation per type across HTTP and browser inputs; `text` is rejected during compilation and has no runtime variant.

## Readiness and direct blockers

- Parent: [#166](https://github.com/timjonaswechler/job-radar2/issues/166).
- Blocked by: [#178 — T10 — Share byte-preserving HTTP responses and bounded decoding](https://github.com/timjonaswechler/job-radar2/issues/178).
- Blocking: [#180 — T11b — Share evidence-backed select Primitives](https://github.com/timjonaswechler/job-radar2/issues/180).
- Readiness: **Blocked** while #178 is open; current paths and private type sketches remain provisional until readiness review against landed T10.
- Open decision: none. The approved `text` rejection contract below is fixed.

## Consumed contracts

- #166 / PRD Decisions 22–30 and 39–40: immutable typed runtime plans, shared phase-neutral Primitives, separate typed phase semantics, byte-preserving HTTP acquisition, and one canonical implementation/registration per authored Primitive type.
- #166 / PRD “Strategy Set Runtime” module decision: callers cross typed Discovery and Detail operations; the private kernel owns ordering, budgets, Cancellation, and attempt history.
- #178 provides the sole HTTP response-byte, charset validation/selection, strict decoding, checked expansion, transport-Diagnostic, and sanitization owner, including `BoundedDecodedBody` or its landed equivalent. It also preserves browser rendered text as a distinct non-HTTP input.
- #178 preserves `compile_source` with the directly passed Source as authoritative, inspectable `EffectiveSourceProfile`/`effective_profile` for profile access, explicit Source-owned access, and immutable compiled runtime plans.
- `handoff/issue-166-delivery.md` supplies shared readiness, hard-cut, test, migration, Definition-of-Done, and PR-evidence rules.

T11b/#180 owns selector consolidation after this ticket exposes the shared parsed document. T11c/#192 owns value-expression consolidation; neither belongs in this registry slice.

## Current gap

The current repository is still pre-T10, so these names are drafting-time evidence rather than the implementation baseline:

- `src-tauri/src/profile_dsl/documents/parse.rs` and `src-tauri/src/schema/profile-dsl/parse.schema.json` both admit `json`, `xml`, `html`, and `text`, plus optional `charset`.
- `runtime/posting_discovery/document.rs` and `runtime/posting_detail/document.rs` each define their own `ParsedDocument`, `RuntimeItem`, and `parse_response_document` switch. Both call `serde_json`, `roxmltree`, and `HtmlDocument` independently and reject `text` late with `unsupported_parse_type`.
- The same two files also own selectors. This ticket moves parser and shared-document ownership only; selector behavior remains phase-local for T11b.
- `runtime/posting_discovery/strategy.rs`, its pagination path, and `runtime/posting_detail/strategy.rs` invoke the duplicated parsers before selection and extraction.
- Current HTTP phase fetch responses and `ProfileBrowserFetchResponse` both carry `String`; browser-backed Discovery and Detail feed rendered text through the same parser calls. #178 must first replace only the HTTP side with its bounded byte-preserving/decode contract while retaining a separate browser path.
- `parse.charset` is currently unused. #178, not this ticket, makes it validated and executable.

Existing behavior is covered by `posting_discovery_runtime`, `posting_detail_runtime`, the Source Live Check and Search Run/lazy Detail tests, and the Greenhouse, Workday, and SuccessFactors profile regressions. The gap is duplicated parser/document ownership and runtime-only rejection of `text`, not a transport, decoding, selector, phase-output, or caller-product redesign.

## Target delta

### Canonical layout and interface

```text
src-tauri/src/profile_dsl/primitives/
├── mod.rs                 # registry/dispatch only
├── registry.rs            # registry validation and completeness metadata
├── document.rs            # one shared ParsedDocument
└── parse/
    ├── mod.rs             # parse-family registration only
    ├── json.rs            # canonical JSON owner
    ├── xml.rs             # canonical XML owner
    ├── html.rs            # canonical HTML owner
    └── text.rs            # canonical authored key and compile rejection
```

Responsibility-level sketch; private names must adapt to #178 without changing the contract:

```rust
enum CompiledParse {
    Json(parse::json::Plan),
    Xml(parse::xml::Plan),
    Html(parse::html::Plan),
}

enum ParseInput<'a> {
    Http(&'a BoundedDecodedBody),
    BrowserRendered(&'a str),
}

fn parse(
    plan: &CompiledParse,
    input: ParseInput<'_>,
    context: PrimitiveContext<'_>,
) -> PrimitiveOutcome<ParsedDocument>;
```

`primitives/mod.rs`, `registry.rs`, and `parse/mod.rs` contain only family-qualified registration, validation, and dispatch metadata. Primitive-specific compilation, context checks, execution, and parse Diagnostics live in the matching key owner. Parser libraries and document trees remain private in-process details; no public parser/plugin trait or per-Primitive API is introduced.

### Parsing invariants

1. HTTP parsing accepts only T10's successfully decoded bounded body. Parse code never reads raw bytes, response headers, BOMs, charset labels, final URLs, status, or transport errors.
2. Browser rendered text remains a distinct typed input. It is not wrapped as an HTTP response/decoded body, charged as HTTP response bytes, or claimed to satisfy HTTP decoding/allocation guarantees.
3. HTTP and browser inputs use the same canonical JSON/XML/HTML implementations; provenance remains typed at their boundary even if parsing borrows a common text view internally.
4. Runtime receives immutable compiled parse plans and typed inputs only. Raw authored JSON does not reach parsing.
5. Discovery and Detail retain their typed inputs, outputs, acceptance, reducers, Diagnostic context/order, and selection/extraction semantics.
6. A parse failure produces no partial `ParsedDocument` or phase output. It emits one runtime Structured Diagnostic at the landed parse path, with the Strategy key and a stable non-provider-specific parser error detail, in Strategy/operation order.
7. If decoding, a cumulative budget, or Cancellation prevents a decoded HTTP body, parsing is not invoked and adds no parse Diagnostic.
8. T10 remains the only owner of authored charset compilation, authored → BOM → HTTP `Content-Type` → UTF-8 precedence, declaration conflicts, strict decoding, checked expansion, transport metadata, and sanitization. Parse files contain no decoder or charset/provider rule.

### `text` compilation

`parse/text.rs` owns the authored key but exposes no Discovery or Detail executable plan. Compiling a complete Source/Profile with `parse.type: "text"` in either phase emits exactly one error-severity compiler Diagnostic:

- responsibility/code: `parse_type_unavailable_in_phase`, unless #178 lands one equivalent stable unavailable-capability code, in which case use it directly without an alias;
- path: the concrete schema-v3 `.../parse/type` path;
- context: Strategy key;
- typed details: `{ parseType: "text", phase }`.

Compilation produces no plan. `CompiledParse` and `ParsedDocument` have no runtime-reachable `Text` variant, and the late `unsupported_parse_type` branches are deleted. Unknown parse values remain schema/Serde-invalid.

### Parse-family completeness

Completeness is proven for the `parse` family only, with registry identity `(family, authored_type)`:

1. A production-backed test extracts the real keys from `parse.schema.json`, compares them with an exhaustive `ParseType` enumeration/match, and compares both with real compiled registrations. JSON/XML/HTML each have exactly one executable registration; `text` has exactly one compiler-rejection registration; no production registration exists outside that key set.
2. The same registry validator receives synthetic descriptor sets with one omitted registration and one duplicated `(family, authored_type)` registration. These fail deterministically without mutating, feature-gating, or conditionally compiling the production registry.
3. A deterministic filesystem inventory proves that the four key files are the only canonical key owners. Repository-wide searches identify every parser-library entry point, parsed-document declaration/alias/import, dispatch function, and parser call site; review proves behavior exists only in the matching canonical owner. Self-reported owner-path metadata is not ownership evidence.

The infrastructure may support later families, but this ticket adds no selector, value, fetch, pagination, extraction, transform, predicate, or compatibility/legacy registration and makes no global-completeness claim.

## Dependency and deletion decision

| Dependency | Category and decision |
|---|---|
| Compiled plans, registry, parsed trees, parser libraries | In-process concrete implementation; no trait or adapter |
| T10 decoded HTTP body | Immutable typed input consumed directly; no wrapper or second decoder |
| HTTP endpoints | Already behind T10's production reqwest and ordered scripted adapters; parse files never call them |
| Browser runtime | Preserve T10's production client and deterministic fake; rendered text stays a separate input |
| Phase acceptance/reducers/outputs | Remain in typed Discovery/Detail adapters |
| SQLite and higher-level callers | No persistence change; regression callers only unless T10's landed phase interface requires direct migration |

**Deletion test:** Removing the shared parse module would force parse dispatch, JSON/XML/HTML library integration, shared document ownership, parse Diagnostics, compiler context validation, and `text` rejection back into both Discovery and Detail and later Detection. A module that merely forwards to behavior centralized elsewhere fails this test.

## Examples

1. **JSON over HTTP:** T10's scripted adapter supplies bounded bytes and T10 creates the decoded body. Discovery and Detail both invoke `parse/json.rs`; the parser sees no transport or charset metadata.
2. **HTML from browser:** the browser client returns rendered text through `BrowserRendered`; `parse/html.rs` handles it without constructing an HTTP value or debiting HTTP response bytes.
3. **Charset-declared XML:** T10 validates `windows-1252`, selects and applies the encoding, and passes decoded text. `parse/xml.rs` performs XML parsing only.
4. **Unsupported text:** Discovery or Detail compilation reports the approved error at `.../parse/type` and produces no plan; runtime has no text fallback.
5. **Decode failure/Cancellation:** T10 emits its owned terminal/Diagnostic before a decoded input exists; the parse registry records no invocation and returns no partial document/output.

## Scope

- Add the top-level Primitive registry infrastructure and register only the parse family.
- Add exactly one canonical owner each for JSON, XML, HTML, and authored `text`.
- Move parse compilation/context validation, execution, and Diagnostics into those owners; add one shared `ParsedDocument` used by both phases.
- Preserve T10's decoded HTTP input and distinct browser-rendered input without moving any decoding responsibility.
- Migrate direct parser/phase-adapter call sites. Treat Search Run Discovery (`src-tauri/src/search/run/execution.rs`), Source Live Check Discovery/Detail (`src-tauri/src/checks/source_live/mod.rs`), and lazy Detail (`src-tauri/src/search/posting/service.rs`) as regression callers; edit them only if T10's landed phase interface requires it.
- Keep phase-specific selectors, cardinality, pagination interpretation, matching, extraction, acceptance, reducers, and output validation in their current phase owners.
- Add real parity, synthetic missing/duplicate validator, filesystem inventory, and repository-search ownership evidence.
- Delete duplicated phase-local parser functions, parsed-document definitions, parse switches/Diagnostics, late text branches, forwarding wrappers/aliases/compatibility registrations, and superseded parser-only tests after equivalent interface coverage exists.
- Preserve Greenhouse, Workday, and SuccessFactors through generic behavior only.

## Adjacent non-goals

- Selector consolidation or movement of `RuntimeItem`: T11b/#180.
- `first_non_empty`, capture, or other value-expression consolidation: T11c/#192.
- Enabling a text document model, text selectors, or text extractors.
- Moving or changing T10 response-byte limits, HTTP adapters, charset/decoding precedence, browser ceilings, transport sanitization, budget, or Cancellation behavior.
- Detection parse convergence, later Primitive-family registration, Candidate Resolution, persistence/status changes, or browser-to-HTTP conversion.
- JSON-LD, microdata, feeds, arbitrary XPath, scripts, recursive workflows, or provider-specific parsers.

## Acceptance and validation

| Case | Expected observable result | Test or static/manual check |
|---|---|---|
| JSON Discovery and Detail over HTTP | Both phases preserve outputs and use only `parse/json.rs` | External compiled phase tests with T10 scripted HTTP adapter |
| Invalid JSON | One ordered parse Diagnostic with Strategy key; no partial document/output | Discovery and Detail phase tests |
| XML Discovery and Detail | Both phases preserve outputs and use only `parse/xml.rs` | Cross-phase external tests |
| Invalid XML | Stable parse Diagnostic; no phase-local XML branch or partial output | External phase tests plus implementation search |
| HTML over HTTP and browser | Both inputs use `parse/html.rs`; browser remains non-HTTP | Phase tests with scripted HTTP and deterministic browser fake |
| Charset-declared XML/HTML | T10 decodes; parse owners receive text only | T10 regression, phase test, and charset search |
| Decode failure | T10 Diagnostic only; parser not invoked | Scripted HTTP phase test |
| Pre-/mid-Cancellation or byte-budget exhaustion before decode | Existing typed terminal; no parse invocation/output or persistable Cancellation Partial Completion | T10/Strategy Set and caller regressions |
| `text` in Discovery | One compiler error at `.../parse/type`; no plan | External compiler test |
| `text` in Detail | Same rejection; no runtime Text variant/fallback | External compiler test plus static search |
| Unknown parse value | Rejected by schema/Serde before compiler/runtime | Schema validation test |
| Real registry parity | Exact schema ↔ exhaustive `ParseType` ↔ production registration equality | Production-backed completeness test |
| Missing/duplicate registration | Synthetic sets fail deterministically without production mutation | Focused negative registry tests |
| Canonical ownership | Exactly four key files and one matching parser implementation each | Filesystem inventory and reviewed repository searches |
| Later-family boundary | No non-parse registration or completeness placeholder | Registry review/search |
| Phase behavior | Selection, matching, extraction, acceptance, ordering, and Diagnostics remain phase-typed and unchanged | Discovery/Detail regressions |
| Production callers | Search Run, Source Live Check, and lazy Detail behavior is unchanged | Existing caller tests |
| Acceptance profiles | Greenhouse, Workday, and SuccessFactors outputs/order remain unchanged without provider branches | Three profile regression targets |
| Deletion | No old/renamed phase parser, duplicate document type, wrapper, alias, or compatibility registration remains | Reviewed searches below |

Tests primarily cross real compilation and the landed typed Discovery/Detail operations. They use the real registry, parser, decoder, and Strategy Set runtime with T10's ordered scripted HTTP adapter or the deterministic browser fake. Narrow in-module parser-library edge tests may supplement, but not replace, caller-facing phase tests.

### Focused commands

Inspect #178's landed target names and substitute the exact equivalent where it renamed a phase target:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test primitive_registry
cargo test --manifest-path src-tauri/Cargo.toml --test schema_validation
cargo test --manifest-path src-tauri/Cargo.toml --test compiler_resolution
cargo test --manifest-path src-tauri/Cargo.toml --test http_response_runtime
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

## Ticket-specific migration items

- [ ] Re-baseline against #178 and inventory every parse schema key, Rust/compiled variant, registration, parser call site, shared-document type, Diagnostic, test, and regression caller.
- [ ] Add family-qualified registry validation, the parse registrations, four canonical owner files, and one shared parsed document.
- [ ] Add real parity plus synthetic missing/duplicate tests without altering production registration.
- [ ] Route HTTP only from T10's decoded body and browser only from its rendered-text variant.
- [ ] Move direct parser/phase-adapter callers; justify any higher-level edit to Search Run, Source Live Check, or lazy Detail from the landed T10 interface.
- [ ] Delete both phase-local `parse_response_document` functions, duplicate `ParsedDocument` definitions, parser switches/Diagnostics, `unsupported_parse_type` branches, wrappers, aliases, compatibility registrations, and superseded parser-detail tests.
- [ ] Keep phase-local selectors/`RuntimeItem` for T11b and keep every later family outside registry completeness.
- [ ] Review and classify every hit from:

```bash
find src-tauri/src/profile_dsl/primitives/parse -maxdepth 1 -type f -name '*.rs' -print | sort

rg -n 'serde_json::(from_str|from_slice|from_reader|Deserializer)|JsonValue::from_str|Value::deserialize|roxmltree::Document::parse(_with_options)?|HtmlDocument::from|dom_query::Document::from' \
  src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives --glob '*.rs'

rg -n 'ParsedDocument|type\s+.*Parsed|use\s+.*ParsedDocument|fn\s+[A-Za-z0-9_]*parse[A-Za-z0-9_]*|parse_response_document|parse_document|parse_(json|xml|html)|primitives::parse|[A-Za-z0-9_:]+::parse\(|CompiledParse|ParseType::(Json|Xml|Html|Text)|match\s+.*parse' \
  src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives src-tauri/tests --glob '*.rs'

rg -n 'unsupported_parse_type|CompiledParse::Text|ParsedDocument::Text|ParseType::Text' \
  src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'

rg -n 'charset|encoding_rs|BOM|content.?type|decode|from_utf8|ResponseHeaders|BoundedHttpResponse' \
  src-tauri/src/profile_dsl/primitives/parse --glob '*.rs'

rg -n 'ProfileBrowserFetchResponse|BrowserRendered|BoundedDecodedBody|BoundedHttpResponse|response_bytes' \
  src-tauri/src/profile_dsl src-tauri/tests --glob '*.rs'

rg -n 'register|registration|registry|legacy|compat|placeholder|forward' \
  src-tauri/src/profile_dsl/primitives --glob '*.rs'

rg -n 'SourceDocument|SourceProfileDocument|serde_json::Value|greenhouse|workday|successfactors|profile_key|source_key.*(match|==)' \
  src-tauri/src/profile_dsl/runtime src-tauri/src/profile_dsl/primitives --glob '*.rs'
```

Expected classification: parser-library execution only in the matching JSON/XML/HTML owner; one shared `ParsedDocument`; delegation-only dispatch/call sites; `ParseType::Text` only in compiler rejection/registration and tests; no charset/decoder implementation in parse files; browser remains distinct; registry scope is parse-only; and no provider/profile/Source-key runtime dispatch survives.

All other delivery, testing, migration, Definition-of-Done, and PR-evidence requirements follow `handoff/issue-166-delivery.md`.
