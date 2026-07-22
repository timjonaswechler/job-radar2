# Issue #268 retained Browser inventory

This inventory classifies the Browser seams intentionally retained after B03b. Line numbers were rechecked after `cargo fmt`. A02 owns the later A07–A28 productive migration/deletion groups; D03 owns Detection; T02–T07 denotes retained legacy regression fixtures. B01/B03a and B03b entries are canonical and are not migration residue.

## A07–A11 — retained Discovery branch, signatures, browser-free construction, and budget threading

- `src-tauri/src/profile_dsl/runtime/browser.rs:56-255` — legacy `ProfileBrowserClient`, unavailable client, and managed legacy implementation retained across the productive migration groups.
- `src-tauri/src/profile_dsl/runtime/discovery.rs:35-36,75-94,131-140` — legacy imports, unchanged public Discovery signature, and closed legacy backend selection.
- `src-tauri/src/profile_dsl/runtime/discovery/fetch.rs:15-27,275-364` — direct legacy Discovery Browser fetch branch, alongside but not forwarded through the canonical adapter.
- `src-tauri/src/profile_dsl/runtime/discovery/strategy.rs:4-16,128-141` and `src-tauri/src/profile_dsl/runtime/discovery/pagination/{mod.rs:10-28,page.rs:4-21,offset_limit.rs:4-21,cursor.rs:4-21,sitemap.rs:4-21}` — retained legacy type flow through Strategy and pagination execution.

## A12–A15 — retained Detail branch, signatures, and browser-free construction

- `src-tauri/src/profile_dsl/runtime/detail.rs:38-39,75-98,147-158` — legacy imports, unchanged public Detail signature, and closed legacy backend selection.
- `src-tauri/src/profile_dsl/runtime/detail/fetch.rs:7-19,130-215` — direct legacy Detail Browser fetch branch, alongside but not forwarded through the canonical adapter.
- `src-tauri/src/profile_dsl/runtime/detail/strategy.rs:3-18` and `src-tauri/src/profile_dsl/runtime/source_detail.rs:16,221-275` — retained productive candidate-scoped Detail route.

## A16–A20 — retained Source Live Check orchestration and wrappers

- `src-tauri/src/checks/source_live/mod.rs:17-20,62-98,154-198` and `src-tauri/src/checks/source_live/activation.rs:5-149` — Source Live Check and activation legacy client construction/signatures.
- `src-tauri/src/checks/mod.rs:26-29` — retained public `_with_clients` and `_with_fetcher` Source Live Check exports.
- `src-tauri/src/app/commands.rs:530-594` — productive Live Check and activation client construction.

## A21–A24 — retained Search Run Discovery callers

- `src-tauri/src/search/run/execution.rs:9-10,95-128` — Search Run legacy Discovery caller.

## A25–A28 — retained lazy posting Detail callers

- `src-tauri/src/search/posting/service.rs:9,136-150` — lazy posting Detail legacy caller.
- `src-tauri/src/app/commands.rs:594-622` — productive command construction shared with Detection.
- `src-tauri/src/lib.rs:15-18,147-162,174` and `src-tauri/src/profile_dsl/runtime/mod.rs:25-28` — retained public legacy exports required until A02.

## D03 — Detection-only Browser work

- `src-tauri/src/source_profile/detection/browser.rs:18-29` — Detection Browser strategy client boundary.
- `src-tauri/src/source_profile/detection/mod.rs:10,115-132,290` — Detection operation client selection; it does not consume posting adapters.
- `src-tauri/src/app/commands.rs:594-622` — Detection command wiring.
- `src-tauri/src/app/commands.rs:1228-1235,1333-1338,1359-1370` — command-seam Detection fixtures, injected Browser client wiring, and the Detection-only static client implementation.
- `src-tauri/tests/source/profile_detection.rs:4-8,116-1398,1475-1525` — Detection-only scripted Browser evidence.

## T02–T07 — retained old-seam regression fixtures

- `src-tauri/tests/profile_dsl_runtime/detail.rs:25-29,163-213,1276-1390` — old Detail fake/unavailable/cancellation coverage.
- `src-tauri/tests/profile_dsl_runtime/discovery.rs:12,86-101` and `src-tauri/tests/profile_dsl_runtime/discovery/cancellation.rs:80,198-240` — old Discovery fake/cancellation coverage.
- `src-tauri/tests/profile_dsl_runtime/strategy_allowances.rs:21,115-717` and `src-tauri/tests/profile_dsl_runtime/strategy_set.rs:18,487-1948` — old phase Policy/allowance regression fixtures.
- `src-tauri/tests/source_detail_execution.rs:13,92-622` and `src-tauri/tests/profile_dsl_compiler/resolution.rs:11,100` — old Source Detail/compiler caller fixtures.
- `src-tauri/tests/support/mod.rs:7,137-262` and `src-tauri/tests/profile_dsl_profiles/workday.rs:12,196` — shared browser-free fake helpers retained for A02.
- `src-tauri/src/search/posting/tests.rs:4-91` and `src-tauri/src/search/posting/tests/detail_loading/{context.rs:70-74,diagnostics.rs:57-329,basic.rs:50-280,fallback.rs:73-274,browser.rs:46-55}` — lazy posting legacy tests.
- `src-tauri/src/search/run/tests/support.rs:112,158`, `src-tauri/tests/source/live_check.rs:4-998`, and `src-tauri/tests/source/profile_detection.rs:4-1525` — retained caller-level tests.

## B01/B03a — canonical Browser acquisition and primitives

- `src-tauri/src/profile_dsl/runtime/browser_acquisition.rs:23-323,327-1064` — B01 request/control/terminal, scripted acquisition, lifecycle, and test invocation owner.
- `src-tauri/src/profile_dsl/execution_plan/capabilities.rs:14-268` and `src-tauri/src/profile_dsl/documents/fetch.rs:1-248` — B03a immutable Browser Fetch/wait/interaction compilation and authored types.
- `src-tauri/tests/browser_acquisition_contract.rs:4-508`, `src-tauri/tests/browser_scripted_adapter.rs:2-260`, `src-tauri/tests/browser_managed_adapter.rs:2-145`, and `src-tauri/tests/browser_primitives.rs:1-123` — canonical deterministic contract evidence.

## B03b — canonical posting adapters

- `src-tauri/src/profile_dsl/runtime/browser_phase.rs:20-160` — exhaustive `PhaseBrowser`, rendered-input projection, exhaustive B01 terminal translation, and privacy-safe Diagnostics.
- `src-tauri/src/profile_dsl/runtime/discovery/browser_adapter.rs:1-25` and `src-tauri/src/profile_dsl/runtime/discovery.rs:96-129` — Discovery adapter and final operation.
- `src-tauri/src/profile_dsl/runtime/detail/browser_adapter.rs:1-25` and `src-tauri/src/profile_dsl/runtime/detail.rs:100-145` — Detail adapter and final operation.
- `src-tauri/tests/discovery_browser_adapter.rs:1-578` and `src-tauri/tests/detail_browser_adapter.rs:1-587` — adapter parity, acceptance separation, exact report equality/serialization, byte and non-byte failure/budget/Cancellation terminals, Diagnostic order/privacy, and browser-free zero-call evidence.
