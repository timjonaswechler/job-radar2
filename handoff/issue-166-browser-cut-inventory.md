# Issue #166 — Browser seam caller/deletion inventory (D-006/D-007)

Status: read-only code inventory; generated 2026-07-18. Project/source files were not changed.

> **Adversarial lifecycle clarification:** D-007 requires typed `BrowserInfrastructureFailure` when bounded teardown cannot establish process termination/reap and its cleanup invariant. A bounded filesystem `remove_dir_all` residue that is safely quarantined may remain private evidence and does not automatically replace an otherwise successful primary result. Final B02 readiness must classify these cases explicitly rather than preserving today’s ignored cleanup or treating every filesystem residue as a domain failure.

## Normative constraints applied (not reopened)

The complete Phase-1 handoff and contract decisions were read. This inventory applies accepted D-006/D-007:

- A single phase-neutral Browser Acquisition module owns process lifecycle, navigation/actions, cancellation-aware waits, content acquisition, bounded teardown, and infrastructure-failure classification.
- It has one managed production adapter and one scripted deterministic adapter.
- Detection, Discovery, and Detail keep typed phase adapters, phase output semantics, and phase-owned allowances.
- Detection uses invocation → profile → Strategy child scopes with atomic checks at all scopes; its Browser Strategies emit native ordered `DetectionContribution`s into `ReconciledDetectionState`. The lossy current browser aggregate is not translated.
- One Browser/Detection cross-phase activation migrates **all** productive callers and deterministic tests and deletes `ProfileBrowserClient`, `render*`, old implementations, exports, aliases, wrappers, and duplicate fakes in the same slice. T14c does not own that cut; T14d is guard-only.
- Cancellation returns only after bounded teardown. Cleanup failure is typed `BrowserInfrastructureFailure`; private teardown residue is not projected into phase Diagnostics or Cancellation.

Primary decision evidence: `handoff/issue-166-phase-1-decisions-handoff.md:119-135,273-285`; `handoff/issue-166-contract-decisions.md:196-279,281-302,603-604`.

## Actual current call graph

Only three leaf runtime sites invoke the seam:

```text
Detection command
  -> detect_source_proposal_with_clients
  -> evaluate_profile
  -> evaluate_browser_probes
  -> ProfileBrowserClient::render

Discovery callers (Source Live Check / Search Run / direct tests)
  -> execute_posting_discovery_with_clients[_and_context]
  -> execute_strategy / pagination
  -> fetch_browser_strategy_document
  -> ProfileBrowserClient::render_with_context

Detail callers (Source Live Check / posting UI / direct tests)
  -> execute_posting_detail_with_clients[_and_context]
  -> execute_strategy
  -> fetch_browser_strategy_document
  -> ProfileBrowserClient::render_with_context

ManagedProfileBrowserClient::{render,render_with_context}
  -> browser_runtime::status_for_runtime_dir
  -> BrowserRuntimeRenderRequest conversion
  -> browser_runtime::render_page_html_with_actions_and_context
  -> chromiumoxide Browser::launch/new_page/goto/waits/interactions/content/close
```

Repository search found no other relevant `.render` call; `src-tauri/src/bin/agent_debug.rs` has an unrelated UI event renderer.

## Seam and implementation inventory

| Current path/symbol | Current responsibility / behavior | Final owner / adapter | Exact migration or deletion target | Evidence / uncertainty |
|---|---|---|---|---|
| `src-tauri/src/profile_dsl/runtime/browser.rs:9-15` `BoxedProfileBrowserFuture` | Old trait future alias | Final Browser Acquisition interface | Delete alias with old trait; use final acquisition future/interface directly | No callers outside old trait implementations. |
| `browser.rs:18-28` `ProfileBrowserFetchRequest/Response` | Phase-erasing URL + timeout + plan waits/interactions → body `String` | Phase-neutral acquisition input/result internally; phase adapters own their typed inputs/projections | Delete old DTOs, including names and exports; phase adapters map directly to final acquisition contract | Current response cannot report usage, final URL, teardown, or typed Cancellation. Exact final type names are not yet present. |
| `browser.rs:31-53` `ProfileBrowserFetchError{,Kind}` | Seven acquisition errors, including Cancellation as an error kind | Browser Acquisition terminal classification; phase adapters project safe failures; typed Cancellation remains control flow | Delete old errors and both Discovery/Detail diagnostic matchers; replace with exhaustive final adapter mappings | `RuntimeUnavailable` also represents launch/page/close failures, so current classification is lossy. |
| `browser.rs:56-77` `ProfileBrowserClient::{render,render_with_context}` | Shared old seam. Default context method only polls before/after non-cancellable `render` | Final phase-neutral Browser Acquisition seam | Delete trait and both methods; no forwarding compatibility trait | Default method cannot interrupt active scripted/other `render`; only managed overrides it. |
| `browser.rs:81-92` `UnavailableProfileBrowserClient` | Injected by browser-incapable convenience helpers | No old-seam replacement object | Delete. Migrate phase operations so browser capability is supplied through the final adapter when required; do not retain an alias/fake | Exact optional/unavailable representation is a design detail for final phase adapters, not decided here. |
| `browser.rs:95-211` `ManagedProfileBrowserClient` | Checks installed runtime, converts plan wait/action types, calls process control, maps body/error | Managed production adapter of Browser Acquisition | Replace/delete this old implementation in activation; production construction sites move directly to final managed acquisition + typed phase adapter | Runtime status lookup occurs on every acquisition. No direct unit test of conversion/mapping. |
| `browser.rs:213-246` cancellation/error mapping helpers | Old-seam translations | Final acquisition classification + phase projections | Delete with `browser.rs` | Mapping currently folds close/launch/page errors into `RuntimeUnavailable`. |
| `src-tauri/src/browser_runtime/types.rs:81-141` `BrowserRuntimeRender*`, `BrowserRuntimeWait/Interaction` | Second acquisition DTO/error algebra under Profile seam | Browser Acquisition module internal/final types | Replace old `Render` types rather than retain aliases; keep install/status types in this file/module only if still appropriate | Duplicate request/error models are exactly the layering D-007 removes. |
| `browser_runtime/control.rs:31-54` `render_page_html_with_actions_and_context` | Session dir + render + best-effort cleanup | Browser Acquisition managed adapter/lifecycle | Move/replace through final acquisition interface; delete old function name/export | Crate-private, sole caller is `ManagedProfileBrowserClient`. |
| `control.rs:62-93` cleanup/result helpers | 3 `remove_dir_all` tries, 50ms sleeps; **discard cleanup failure** | Bounded teardown owned by acquisition | Delete best-effort/success-preserving semantics. Cleanup failure must become typed infrastructure failure after bounded close/terminate/reap/finalization | Current tests explicitly assert success survives cleanup failure (`browser_runtime/tests.rs:261-284`), which must be replaced, not preserved. |
| `control.rs:95-269` waits/interactions | Cooperative cancellation, selector polling, clicks and waits | Shared Browser Acquisition | Migrate behavior into final module; preserve interaction index/error parity and absent optional-click semantics, add truthful reservations | No browser action/wait usage is reported or reserved today. `NetworkIdle` is a sleep (plus optional selector), not actual network-idle observation. |
| `control.rs:272-407` smoke/render/launch | Launches one Chromium process/session per request; page timeout wraps page work; then unbounded `browser.close()` and handler join | Managed production adapter + bounded lifecycle | Replace old launch/render/close path; operational smoke must exercise final managed adapter lifecycle | D-007 mismatch: teardown is outside `timeout`, close/join are unbounded, no forced terminate/reap, no reserved teardown deadline; `page.content()` has unbounded temporary allocation. |
| `browser_runtime/mod.rs:35-72` active-session set/drop guard | Protects live session dirs from status cleanup | Final managed lifecycle/private session bookkeeping | Retain only through final Browser Acquisition ownership or replace; remove old control coupling | In-process path set is not process-reap proof. |
| `browser_runtime/status.rs:5-45` `check_runtime` → `control::smoke_test` | UI operational health check | Operational caller of final managed adapter | Migrate smoke to final managed lifecycle; preserve `BrowserRuntimeCheckResult` UI projection | Not a Detection/Discovery/Detail phase and owns no phase budget. |
| `browser_runtime/status.rs:90,223-244` stale temp cleanup | Status checks remove stale install/session dirs except active set | Managed runtime installation/lifecycle boundary | Reconcile with final private teardown/session ownership; keep status behavior only if it cannot race final lifecycle | Current ignored cleanup errors and process-local activity tracking are risks. |
| `browser_runtime/{archive,download,install,manifest,spec,status}.rs` | Runtime distribution/install/status, not acquisition seam | Managed runtime facility used by final production adapter | Retain generally; migrate only acquisition/lifecycle coupling. Do **not** delete browser installation UI/API as seam residue | `install_runtime` cleanup is separate install-workspace behavior. |
| `Cargo.toml:37,39,49` | `chromiumoxide`, futures, Tokio | Managed adapter dependencies | Retain or replace based on final implementation; no dependency change implied by inventory | No external API research was needed; behavior is locally explicit. |

### Lifecycle blockers exposed by the verified implementation

1. The current request timeout does not include teardown (`control.rs:306-358` vs. close/join at `360-370`).
2. Cancellation exits page work but then awaits unbounded close and handler completion.
3. There is no forced process termination/reap path.
4. Session-directory cleanup failure is deliberately ignored even on success.
5. Rendered HTML has no pre-exposure 2 MiB/4 MiB/16 MiB accounting; temporary `page.content()` allocation is unconstrained.
6. Actions, waits, navigation, bytes, and complete duration usage are not reported to any phase ledger.
7. No consolidated scripted adapter exists. Current fakes only return a body/error by URL (except a one-off cancellation fake), so they do not prove shared lifecycle/action/accounting semantics.

These are required foundation work, not uncertainties that may be silently preserved.

## Caller inventory by required group

“Activation” below always means the single D-007 Browser/Detection cross-phase hard cut after final Browser Acquisition, all three phase adapters, D-006 URL/HTTP/Browser contribution support, and required budget foundations exist.

### Detection

| Current path/symbol | Current path and output projection | Final phase adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `source_profile/detection/browser.rs:22-78` `evaluate_browser_probes` | Sequentially templates probes, calls `render`, regex/contains checks body, mutates shared captures map, appends browser evidence; first failure returns `false` | Detection Browser Strategy adapter → Browser Acquisition → native ordered `DetectionContribution`s and immutable reconciled state | Detection invocation parent → profile child → Strategy child; accepted ceilings include nav/actions/waits/duration/rendered bytes at all scopes | `tests/source_profile_detection.rs:801-1200`: evidence/captures/request, Source Config URL, wait/error mapping, non-match, bounded timeout/waits/interactions, unavailable executor | Migrate function to final Detection Strategy contract; delete mutable capture mutation, old request/error projection, and old function if superseded. Activation owner is cross-phase cut, not T14c | Current Detection has no `RuntimeExecutionContext`, typed Cancellation, operation/profile ledger, cumulative ceilings, rendered-byte bound, or conflict-safe capture contribution. |
| `source_profile/detection/mod.rs:112-132` `detect_source_proposal_with_clients/internal` | Accepts generic old browser or `Option<dyn old trait>` | Public typed Detection operation with Detection acquisition adapter | Detection operation owns parent across registry profiles | Same external detection tests; API test below | Delete generic old-trait signatures/options; no wrapper. Retain only final typed public operation | Exact final operation/adapter names pending restructuring. |
| `detection/mod.rs:205-309` `evaluate_profile` | URL → HTTP → builds provisional Source Config → Browser → proposal; browser receives mutable captures and aggregate config | D-006 reconciled Detection pipeline; dependent Browser reads immutable state | Profile/Strategy children under invocation parent | Existing tests prove current ordering/output only | Delete old evaluator/provisional aggregate path in activation; do not translate aggregate | The current pre-browser `build_source_config` is lossy and conflicts with D-006; parity is observable proposal behavior, not internal aggregate preservation. |
| `app/commands.rs:489-527` `detect_source_proposal_from_url[_with_clients]` | Constructs managed old client and returns `SourceProposalDetectionResult` to Source-create UI | Application command injects final managed acquisition into final Detection adapter | Detection operation, not command | `commands.rs:1080-1237` no-browser and browser proposal tests; `tests/source_profile_detection.rs` | Migrate command/helper directly; delete generic helper’s old bound and `ManagedProfileBrowserClient` construction | Command remains the productive entry point; UI contract should remain unless final Detection result intentionally changes elsewhere. |
| `src/lib.rs:66-72` Detection exports | Public integration seam used by external tests | Final Detection operation exports | Detection | External detection integration suite | Remove old-client export variant/signature and re-export final operation only | No compatibility export allowed. |
| `src/lib/api/sources.ts:356-358` and `features/sources/create/source/use-source-create.ts:229-235` | UI invokes command and consumes matched/ambiguous/unsupported proposal | Unchanged UI → command → final Detection adapter | None in UI | Existing frontend Source-create behavior; backend command tests | No Browser type deletion in TS; migrate backend beneath same command. Update UI only if final result shape changes | Browser-specific end-to-end UI test is absent. |

Detection current Diagnostics map all Browser failures at `detection/browser.rs:286-336`; this mapper is deleted/replaced by the final Detection adapter. Safe codes/path parity is evidence to retain, but Cancellation must become typed control rather than `browser_probe_cancelled` Diagnostic.

### Discovery

| Current path/symbol | Current path and output projection | Final phase adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `profile_dsl/runtime/posting_discovery/fetch.rs:288-342` `fetch_browser_strategy_document` | Renders URL, calls `render_with_context`, projects body to `PostingDiscoveryFetchResponse`; error → runtime Diagnostic | Discovery Browser acquisition adapter | Discovery/T9 phase allowance, Strategy policy scope; caller tightening where applicable | `tests/posting_discovery_runtime/document_types_and_browser.rs:88-184` rendered HTML/request and wait Diagnostic | Replace generic `B: ProfileBrowserClient` throughout `fetch.rs`, `strategy.rs`, `pagination.rs`, `posting_discovery.rs`; delete helper’s old DTO mapping | Current Browser fetch itself has only authored timeout; pagination request cap is not a complete browser ledger. |
| `posting_discovery/support.rs:69-119` `push_browser_fetch_diagnostic` | Old error → Discovery Diagnostic; Cancellation encoded as diagnostic | Discovery adapter terminal projection | Discovery | Error/cancellation tests | Delete old matcher; final adapter maps acquisition infrastructure terminals while typed Cancellation stays outside ordinary outcome under D-003/D-010 | Final outcome algebra lands in other accepted slices. |
| `posting_discovery.rs:217-287` `execute_posting_discovery_with_clients[_and_context]` | Generic old browser threaded through Strategy runtime; returns candidates + Diagnostics | Typed Discovery operation taking final Discovery adapter/dependency | Discovery phase allowance | External integration tests broadly cover Discovery and Browser cases | Delete `_with_clients` old signature/name if superseded; migrate every caller directly, no forwarding helper | Current public result and policy model are also scheduled for later accepted migration; Browser activation must target the final interface available then. |
| `posting_discovery.rs:205-215` `execute_*`/`with_fetcher` | Browser-incapable helpers inject `UnavailableProfileBrowserClient` | Final typed operation with explicit production/deterministic acquisition dependency as required | Discovery | Many HTTP-only tests | Delete unavailable injection and old convenience wrapper if it would forward old/new seams; callers migrate to final phase operation | Whether a browser-free phase adapter is represented by enum/capability is not decided. |
| `profile_dsl/runtime/cancellation.rs:18-70` current `PostingDiscoveryExecutionBudget` | Only max pagination requests **per Strategy**; context also carries Cancellation | Final T9 Discovery budget report/allowance, passed to Discovery adapter | Discovery; Source Live Check currently tightens to one request/Strategy | pagination integration tests and cancellation suite | Browser migration must not misrepresent this as complete D-007 accounting | Current browser navigation/actions/bytes/duration do not debit it. |

### Detail

| Current path/symbol | Current path and output projection | Final phase adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `profile_dsl/runtime/posting_detail/fetch.rs:186-235` `fetch_browser_strategy_document` | Renders Source/posting/capture template, calls context-aware render, body → `PostingDetailFetchResponse` | Detail Browser acquisition adapter | Detail/T9 candidate-scoped phase allowance | `tests/posting_detail_runtime.rs:754-840` rendered HTML/request and interaction Diagnostic | Replace generic old client through Detail runtime; delete old request/body projection | Current Detail has no caller budget/report beyond authored timeout. |
| `posting_detail/support.rs:98-151` `push_browser_fetch_diagnostic` | Old error → Detail Diagnostic, including Cancellation diagnostic | Detail adapter terminal projection | Detail | Detail error and cancellation tests | Delete old matcher; typed Cancellation remains outside phase outcome | Same codes can be retained as safe observable parity where valid. |
| `posting_detail.rs:214-329` `execute_posting_detail_with_clients[_and_context]` | Generic old browser; first accepted description or Diagnostics | Typed Detail operation taking final Detail adapter | Detail | External Detail integration tests | Delete/migrate old signatures and all callers, no wrapper | Final requested-fields/output algebra comes from accepted T12b/T15 work; activation must use whichever final interface has landed. |
| `posting_detail.rs:198-212` `execute_*`/`with_fetcher` | Injects unavailable old client | Final typed phase dependency | Detail | HTTP-only tests | Delete old unavailable helper/injection | Same representational uncertainty as Discovery. |

### Source Live Check

| Current path/symbol | Current path and output projection | Final adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `checks/source_live/mod.rs:91-114,157-264` `check_source_with_clients` / `build_source_live_check_report` | Threads one old client into Discovery then optional first-candidate Detail; projects candidate/detail facts + Diagnostics into persisted `CheckReport` | Source Live Check orchestrates final Discovery and Detail adapters; it is not a fourth Browser phase adapter | Current Discovery tightening is `SOURCE_LIVE_CHECK_MAX_PAGINATION_REQUESTS_PER_STRATEGY = 1` (`:43,200-203`); Detail has no caller budget. Final allowances remain phase-owned and Source Live Check supplies typed tightening | `tests/source_live_check.rs` covers HTTP Discovery/Detail, pagination cap, reports and activation, but **no browser client/browser plan** | Migrate `check_source_with_clients`, report builder, and command callers to final phase adapters; delete old generic bound. Keep report semantics and persistence | Browser-specific Source Live Check parity is missing and must be added with final scripted adapter. Accepted target tightening must be asserted at invocation scope, not inferred from current per-Strategy cap. |
| `checks/source_live/mod.rs:61-89` browser-incapable wrappers | Inject `UnavailableProfileBrowserClient` | Final phase operations | Source Live Check | HTTP tests | Delete old unavailable object/signature; avoid a forwarding compatibility wrapper | Representation pending. |
| `checks/source_live/activation.rs:35-164` check/activate/reactivate variants | Same report builder, then Source status transition | Same final Discovery/Detail adapters | Same phase budgets | `tests/source_live_check.rs:804-980` activation/reactivation (HTTP only) | Migrate all three `_with_clients` paths; delete old generic bounds/unavailable injections | Need scripted-browser pass/fail activation parity. |
| `app/commands.rs:424-478` three Source Live Check commands | Construct one managed old client and return `CheckReport` | Inject final managed acquisition and final phase adapters | Phase-owned | No direct browser command test | Replace construction/signatures in cross-phase activation | Synchronous command uses `block_on` internally; final lifecycle must remain safe under Tauri runtime. |
| `src/lib/api/sources.ts:337-347`; `source-live-check-section.tsx:85-95` | UI invokes check/activate/reactivate and renders report/status | UI unchanged over Tauri command | None | UI/model tests are not browser-specific | Backend migration; retain TS surface unless report contract changes elsewhere | Browser runtime unavailable presentation relies on Diagnostics. |

### Search Run

| Current path/symbol | Current path and output projection | Final adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `search/run/execution.rs:84-103` `DefaultSourceExecutor` | Per Source call creates Reqwest fetcher + managed old browser, invokes Discovery | Production Source executor injects final managed acquisition through Discovery adapter | Current Search Run passes Cancellation only; final Discovery allowance is child scope supplied by Candidate Resolution/Source Resolution under D-009/T9 | Search Run suites use fake `SourceExecutor`; no direct managed/browser `DefaultSourceExecutor` parity found | Replace `ManagedProfileBrowserClient` construction and generic helper bound; keep SourceExecutor deep seam only if retained by target architecture | Missing browser-capable Search Run integration test is a material gap. |
| `execution.rs:106-164` `execute_posting_discovery_for_source` | Cancellation context, Discovery candidates → `SourceCandidate`; Diagnostics determine execution failure | Final Discovery operation/projection | Discovery child report committed by owning Search Run/Resolution layer | Discovery cancellation behavior indirectly covered; no browser-specific test | Migrate helper directly; no old/new coexistence | Current control derives failure from Diagnostics, later accepted result algebra will change this independently. |
| `app/commands.rs:692-748` `schedule_search_request_run` | Passes runtime dir into `DefaultSourceExecutor` background task | Final production Source executor with managed acquisition | Search Run/Resolution parent supplies phase child allowance | Existing Search Run command/service tests; not browser-specific | Constructor moves to final adapter/dependency | No command API change required solely for Browser seam. |
| `search/smoke/cli.rs:54`; `app/commands.rs:701-712` | Dev smoke/command constructs DefaultSourceExecutor | Same final production path | Search Run | Network-dependent manual smoke only | Migrate through DefaultSourceExecutor; no separate adapter | Smoke is not deterministic parity evidence. |

### Posting / UI path

| Current path/symbol | Current path and output projection | Final adapter | Budget owner/scope | Current parity evidence | Activation / deletion target | Uncertainty |
|---|---|---|---|---|---|---|
| `search/posting/service.rs:126-150` `get_posting_detail[_with_clients]` | Lazy UI detail constructs managed old client; test seam generic | Final Detail adapter over managed/scripted acquisition | UI-triggered candidate-scoped Detail allowance | `search/posting/tests/detail_loading/browser.rs:4-75` verifies compiled browser Detail, persisted description, requested URL; other detail-loading tests inject unavailable client | Replace production construction and test helper bound; delete old client use | No explicit current Detail safety allowance; target must supply a typed phase allowance. |
| `service.rs:151-275` source fallback loop | Compiles each persisted source, calls Detail; accepted description persisted and returned as `JobPostingDetail` | Posting service remains phase caller of final Detail operation | Detail per attempted source/candidate; service owns source fallback policy, not acquisition lifecycle | Basic/fallback/diagnostic/browser service tests | Migrate direct call; preserve lazy loading, fallback, read marking, persistence/output parity | How final `SourceDetailOutcome` maps into this UI-only lazy path must be made explicit by the Detail/T15 owner. |
| `app/commands.rs:807-818` `get_posting_detail` | Passes app/runtime dirs to service | Service injects final managed adapter | Detail | Backend posting service tests | Constructor/signature migration | No browser type crosses Tauri. |
| `src/lib/api/job-postings.ts:89-91` → `postings-workspace-provider.tsx:158-165` → `load-posting-detail.ts:19-34` | UI calls command, updates list/detail | UI remains indirect Detail caller | None | `features/postings/tests/postings-ui-contract-tests.ts` proves loader behavior with fake API, not Browser | No Browser-specific TS deletion; backend hard cut must keep command output parity | Browser infrastructure errors surface through `descriptionState`; final safe projection must preserve useful behavior. |

### Commands and runtime-management UI

Productive browser commands are the Detection, three Source Live Check, Search Run, and posting-detail commands listed above. Separately:

| Current path/symbol | Role | Final target | Evidence / deletion |
|---|---|---|---|
| `app/commands.rs:311-375` runtime status/install/uninstall/check | Managed runtime administration, not `ProfileBrowserClient` caller; `check` reaches control smoke | Retain command/UI API; migrate smoke/lifecycle internals to final managed Browser Acquisition adapter | `browser_runtime/tests.rs` covers installation/status/session cleanup. Do not delete these commands as old-seam residue. |
| `app/state.rs:13,56` install lock; `app/paths.rs:64,81` runtime dir | Runtime facility state/path | Retain and inject into final managed adapter | No phase budget. |
| `src/lib/api/browser-runtime.ts`; `features/sources/runtime/browser-runtime-controller.ts`; `browser-runtime-card.tsx`; `sources-workspace-view.tsx` | Runtime administration UI | Retain | Not a phase acquisition caller. The health check must use final lifecycle underneath. |
| `src-tauri/src/lib.rs:139-143` runtime commands in Tauri handler | Exposes administration | Retain | Independent from old Rust seam exports. |

### Production adapter construction sites (complete)

`ManagedProfileBrowserClient::new` occurs only at:

1. `app/commands.rs:430` Source check;
2. `app/commands.rs:449` check-and-activate;
3. `app/commands.rs:468` check-and-reactivate;
4. `app/commands.rs:494` Detection;
5. `search/run/execution.rs:100` Search Run Discovery;
6. `search/posting/service.rs:134` posting/UI Detail.

All six migrate in the single cross-phase activation. Repository search found no other construction site.

## Scripted fakes and tests (complete)

There is **no current shared scripted Browser adapter**. Every implementation of `ProfileBrowserClient` is listed below.

| Fake / path | Used by | What it proves now | Final migration/deletion |
|---|---|---|---|
| `tests/source_profile_detection.rs:1219-1273` `FakeBrowser` | Detection tests at `:175-200,801-1200` | URL/request shape, body evidence/capture, error mapping, compile/bounds rejection | Rewrite scenarios against final scripted acquisition + Detection adapter, then delete fake/old DTO assertions. Add native contribution/conflict/scoped-ledger/Cancellation evidence. |
| `tests/posting_discovery_runtime.rs:80-137` `FakeBrowser` (submodules share it) | Browser tests in `document_types_and_browser.rs:88-184`; cancellation HTTP test uses empty fake | Request/body/error parity | Rewrite against shared scripted adapter + Discovery adapter; delete duplicate fake. |
| `tests/posting_discovery_runtime/cancellation.rs:163-211` `CancellationAwareBrowser` | `posting_discovery_browser_cancellation_is_distinct_from_runtime_failure` (`:56-98`) | Proves runtime calls `render_with_context` and active call can await cancellation | Replace with scripted adapter cancellation script/lifecycle assertion; delete fake and old-method panic. Must assert bounded teardown before typed Cancellation. |
| `tests/posting_detail_runtime.rs:993-1049` `FakeBrowser` | Detail browser/error/cancellation-context tests (`:754-840,908-940`) | Request/body/error parity; pre-cancel context | Rewrite against shared scripted adapter + Detail adapter; delete duplicate fake. Add active cancellation/teardown test (currently absent for Detail). |
| `src/search/posting/tests.rs:37-97` `FixtureProfileBrowserClient` | `detail_loading/browser.rs:4-75` | Posting service lazy Browser Detail and requested URL | Rewrite using shared scripted adapter; delete fixture. HTTP-only service tests should no longer inject `UnavailableProfileBrowserClient`. |
| `app/commands.rs:1239-1263` `StaticBrowserClient` | command Detection browser test `:1177-1237` | Command-to-Detection proposal projection | Use shared scripted adapter fixture through final command helper; delete static fake. |
| `UnavailableProfileBrowserClient` in `browser.rs:81-92` | Runtime convenience helpers; Source Live Check defaults; posting tests; command no-browser test | Deterministic “runtime unavailable” or merely browser-unused injection | Delete globally; browser-required unavailable behavior must be represented by final acquisition/adapter terminal, while browser-unused tests should not need a fake old seam. |
| `browser_runtime/tests.rs:261-332` control helper tests | Old process/control internals | Cancellation before session creation and current cleanup behavior | Replace success-on-cleanup-failure assertions with typed infrastructure-failure/cleanup-invariant tests; migrate cancellation test to final acquisition interface. |
| `browser_runtime/tests.rs:334-376` active/stale session test | Runtime status cleanup | Active session directory protection | Retain/adapt as private managed lifecycle test, plus process reap and bounded cleanup tests. |

Additional compile/schema tests (`tests/compiler_security_boundedness.rs:193-284`, `tests/schema_validation.rs:145+`) do not call the Browser seam. Retain their authored/compiled boundedness role, but D-007/D-002 target tests must add actual ledger reservation and usage rather than treating compiler rejection as runtime accounting proof.

## Exports, aliases, and wrapper surface to remove/migrate

1. `profile_dsl/runtime/mod.rs:7-14`: delete all old Browser re-exports and update phase-operation exports/signatures.
2. `src-tauri/src/lib.rs:48-60`: delete public `ManagedProfileBrowserClient`, `ProfileBrowserClient`, `ProfileBrowserFetch*`, `UnavailableProfileBrowserClient`; export only intended final phase operations/adapters. External integration tests must import final interfaces.
3. `checks/mod.rs:20-26` and `src/lib.rs:14-20`: `_with_clients` Source Live Check exports must migrate to final adapter-facing operations; do not leave forwarding aliases.
4. Discovery/Detail `_with_clients`, `_with_clients_and_context`, and `_with_fetcher` are old-dependency convenience surfaces. At activation, either their final phase API replaces them and old names are deleted, or callers move directly; no old-signature forwarding wrapper may remain.
5. `browser_runtime/mod.rs:19,29-32`: remove old render function/types from exports; preserve independent install/status exports.
6. Test imports in `tests/{source_profile_detection,posting_discovery_runtime,posting_detail_runtime}.rs`, `src/search/posting/tests.rs`, and command tests move to the final scripted/phase interfaces.

A residue guard should search at least:

```text
ProfileBrowserClient|ProfileBrowserFetch|ManagedProfileBrowserClient|UnavailableProfileBrowserClient
render_with_context|BoxedProfileBrowserFuture
BrowserRuntimeRender(Request|Error)|render_page_html_with_actions_and_context
StaticBrowserClient|FixtureProfileBrowserClient|CancellationAwareBrowser
```

The generic word `render` cannot be globally forbidden because unrelated UI/event rendering exists; guard the removed trait/module paths and exact Browser symbols.

## Required parity matrix for the activation cut

| Surface | Minimum acceptance evidence before old-path deletion |
|---|---|
| Shared acquisition / managed adapter | installed/unavailable; launch/navigation/wait/action/content classifications; success body; timeout includes teardown; active Cancellation triggers bounded teardown; graceful close; forced terminate/reap; cleanup failure → typed infrastructure failure; no later work after terminal; rendered-byte rejection/accounting; private residue not in public Diagnostics. |
| Shared scripted adapter | Ordered expected acquisitions; request/action/wait assertions; body and each typed failure; active cancellation; deterministic usage; detects missing/unexpected calls. It replaces every duplicate fake above. |
| Detection adapter | Existing request/evidence/proposal/error parity plus native ordered contributions before mutation, conflict origins, immutable state dependency, incremental validation before dependent work, invocation/profile/Strategy atomic ceilings, typed Cancellation. No aggregate translation. |
| Discovery adapter | Existing HTML extraction/request/error/cancellation parity plus complete phase allowance/report and typed outcomes; Browser work charged to Discovery scope. |
| Detail adapter | Existing description/request/error parity plus candidate-scoped allowance/report, requested output projection, active cancellation, typed outcomes. |
| Source Live Check | Scripted Browser Discovery pass/fail, optional Browser Detail pass/fail, persisted report and activate/reactivate behavior, correct phase tightening. Current suite lacks these. |
| Search Run | Deterministic Source/Search Run using Browser Discovery through scripted adapter, Cancellation, failure and output projection. Current Search Run tests bypass `DefaultSourceExecutor`; this is a gap. |
| Posting/UI | Existing `detail_loading/browser.rs` rewritten to scripted adapter; lazy fetch, source fallback, persistence, `descriptionState`; frontend loader/API contract remains passing. |
| Commands | Detection + Source check/activate/reactivate + Search Run construction + posting detail all instantiate the final managed path (or an injectable scripted test path) with no old constructors. |
| Runtime admin | Install/status/check UI remains functional; check smoke exercises final managed lifecycle and does not preserve ignored cleanup failure. |
| Deletion | Repository guard has zero old seam/implementation/fake/export hits except explicit migration-history documentation if allowed; no wrapper or dual productive route. |

## Cross-phase ownership and sequencing

- **Shared Browser Acquisition foundation owner:** final phase-neutral module, managed adapter, scripted adapter, bounded lifecycle/teardown, terminal classification. It may land non-productively through final interfaces.
- **Detection-specific foundation (current T14c responsibility):** Detection Browser Strategy compilation, native contributions, and accepted Detection ceilings/scopes.
- **Discovery/Detail phase owners:** thin typed adapters and their T9-compatible phase allowances/output projection.
- **Activation owner:** one explicit D-007 Browser/Detection cross-phase ticket/slice. It migrates all rows in this inventory and performs same-slice deletion. It must directly depend on shared acquisition + managed/scripted adapters, all three final phase adapters/budget interfaces, and complete D-006 URL/HTTP/Browser contribution/reducer/validation foundation.
- **T14d:** residue guard only; it cannot inherit any known caller, fake, export, or implementation deletion listed here.

## Residual uncertainties (do not reopen D-006/D-007)

1. Final Rust symbol/module names for Browser Acquisition and the three typed adapters are not yet assigned. The responsibilities and deletion boundaries are fixed.
2. The final representation for a phase plan with no Browser Strategy (no adapter argument vs. capability enum vs. another typed construction) is not decided. It cannot be `UnavailableProfileBrowserClient` or an old-seam wrapper.
3. Discovery/Detail final APIs may already be changed by T9/T12b/T15 before activation. Migration must target those final typed operations, not preserve current `_with_clients` names.
4. Current runtime installation/status UI is independent and should remain, but exact module placement of smoke/session bookkeeping under the final acquisition owner needs sizing.
5. Current tests do not prove Browser Source Live Check, managed Browser Search Run, or active Detail cancellation. These are acceptance gaps, not reasons to retain old code.
6. Production Chromium lifecycle behavior (especially forced termination/reap support available through `chromiumoxide`) needs implementation investigation in the Browser foundation. The accepted invariant is fixed even if the low-level mechanism changes.

## Independent review checklist

- Verified direct invocation count: Detection `render` = 1; Discovery `render_with_context` = 1; Detail `render_with_context` = 1.
- Verified managed construction count: 6.
- Verified trait implementations: managed, unavailable, Detection fake, Discovery fake, Discovery cancellation fake, Detail fake, posting-service fixture, command static fake = 8 total.
- Verified no shared scripted adapter exists.
- Verified runtime cleanup failure is currently discarded and teardown is outside/unbounded by request timeout.
- Verified Source Live Check Browser parity and Search Run Browser parity are absent.
- Verified browser runtime admin UI/API is not an old seam caller and should not be deleted wholesale.
- Verified all old exports and convenience injection paths are included.

## Meta-prompt handoff for planning/implementation sizing

**Goal:** Define retained final Browser Acquisition foundation slice(s) and one bounded D-007 cross-phase activation that migrates every inventory row and deletes the complete old seam without wrappers.

**Context/evidence:** Use the call graph, caller tables, eight implementations, six production construction sites, lifecycle blockers, export list, and parity matrix above. D-006 requires native Detection contributions and immutable reconciled-state dependencies; D-007 fixes shared acquisition, managed/scripted adapters, phase-specific scopes, bounded teardown, private residue, and one hard cut.

**Success criteria:** Every productive and deterministic caller uses a final typed phase adapter over the one acquisition module; phase budgets/reports remain owned by Detection/Discovery/Detail; missing browser integration tests are added; all old symbols/implementations/fakes/exports are gone; runtime admin remains functional; guard searches are clean.

**Hard constraints:** No compatibility trait, forwarding wrapper, alias, aggregate-browser translation, dual productive route, Detection-specific process implementation, provider-specific branch, or cleanup-later ticket. Activation is owned cross-phase; T14d is guard-only. Cancellation returns after bounded cleanup; cleanup failure is infrastructure failure.

**Suggested approach:** Size one non-productive final shared foundation (including scripted adapter and lifecycle tests), final thin phase adapters/budget contracts with their owning foundations, then one activation/deletion slice. If the activation is too large, reduce foundation uncertainty before it; do not split productive caller migration in a way that creates old/new coexistence.

**Validation:** Targeted Rust integration suites for Detection, Discovery, Detail, Source Live Check, Search Run, posting service, command helpers, and browser runtime lifecycle; frontend posting/source/runtime contract tests; `cargo test --manifest-path src-tauri/Cargo.toml`; `npm run build`; exact residue guards above; `git diff --check` and status review.

**Stop/escalation:** Stop when every listed row has exactly one target owner and test, and the guard is zero. Escalate only if a proposed boundary would require reopening D-006/D-007, retaining an old wrapper, changing runtime-admin product behavior, or cannot establish bounded forced termination/reap with the chosen managed-browser library.

**Resolved assumptions:** Browser administration is retained; the current aggregate is not translated; phase adapters are separate; all duplicate fakes become one scripted adapter; all productive migration/deletion belongs to one cross-phase activation.

## Acceptance evidence

The requested change is a read-only inventory artifact, not product implementation. No tests were added or changed. Existing unrelated staged/worktree changes were present before this task and were left untouched; therefore `noStagedFiles` is reported truthfully as false.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced the requested read-only D-006/D-007 Browser seam caller/deletion inventory at the authoritative /tmp path without modifying project/source files or widening into implementation."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Inventory includes verified leaf call graph, all 6 managed construction sites, all 8 trait implementations, grouped callers, final adapters/scopes/projections, parity evidence and gaps, activation ownership, exact migration/deletion targets, lifecycle risks, exports, guard terms, and uncertainties with file/line references."
    }
  ],
  "changedFiles": [
    "/tmp/job-radar-166-phase2/browser-inventory.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short && git branch --show-current && git log -1 --oneline",
      "result": "passed",
      "summary": "Established main/d00d637 baseline and confirmed unrelated pre-existing staged/modified files."
    },
    {
      "command": "repository grep/find/read inspection for ProfileBrowserClient, render/render_with_context, BrowserRuntimeRender, all callers, implementations, exports, tests and UI command chains",
      "result": "passed",
      "summary": "Verified 3 direct leaf calls, 6 managed construction sites, 8 trait implementations, no shared scripted adapter, and all higher-level caller groups."
    },
    {
      "command": "read handoff/issue-166-phase-1-decisions-handoff.md and handoff/issue-166-contract-decisions.md in full",
      "result": "passed",
      "summary": "Applied accepted D-006/D-007 without reopening them."
    }
  ],
  "validationOutput": [
    "Direct old-seam invocations: Detection render at source_profile/detection/browser.rs:60; Discovery render_with_context at posting_discovery/fetch.rs:331; Detail render_with_context at posting_detail/fetch.rs:224.",
    "ManagedProfileBrowserClient::new construction sites: app/commands.rs:430,449,468,494; search/run/execution.rs:100; search/posting/service.rs:134.",
    "No current shared scripted Browser adapter was found; duplicate deterministic implementations are fully inventoried.",
    "Project/source files were not modified; only the authoritative /tmp artifact was written."
  ],
  "residualRisks": [
    "Final module/type names and browser-free phase dependency representation remain for restructuring design, without changing accepted responsibilities.",
    "Current tests lack Browser-specific Source Live Check, Search Run, and active Detail cancellation parity.",
    "Existing managed lifecycle ignores cleanup failure and lacks bounded forced termination/reap; final implementation feasibility with chromiumoxide requires focused investigation.",
    "Repository already contained unrelated staged/modified files; they were not touched or attributed to this task."
  ],
  "noStagedFiles": false,
  "diffSummary": "Added one /tmp read-only analysis artifact; no repository diff was created by this task.",
  "reviewFindings": [
    "blocker: src-tauri/src/browser_runtime/control.rs:360 - teardown close and handler join are outside the request timeout and unbounded, contrary to accepted D-007.",
    "blocker: src-tauri/src/browser_runtime/control.rs:81 - cleanup failure is discarded; current tests explicitly preserve success after cleanup failure, contrary to typed BrowserInfrastructureFailure.",
    "blocker: no shared scripted Browser Acquisition adapter exists; six duplicate/one-off test implementations must migrate before hard-cut deletion.",
    "gap: no Browser-capable Source Live Check or Search Run parity test was found."
  ],
  "manualNotes": "Independent review should verify the counts/guard list and ensure the target DAG assigns a shared foundation plus one cross-phase activation; T14c remains Detection-specific and T14d guard-only. noStagedFiles is false solely because of the pre-existing repository baseline."
}
```
