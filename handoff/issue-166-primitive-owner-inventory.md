# Issue #166 / D-005 — complete currently-admitted authored Primitive inventory

Status: read-only Phase-2 analysis; accepted D-001–D-013 were treated as fixed. Generated outside the repository on 2026-07-18. No project/source or GitHub state was changed.

> **Adversarial classification correction:** This inventory enumerates every executable nested authored option so none can escape Schema/Serde/implementation parity. It does **not** require each nested literal to become an independent registry identity, source file, or ticket. `handoff/issue-166-restructuring-plan.md` defines the governing identity rule: independently dispatched executable DSL types receive registrations; HTTP methods/bodies, parameter locations, waits/interactions, and Acceptance properties may remain parent-family-owned options when the landed implementation proves that classification. Every option still needs one owner and parity/deletion evidence.

## 1. Inventory boundary and conclusions

“Currently admitted” means an authored value accepted by the checked-in schema-v2 Profile DSL JSON Schema and/or its direct Serde document model. The inventory includes nested executable discriminators and untagged executable authored shapes, not merely the top-level Parse/Select/Field Expression enums. It separately records Detection constructs because `detect` is part of the authored Source Profile DSL, even though the current schema does not express Detection as the same Strategy shape as Discovery/Detail.

It excludes metadata-only enums (`SourceProfileKind`, support/evidence levels, Diagnostic category/severity), canonical phase output field names, and source/search domain state: these do not select Primitive execution. It also excludes target-only schema-v3 additions not admitted today (`first_non_empty`, `first_accepted`, `all_required`, `at_least`, `collect_all`, URL-component select, JSON-LD/microdata/feed variants). Those may enter the global gate only in the same slice that really admits and implements them.

**Main result:** there is no Primitive registry or compiled-registration catalog today. Fetch and Pagination get dedicated compiled enums; Parse and Select and every Filter/Capture/Value/Transform/Acceptance object are mostly copied raw into the Execution Plan. Runtime behavior is split across Discovery, Detail, Detection, compiler helper switches, and browser conversion. Therefore no current test can prove D-005 global completeness. A correct restructuring needs the explicit family owners below; T11a/T11b/T11c alone cover only Parse, Select, and Value.

Authoritative architecture evidence: `docs/prd/declarative-profile-strategy-algebra.md:23,114-115,147-148,237` calls acquisition, pagination, parsing, selection, predicates, captures, extraction, transforms, acceptance, and diagnostics part of the catalog and requires one implementation file per authored Primitive plus a schema/document/registration completeness test. D-005 requires real family owners and same-slice duplicate deletion; it prohibits placeholder registrations and registry-owned behavior.

## 2. Current representation and consumers

- Schema entrypoints: `src-tauri/src/schema/profile-dsl/{fetch,pagination,parse,select,extract,transform,strategy}.schema.json`; Detection is inline in `src-tauri/src/schema/source-profile.schema.json:39-154`.
- Serde: `src-tauri/src/profile_dsl/documents/{fetch,pagination,parse,select,extract,transform,strategy,posting_discovery,posting_detail}.rs`; Detection: `src-tauri/src/source_profile/documents.rs:39-143`.
- Plans: `execution_plan/capabilities.rs:8-346` compiles Fetch/Pagination/browser subtypes but `clone_parse`/`clone_select` copy authored documents; `execution_plan/posting_discovery.rs:15-128` and `posting_detail.rs:13-102` clone Filter, Capture, FieldExpression, Acceptance and Match documents.
- Dispatch: compiler-wide switches live in `compiler/{capabilities,boundedness,security}.rs` and `compiler/templates/{fetch,fields}.rs`. Runtime switches are spread across `runtime/posting_{discovery,detail}` plus `runtime/transform.rs`. Detection directly interprets raw profile documents in `source_profile/detection/{mod,http,browser,templates,proposal}.rs`, contrary to the schema-v3 target but accurately describing the current implementation.
- Productive phase callers: Search Run Discovery (`search/run/execution.rs`), Source Live Check Discovery/Detail (`checks/source_live/mod.rs`), lazy posting Detail (`search/posting/service.rs`), and Profile Detection commands/callers. Built-ins `resources/profiles/{greenhouse,workday,successfactors}.json` are the principal production fixtures. The frontend has no separate Primitive type model; `src/features/sources/shared/profile-dsl-schema-catalog.ts:1-35` imports the backend schemas wholesale.

## 3. Variant-level inventory and canonical ownership

“Owner” below is a proposed concrete retained family slice and canonical final file, not a placeholder registry. A plural row lists every authored discriminator explicitly; each listed discriminator gets the stated file pattern with its own key file where PRD Decision 39 requires it.

### A. Acquisition / Fetch and browser actions

| Authored key/variant | Schema ↔ Serde | Current plan / implementation / consumers | Duplicate or missing behavior | Proposed family and canonical owner/file | Direct consumed prerequisites | Same-slice deletion targets |
|---|---|---|---|---|---|---|
| `fetch.mode=http` | `fetch.schema.json:11-27`; `Fetch::Http`, `documents/fetch.rs:9-22` | `ExecutionPlanFetch::Http` + `compile_fetch`, `execution_plan/capabilities.rs:9-28,160-180`; phase-local HTTP rendering/fetch in both `runtime/posting_*/fetch.rs`; Discovery, Detail, Search Run, Live Check, lazy Detail | Two phase fetch clients/request renderers; method/body/template/security/bounds spread across compiler/runtime. Detection HTTP is a third separate implementation. | **Fetch Primitive family slice**, `profile_dsl/primitives/fetch/http.rs` owns authored HTTP config, compilation, rendering and result projection; it consumes rather than absorbs T10 transport. | T9 cumulative budgets; T10 byte-preserving shared HTTP boundary/decoder; Template family | Delete `compile_fetch` HTTP arm, both phase HTTP fetch/render helpers and old request/client DTOs after migration; delete duplicate compiler fetch template/security/boundedness branches moved to owner. Do not delete Detection adapter until T14a activation, but route it to the same Primitive first. |
| `fetch.mode=browser` | schema `:69-87`; `Fetch::Browser`, Serde `:23-31` | `ExecutionPlanFetch::Browser`; phase-local browser branches call `ProfileBrowserClient`; Discovery, Detail, Live Check/lazy Detail/Search Run | Duplicate phase projections and a separate Detection browser-probe conversion; old `ProfileBrowserClient` seam is a D-007 deletion target. | **Fetch Primitive family slice**, `primitives/fetch/browser.rs`; phase-neutral acquisition remains D-007 Browser Acquisition, while this file owns authored fetch compilation/context/projection. | D-007 shared Browser Acquisition foundation and typed phase adapters; T9 phase allowance; Template family | Delete browser arm in `execution_plan/capabilities.rs`, phase-local fetch branches/wrappers, `ProfileBrowserClient`/`render*` and old fakes/exports in the D-007 atomic activation slice. |
| HTTP `method=GET`, `POST` | schema `:18`; `HttpMethod`, Serde `:34-40` | Interpreted in phase fetch request creation; POST body handling tested in `tests/posting_discovery_runtime/post_request_bodies.rs` | Rendering and body legality checks are phase-local/compiler-spread. | Same Fetch family: `fetch/http_get.rs`, `fetch/http_post.rs` (or one key file per actual registry identity if family identity is `http_method`). | HTTP Fetch owner + T10 | Delete duplicated GET/POST request construction and compiler method/body checks. |
| `body.type=json`, `text`, `form` | schema `:38-67`; `RequestBody`, Serde `:42-48` | Raw `RequestBody` survives into plan; Discovery fetch renders JSON/text/form; Detail has the same plan but much thinner evidence | JSON-body pagination mutation exists only in Discovery; templates/security checks are separate. | **Request-body subfamily of Fetch**: `fetch/body/{json,text,form}.rs`; these are real registered authored variants, not registry helpers. | Template family; HTTP POST owner; Pagination for JSON-body parameter injection | Delete body variant switches/render helpers in phase fetch and compiler template/security modules; retain no copied authored body in runtime plan. |
| `wait.type=selector`, `network_idle` | schema `:89-98`; `BrowserWait`, Serde `:57-72` | compiled in `execution_plan/capabilities.rs:220-241`; executed through `runtime/browser.rs:150-172`; also reused raw then reconverted by Detection browser | Shared enum but duplicated Detection conversion/bounds/diagnostics. | **Browser-action subfamily**: `fetch/browser_wait/{selector,network_idle}.rs`, consumed by shared Browser Acquisition and all phase adapters. | D-007 Browser Acquisition; T9/T14c scoped budgets | Delete `compile_browser_wait`, Detection `browser_probe_request` wait conversion, runtime wait dispatch duplicated outside acquisition, and superseded fakes/tests. |
| `interaction.type=click_if_visible`, `click_until_gone` | schema `:100-111`; Serde `:74-90`; duplicate `DetectionBrowserInteraction`, `source_profile/documents.rs:113-130` | compiled in `execution_plan/capabilities.rs:243-275`; Detection reconverts its separate enum in `detection/browser.rs:113-166`; shared runtime executes them | Same authored behavior has two Serde enums, two converters, phase-specific diagnostics. | **Browser-action subfamily**: `fetch/browser_interaction/{click_if_visible,click_until_gone}.rs`; one authored document and compiled action consumed through D-007 module. | D-007 Browser Acquisition; T14c Detection typed contribution adapter | Delete `DetectionBrowserInteraction`, inline Detection schema duplicate in favor of the canonical schema ref, both conversion switches, and old runtime dispatch/fakes in activation. |
| Serde-only `execute_script`, `eval`, `mutate_dom`, `login_flow`, `captcha_bypass` | **Mismatch:** absent/rejected by JSON Schema, but admitted by `BrowserInteraction`, Serde `:91-110`; compiler rejects them in `execution_plan/capabilities.rs:267-275` and security switch `compiler/security.rs:180-185` | No executable plan; rejection stubs only | D-005 prohibits placeholder/rejection registrations for future variants; arbitrary script/login/CAPTCHA are explicitly out of scope. | **No owner: delete admission.** Fetch/browser family hard-removes these Serde variants rather than registering them. | D-001/schema-v3 hard cut or earlier parity repair | Delete five Serde variants and all rejection/security/boundedness match arms/tests; schema remains rejecting. |

### B. Pagination

| Authored variants | Schema ↔ Serde | Current implementation / consumers | Duplication / risk | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|---|---|
| `pagination.type=page`, `offset_limit`, `cursor`, `sitemap` | schema `pagination.schema.json:25-81`; `Pagination`, `documents/pagination.rs:14-82`; compiled enum `execution_plan/capabilities.rs:75-130` | All dispatched in one large `runtime/posting_discovery/pagination.rs:3-340`; Discovery only. Workday consumes `offset_limit`; SuccessFactors consumes `sitemap`; external pagination tests cover all four. | Variant compilation and runtime in central switches; sitemap calls Select and fetch; total/cursor JSONPath helpers are hidden in Discovery strategy. | **Pagination Primitive family slice**: `primitives/pagination/{page,offset_limit,cursor,sitemap}.rs`; per-key compilation, stop logic, diagnostics and narrow tests. | Fetch/HTTP family; T9 budgets/report; Select family for sitemap selectors; Parse/value traversal for `totalPath`/`nextCursorPath` (or compile these paths as Select plans) | Delete `compile_pagination`, `ExecutionPlanPagination` central behavior switch, entire variant dispatch in `runtime/posting_discovery/pagination.rs`, hidden total/cursor extraction helpers, and superseded variant tests after public equivalents. |
| `parameterLocation=query`, `json_body` | schema `:14-16`; Serde `PaginationParameterLocation`, `documents/pagination.rs:7-11` | `strategy.rs:88-106` chooses query vs JSON-body params; JSON-body mutation in Discovery fetch | Crosses Pagination and HTTP body construction; currently a small dispatch outside owner. | Pagination option family: `pagination/parameter_location/{query,json_body}.rs` (or owned explicitly by each pagination key with registry metadata proving no independent variant identity). Classification is **ambiguous** because it is an enum-valued option, but it selects executable behavior and must be in the gate either way. | HTTP JSON body owner | Delete `query_params_for_location` / `json_body_params_for_location` and duplicate mutation logic. |

### C. Parse and Select (accepted focused owners)

| Variants | Current implementation | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|
| Parse `json`, `xml`, `html`, `text` (`parse.schema.json:11`; `ParseType`, `documents/parse.rs:14-20`) | Discovery and Detail each define `ParsedDocument`, parser switch, diagnostics, and late `text` rejection (`runtime/posting_discovery/document.rs:4-57`, `posting_detail/document.rs:4-58`). Plans clone raw Parse. | **T11a Parse family** exactly as accepted: `primitives/parse/{json,xml,html,text}.rs`. `text` is currently admitted but non-executable; T11a’s accepted compile-time unavailability is a real owned rejection for a currently declared phase context, although global review must verify it is not a prohibited future-placeholder registration. | T10 decoded HTTP input; D-007 browser-rendered input | Delete both parser switches/types/late `unsupported_parse_type`, `clone_parse`, duplicate parser tests. |
| Select `document`, `json_path`, `xml_element`, `xml_text`, `css`, `sitemap_urls` (`select.schema.json:21-68`; `Select`, `documents/select.rs:9-31`) | Plans clone raw Select. Discovery/Detail duplicate selector switches; sitemap has another special selector; JSON/XML/CSS traversal is duplicated with Field Expressions. | **T11b Select family**: `primitives/select/{document,json_path,xml_element,xml_text,css,sitemap_urls}.rs`. | T11a shared parsed document; `sitemap_urls` also Pagination | Delete `clone_select`, both phase selector switches, sitemap selector helper, duplicate traversal/syntax/context code and tests. |

### D. Value / extraction, list composition, cardinality, combine and template

| Variants / authored shape | Current implementation / mismatch | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|
| Field Expression `const`, `template`, `source_config`, `posting_meta`, `capture`, `item_field`, `json_path`, `xml_text`, `xml_element`, `css_text`, `css_attribute`, `combine` (`extract.schema.json:50-184`; `FieldExpression`, `documents/extract.rs:17-107`) | Plans clone raw expressions. Near-complete switches, conversion, cardinality and combine are duplicated in `runtime/posting_discovery/extract/fields.rs:8-430` and `posting_detail/extract/fields.rs:9-402`; compiler context/template checks are partial. Discovery has no normal `posting_meta` arm and reaches a late unsupported path. `const` schema allows scalar only but Serde stores arbitrary `serde_json::Value` (`extract.rs:18-24`). | **T11c Value family** for all 12 currently admitted keys: `primitives/value/<key>.rs`. Target-only `first_non_empty` becomes the 13th only when its schema/Serde/executable implementation lands in that slice. | T11b selector traversal; Transform and Cardinality families below; Template syntax family | Delete raw FieldExpression from plans, both phase evaluator switches/converters/combine helpers, partial compiler template/capability switches, late unsupported branches, and superseded tests. Constrain `const` Serde to scalar in same slice. |
| `ListFieldExpression::Single`, `Multiple` (untagged; schema `listFieldExpression`, `extract.schema.json:26-36`; Serde `extract.rs:109-114`) | Used for Discovery `locations`; aggregation/deduplication in `posting_discovery/extract.rs:292+`. Empty array is schema/Serde-admitted. | **Value composition**: `primitives/value/list.rs` owns the two authored shapes and list flattening contract; T11c is appropriate because this is an extraction result shape, not a new Transform. | Value + Cardinality | Delete phase-local list branching/flattening; gate must compare the untagged schema shapes as well as tagged enums. Flag empty-array semantics for owner decision. |
| Cardinality `one`, `first`, `optional`, `all` (`extract.schema.json:8`; `Cardinality`, `extract.rs:8-13`) | Implemented twice in the phase field evaluators/adjacent values modules; semantics apply to every value type. | **Explicit Cardinality family slice**, `primitives/cardinality/{one,first,optional,all}.rs`, landing before T11c consumes compiled cardinality. This keeps T11c focused on lookup/value variants rather than silently owning four omitted Primitives. | Shared typed selected-value sequence from T11b | Delete both phase cardinality helpers/diagnostics/tests and all raw optional-cardinality interpretation. |
| `combine.parts[].value`, optional flag, `join` | `combine` itself is a Value variant; recursive implementation duplicated in both phase evaluators. Schema minItems 1; Serde does not enforce non-empty without schema. | `primitives/value/combine.rs` under T11c owns authored `combine`; `CombinePart` is its nested config, not a separate tagged registry key. | Value recursion, Cardinality, Transform | Delete duplicated combine helpers; add direct-Serde parity for non-empty parts. |
| `FieldExpression.type=template`; all other `templateString` members (fetch URL/body/fields, detection URLs/key/name/source config) | Shared renderer exists at `profile_dsl/template.rs:1-105`, but contexts and validation are split between compiler templates, phase value evaluators, and Detection templates. | **Two explicit layers:** `primitives/template/string.rs` owns the authored template-string grammar/rendering/context-neutral compilation; `primitives/value/template.rs` owns the Value result/cardinality/transforms. This is not duplicate ownership: one is the cross-family template syntax Primitive, one is the tagged value lookup/composition variant. | Typed context contracts from compiler; Value adapter consumes Template syntax | Delete `profile_dsl/template.rs` old location, Detection-specific renderer wrappers where mere forwarding, phase template parsers, and compiler template switches after typed callers migrate. Keep context-specific namespace declaration in phase adapters, not renderer dispatch. |

### E. Transform

Schema keys are at `transform.schema.json:25-70`; Serde variants are `documents/transform.rs:5-38`; all execute through one central match in `runtime/transform.rs:14-103`. Every Field Expression in both phases consumes them.

| Authored variants | Canonical owner | Direct prerequisites | Same-slice deletion / notes |
|---|---|---|---|
| `trim`, `normalize_whitespace`, `html_to_text`, `url_decode`, `slug_to_title`, `dedupe`, `to_string`, `split`, `join`, `regex_replace` | **Transform Primitive family slice**, `primitives/transform/{trim,normalize_whitespace,html_to_text,url_decode,slug_to_title,dedupe,to_string,split,join,regex_replace}.rs`, before T11c. | Typed string/list value sequence; regex engine for regex_replace; HTML text library for html_to_text | Delete `runtime/transform.rs` central behavior match and its private helpers after calls compile to typed transform plans; move narrow tests to key files and preserve external Discovery/Detail tests. `to_string` is currently a no-op because scalar conversion already occurred; owner must make its contract real or remove authored admission, not preserve a placeholder. `url_decode` is lossy (`String::from_utf8_lossy`) and needs explicit retained semantics/security review. |
| Aliases `normalizeWhitespace`, `htmlToText`, `urlDecode`, `slugToTitle`, `toString` (schema also admits them; Serde `alias`) | **Schema/Serde parity but compatibility ambiguity.** They are separate authored strings today, yet serialize canonically to snake case. Under D-001/schema-v3 clean-cut, remove these alias admissions rather than create duplicate canonical implementation owners. Until removed, the global extractor must count them as admitted aliases mapped explicitly to the one canonical key and fail if they silently disappear from parity. |

Potential Serde field mismatch: `Transform::Split` fields are `trim_parts`/`drop_empty` without explicit `rename` (`documents/transform.rs:21-28`), while schema authors `trimParts`/`dropEmpty` (`transform.schema.json:50-51`). Confirm direct-Serde behavior; if Serde expects snake_case, fix in the Transform slice and add parity fixtures.

### F. Predicate / filter / exact match

| Authored variant | Current implementation | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|
| `where[].type=non_empty`, `regex` (`select.schema.json:84-97`; `Filter`, `documents/select.rs:35-44`) | Discovery switch in `runtime/posting_discovery/extract.rs:180-230`; Detail switch in `runtime/posting_detail/strategy.rs:396-461`; raw Filter in plans. | **Predicate Primitive family slice**, `primitives/predicate/{non_empty,regex}.rs`. | T11c compiled Value and typed placement context | Delete both phase filter switches, runtime regex compilation, raw Filter plans, duplicate tests; compiler owns regex syntax before I/O. |
| Detail `match {left,right}` (no discriminator; schema `strategy.schema.json:87-95`; `FieldMatch`, `documents/strategy.rs:20-24`) | Equality and exact-one collection logic in `runtime/posting_detail/strategy.rs:119-393`; raw expressions in plan. | **Predicate family**, `primitives/predicate/equal.rs` owns left/right equality. Exact-one collection selection remains the Detail adapter’s typed phase contract, not generic predicate behavior. | T11c Value + T11b selected collections | Delete `detail_document_matches_field` equality implementation and raw FieldMatch plan. Preserve phase-owned zero/multiple-item diagnostics. |
| PRD target predicates `all`, `any`, `none`, negation, containment, count | Not admitted in current schema/Serde | No current owner/registration. Introduce only with a future real capability slice. | — | Global gate must prove absence rather than add stubs. |

### G. Capture

| Authored construct | Current implementation | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|
| Strategy `captures.<key> = {from, pattern}` (`select.schema.json:99-113`; `CaptureRule`, `documents/select.rs:46-53`) | Discovery and Detail have almost identical `evaluate_strategy_captures` and `apply_capture_rule` files (`runtime/posting_discovery/extract/captures.rs:4-115`, `posting_detail/extract/captures.rs:4-111`). They clone the partial map before each BTreeMap-ordered rule, enabling accidental sequential capture chaining. Plans carry raw rules. | **Capture Primitive family slice**, `primitives/capture/regex.rs` owns pattern compilation, named capture selection, trimming and diagnostics. Phase adapters supply typed source contexts. | T11c compiled Value source context; compiler typed contexts | Delete both capture files, raw CaptureRule plans, runtime regex compilation, partial-map chaining, duplicate tests. T11c’s accepted capture-source hard cut is enforced here jointly: capture sources cannot read partially completed captures. |
| Detection input URL `pattern` plus optional capture-name list; HTTP `regex`; browser `htmlRegex` | Separate regex compilation/capture loops in `source_profile/detection/mod.rs:204+`, `detection/http.rs:117+`, `detection/browser.rs:187+`; mutable maps can overwrite same keys | Same `capture/regex.rs` engine, with Detection-specific contribution projection owned by T14a/T14b/T14c. `contains` predicates stay Predicate/Detection adapter. | Capture engine; D-006 contribution/reducer state; T14a URL/HTTP and T14c browser Strategy adapters | Delete the three regex/capture loops and mutable direct-map writes during D-006/D-007 activation; emit ordered DetectionContribution values instead. |

Ambiguity: current extraction capture fallback accepts named `value`, then any named group, then group 1, then group 0, while the DSL PRD says unnamed groups are not contract. The Capture owner must preserve only an explicitly approved schema-v3 rule; this inventory does not reopen D-005 but flags current code/docs disagreement.

### H. Acceptance / validation

`acceptWhen` can occur at phase step and Strategy level (`strategy.schema.json:5-45,97-112`; `Acceptance`, `documents/strategy.rs:7-17`). Plans clone it. Discovery and Detail duplicate merge/dispatch logic in `runtime/posting_discovery/acceptance.rs:3-174` and `posting_detail/acceptance.rs:3-120`.

| Authored key | Current semantics / consumers | Canonical owner | Direct prerequisites | Same-slice deletion |
|---|---|---|---|---|
| `requiredFields` | Discovery checks candidate normalized fields including `postingMeta.<key>`; Detail only accepts `descriptionText`; duplicate step/strategy union | **Acceptance Primitive family slice**, `primitives/acceptance/required_fields.rs`; phase adapters supply typed output field access. | T12a Discovery occurrence and T12b Detail patch/reducer contracts; T11c values indirectly | Delete both required-field helper/dispatch paths and raw Acceptance plans. |
| `minDescriptionLength` | Both phases implement separate character-count checks/codes | `primitives/acceptance/min_description_length.rs` | Typed phase outputs | Delete duplicate checks/tests after shared owner plus phase projection coverage. |
| `minResults` | Discovery implements; Detail silently ignores it even though shared schema/Serde admits it in Detail | `primitives/acceptance/min_results.rs`, with compiler context validation. Either provide real Detail semantics if meaningful or reject/remove Detail placement from schema; do not silently ignore. | T12a Discovery collection result; compiler placement context | Delete Discovery helper and silent Detail omission; add schema/compiler placement parity. |
| `maxErrorRatio` | Both phase acceptance modules emit `acceptance_max_error_ratio_unsupported` and reject every authored use; no executable result model supports it | **Admitted but unimplemented.** The Acceptance family slice must either implement it against a typed, truthful error denominator produced by the final phase result/reducer or remove the authored key from schema and Serde in the same hard cut. A permanent rejection registration violates D-005. Recommended based on current evidence: remove initial schema-v3 admission; no accepted #166 contract supplies denominator semantics. | If retained, T12b typed attempt/reducer evidence and T9 report; otherwise schema-v3 activation | Delete both unsupported stubs and tests; either add real key file or remove schema/Serde key. |

“Validation” beyond these keys: required Discovery title/company/url and Detail requested-field/output validation are phase output contracts, not separately discriminated authored Primitives. Source Config JSON Schema is its own compiler/Detection validator responsibility (D-006/D-011), not a Primitive registry variant. URL validity is mentioned in old docs but has no admitted authored key today.

### I. Detection-only authored strategy/predicate shapes

Current Detection is not yet the common schema-v3 Strategy algebra. Nevertheless these are executable authored DSL constructs and must not disappear from completeness accounting during convergence:

| Authored shape/key | Current path | Final classification / owner | Direct prerequisites and deletion |
|---|---|---|---|
| `inputUrlPatterns[] {pattern,captures}` (ordered first match) | schema `source-profile.schema.json:79-91`; docs `source_profile/documents.rs:59-65`; runtime `detection/mod.rs:204+` | T14a URL Detection Strategy adapter consumes canonical Capture regex + Template as needed; its Strategy type/registration is a Detection strategy, not a second regex implementation. | D-006 contributions/reconciled state. Delete `match_input_url_patterns`, mutable maps, and old evaluator in combined activation. |
| `httpChecks[]` with `expectStatus`, `contains`, `regex` | schema `:105-119`; docs `:76-91`; runtime `detection/http.rs` | T14a HTTP Detection adapter consumes canonical HTTP acquisition, Predicate (`equal status`, `contains`, regex) and Capture; these fields are currently a bundled legacy Strategy shape, not four generic implementations. | T10/shared HTTP if selected by final architecture, T9/T14 budgets, D-006 state. Delete old Detection HTTP client/evaluator at activation. |
| `browserProbes[]` with `htmlContains`, `htmlRegex`, waits/interactions | schema `:121-139`; docs `:93-111`; runtime `detection/browser.rs` | T14c Browser Detection Strategy adapter consumes canonical Browser action, Predicate, Capture, Template and D-007 Browser Acquisition. | D-007 foundation + T14b reconciled state. Delete raw probe conversion/evaluator and old seam at combined activation. |
| detection `sourceConfig`, `keyCandidates`, `nameCandidates`, `recommendedAccessPathKey`, `evidence` | proposal contributions/templates, not discriminated execution variants | T14b owns contribution reduction/proposal construction; Template owner handles strings. Evidence `kind=url/http/html/browser` is provenance metadata, not Primitive registration. | Delete duplicate proposal builders/mutable aggregation under D-006. |

The global gate must enumerate the final schema-v3 Detection Strategy discriminators after T14a/T14c land, but it must not fabricate registrations for today’s target-only common Strategy representation.

## 4. Schema/Serde/implementation mismatch register

1. **Fetch timeout requiredness:** schema requires `timeoutMs` for HTTP/browser (`fetch.schema.json:14,72`), Serde makes both optional (`documents/fetch.rs:18-26`); compiler later requires positive.
2. **Retry ghost:** Serde admits `fetch.http.retry.maxAttempts` and the plan compiles it (`documents/fetch.rs:20-21,50-55`; `execution_plan/capabilities.rs:21-27,204-218`), but fetch schema has no `retry`. D-012 requires deletion from initial #166 plans/results/schemas. Delete, do not register.
3. **Prohibited browser Serde variants:** five script/login/CAPTCHA variants are direct-Serde-admitted but schema-invalid and compile-rejected. Delete them (Section A).
4. **Browser bounds requiredness:** schema requires wait `timeoutMs` and interaction `selector/maxCount`; Serde makes wait timeout/selector and interaction maxCount optional. Detection duplicates the optional model.
5. **Pagination bounds requiredness:** schema requires `limits` on all four variants and `maxRequests` within limits (`pagination.schema.json:18-23,29,44,59,71`); Serde makes `limits` optional and all limit fields optional (`documents/pagination.rs:14-96`); compiler accepts any one stop rule, including maxDepth alone.
6. **Transform aliases:** schema and Serde admit five camelCase aliases in addition to canonical snake_case; clean schema-v3 should remove alias authorship instead of treating aliases as implementation variants.
7. **Split option names:** probable schema `trimParts`/`dropEmpty` versus Serde `trim_parts`/`drop_empty` mismatch; confirm with direct Serde test.
8. **Const domain:** schema allows string/number/boolean; Serde accepts any JSON value.
9. **`maxErrorRatio`:** schema/Serde admitted in both phases, runtime only has rejection stubs.
10. **`minResults` in Detail:** schema/Serde admitted but Detail runtime ignores it.
11. **Parse `text`:** schema/Serde admitted, both runtimes reject late. T11a’s accepted owned compiler rejection must be reviewed against D-005’s no-future-stub rule; it is at least explicit rather than silently ignored.
12. **Select `sitemap_urls`:** schema permits it anywhere a Select is referenced; current compiler broad compatibility admits XML placement, while meaningful runtime behavior is Discovery sitemap pagination only. T11b owns placement tightening.
13. **Field Expression context:** `posting_meta` is schema-admitted in Discovery but not executable there; capture references can chain accidentally; item/document expressions can survive into invalid Detail capture placement. T11c compiler contexts own hard rejection.
14. **Capture semantics:** implementation falls back to unnamed regex groups despite docs requiring named captures.
15. **Detection interaction duplication:** Detection defines a second interaction schema/Serde enum instead of referencing the canonical authored interaction document (only waits are shared).
16. **No registry/typed compiled parity:** copied authored documents mean successful Serde is often the only “registration”; runtime switches can omit/ignore variants without a compile-time/global failure.

## 5. Duplicate helpers/tests and family migration evidence

High-value duplicate implementation paths:

- Parse/Select: both `runtime/posting_discovery/document.rs` and `runtime/posting_detail/document.rs`; sitemap selector adds a third path.
- Value/Cardinality/Combine: both `runtime/posting_*/extract/fields.rs` plus phase `values.rs` and shared-yet-unregistered `runtime/transform.rs`.
- Filter: Discovery `extract.rs` versus Detail `strategy.rs`.
- Capture: both phase `extract/captures.rs`, plus three Detection regex/capture loops.
- Acceptance: both phase `acceptance.rs` duplicate precedence/field/length helpers.
- Fetch: both phase `fetch.rs` plus Detection HTTP and Detection browser request conversion.
- Browser waits/actions: `execution_plan/capabilities.rs`, `runtime/browser.rs`, and `source_profile/detection/browser.rs` all switch over them; Detection has a duplicate authored enum.
- Pagination: one monolithic four-variant runtime file plus hidden total/cursor extraction in Discovery strategy/document code.
- Template: shared parser exists, but compiler, phase and Detection contexts/wrappers duplicate validation/projection.

Tests that must be migrated, not merely deleted: `tests/{posting_discovery_runtime,posting_detail_runtime}.rs` and their submodules; `tests/posting_discovery_runtime/{pagination,transforms_and_combine,fallback_acceptance,document_types_and_browser,core,post_request_bodies,template_validation,failure_diagnostics,cancellation}.rs`; `tests/{compiler_semantic_validation,compiler_security_boundedness,schema_validation,source_profile_detection,source_live_check,greenhouse_profile_dsl,workday_profile_dsl,successfactors_profile_dsl}.rs`; in-module `runtime/transform.rs` tests; Search Run and posting service tests. Duplicate implementation-detail tests should be removed only after equivalent external compile-plus-phase tests use canonical owners. Built-in profile regressions prove generic cross-family composition, not registry completeness by themselves.

## 6. Proposed dependency edges based on consumed interfaces

This is ownership input, not approval of final ticket numbers:

```text
T9 budget/report foundation ───────────────┐
T10 shared HTTP transport/decoder ────────┼→ Fetch/http Primitive
D-007 Browser Acquisition foundation ─────┴→ Fetch/browser + browser-action Primitives

T11a Parse → T11b Select
T11b typed selected values → Cardinality + Transform
Cardinality + Transform + T11b → T11c Value
T11c Value → Predicate + Capture
T12a/T12b typed phase outputs + Predicate/Capture → Acceptance
Fetch + T9 + T11b Select → Pagination

Template syntax is an early shared prerequisite for Fetch, Value, and Detection adapters.
T14a consumes HTTP/Template/Predicate/Capture; T14c consumes Browser actions/Template/Predicate/Capture.
No consumer waits for the global gate unless it consumes all families.
All real family owners → implementation-free global D-005 convergence gate → #166 completion.
```

Acceptance may be split by keys if `maxErrorRatio` is removed separately, but it must still have one family owner. Pagination must not be hidden in T14a; Fetch must not be mistaken for T10 transport; browser authored actions must not be hidden in D-007 acquisition infrastructure; Predicate/Capture/Transform/Cardinality/Acceptance must not be silently assigned to T11c.

## 7. Global completeness gate: required evidence

The final gate adds no capability. It should fail the build/test when any admitted variant has zero/multiple registrations or any registry/dispatch file contains implementation behavior.

1. **Schema extraction:** resolve the real modular schemas and extract every discriminated `const`/`enum` and executable untagged union at the exact JSON Pointer, including aliases and Detection strategy definitions. Do not regex only the three headline enums.
2. **Serde exhaustiveness:** each authored document enum exposes an exhaustive descriptor through a match or generated static list; untagged `ListFieldExpression`, CaptureRule, Acceptance keyed variants and Match are included by explicit family descriptors. Synthetic missing/duplicate descriptor sets prove validator failure without mutating production registration.
3. **Compiled registrations:** compare `(family, authored_key, allowed_contexts)` against schema and Serde. Rejected/removed keys cannot masquerade as executable registration. A context-unavailable key such as `text` or `sitemap_urls` needs an explicit, reviewed contract; future placeholders are forbidden.
4. **Exactly-one owner:** registration records the canonical source file, but deterministic filesystem/call-graph searches independently prove behavior resides only in `profile_dsl/primitives/<family>/<dsl_type>.rs`; metadata is not self-proving ownership.
5. **No behavior in dispatch:** `primitives/mod.rs`, family `mod.rs`, and registry modules may select/delegate only. Compiler validation, bounds/security, regex/parser/library integration, execution and Primitive diagnostics must be in the key owner.
6. **No old duplicates:** family slices carry reviewed negative searches for old phase switches/helpers/DTOs/fakes/aliases and named deletion targets above. The gate checks known residue but does not inherit known deletion work.
7. **Consumer coverage:** external tests compile complete Source/Profile documents and exercise the real typed Discovery/Detail/Detection operations with deterministic HTTP/browser adapters. Greenhouse, Workday and SuccessFactors remain generic acceptance profiles.
8. **Global set expected after current admission cleanup:** Fetch `{http,browser}`; HTTP methods `{GET,POST}`; body `{json,text,form}`; waits `{selector,network_idle}`; interactions `{click_if_visible,click_until_gone}`; Pagination `{page,offset_limit,cursor,sitemap}` plus parameter location `{query,json_body}`; Parse `{json,xml,html,text-context-contract}`; Select six; Value twelve (plus `first_non_empty` only when actually landed); Cardinality four; Transform ten canonical keys; Predicate `{non_empty,regex,equal}`; Capture regex; Acceptance retained keys; Template string; and final concrete Detection strategy discriminators. Removed aliases/ghosts are proved absent.

## 8. Resolved assumptions and residual risks

- D-001–D-013 were not reopened. In particular Retry is classified as deletion under D-012, browser ownership respects D-007, Detection state respects D-006, and the gate follows D-005.
- “One canonical file per DSL type” does not require phase output reducers, Source Config validation, Strategy Policy, or metadata enums to become Primitives. Those remain their accepted owners.
- The exact target ticket labels are not yet approved. The family slices above are concrete responsibility owners/files and prerequisite seams for restructuring; they are not empty placeholder tickets.
- Ambiguities requiring implementation-readiness resolution, not architecture reopening: whether parameter location is independently registered; whether `text` remains an authored compile-time unavailable capability or is removed; whether `maxErrorRatio` is removed (recommended) or obtains a truthful denominator; direct Serde naming for split options; capture unnamed-group compatibility; and whether Template syntax is a separately registered family or an explicitly enumerated cross-family authored shape. The global gate must encode whichever final authored schema actually lands.
- Repository had unrelated pre-existing staged files. They were not touched, unstaged, or attributed to this work.

## 9. Validation performed

- Read Phase-1 decisions handoff and the full normative D-001–D-013 contract decision document.
- Read all modular DSL schemas and every relevant Serde document, compiled plan, compiler dispatch, phase runtime, Detection runtime, shared template/transform implementation, focused lean T8/T10/T11a/T11b/T11c ownership contracts, PRD Primitive rules, tests/fixtures/built-ins and product callers located by repository searches.
- Programmatically enumerated every `const` and `enum` in the seven modular capability schemas; manually added keyed/untagged executable authored shapes and inline Detection constructs.
- Verified current git baseline and staged-file list; wrote only this `/tmp` artifact.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced only the requested read-only D-005 variant/owner inventory at /tmp/job-radar-166-phase2/primitive-inventory.md; no project or GitHub files were modified and accepted D-001–D-013 were not reopened."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Inventory enumerates schema/Serde variants and mismatches, plans/dispatch/runtime paths, productive consumers, duplicate helpers/tests, concrete canonical owners/files, direct prerequisites, same-slice deletions, and an independent global completeness proof."
    }
  ],
  "changedFiles": [
    "/tmp/job-radar-166-phase2/primitive-inventory.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "repository find/grep/read pass over handoff decisions, schemas, Serde documents, compiler, execution plans, runtime, Detection, tests, fixtures, built-ins and docs",
      "result": "passed",
      "summary": "Located and inspected all admitted Primitive families and current consumers/duplicates."
    },
    {
      "command": "python3 schema const/enum inventory for fetch, pagination, parse, select, extract, transform and strategy schemas",
      "result": "passed",
      "summary": "Enumerated all tagged schema keys; untagged/keyed and Detection constructs were then reviewed manually."
    },
    {
      "command": "git status --short && git diff --cached --name-only",
      "result": "passed",
      "summary": "Confirmed unrelated pre-existing staged files; this task did not change repository state."
    }
  ],
  "validationOutput": [
    "No current Primitive registry exists; Fetch/Pagination compile to typed enums while Parse/Select and remaining authored constructs are copied into plans.",
    "Found explicit schema/Serde/runtime gaps including retry ghost, five prohibited Serde-only browser interactions, optional-vs-required bounds, const domain mismatch, transform aliases/field naming, maxErrorRatio stub, Detail minResults omission, text late rejection, sitemap placement, and duplicate Detection interactions.",
    "Output file exists at the authoritative /tmp path."
  ],
  "residualRisks": [
    "Exact final target ticket labels/boundaries remain subject to restructuring review; concrete family responsibilities and canonical files are supplied.",
    "Parameter-location registration, text admission, maxErrorRatio removal/implementation, split Serde naming, capture unnamed-group semantics, and Template family classification need readiness resolution against the final schema-v3 authored contract.",
    "Repository contains unrelated pre-existing staged files, so a literal no-staged-files condition is not true even though this task staged nothing."
  ],
  "noStagedFiles": false,
  "diffSummary": "Added one external /tmp analysis artifact; repository diff was not changed by this task.",
  "reviewFindings": [
    "blocker: D-005 completion is impossible with the current tree because no global registry or registration parity mechanism exists and several admitted variants are ignored, late-rejected, or duplicated.",
    "blocker: maxErrorRatio is authored but has only runtime rejection stubs; it must be implemented truthfully or removed from admission.",
    "blocker: Serde directly admits retry and prohibited browser interactions that schema does not; D-012/security require deletion rather than registrations.",
    "review required: independent reviewer must verify family boundaries and the ambiguous classifications before target ticket/DAG approval."
  ],
  "manualNotes": "No tests were added or run because this is a review/inventory-only task. Existing staged files predate this work and were left untouched."
}
```
