# Issue #265 — managed Browser acquisition inventory

This inventory records the coexistence boundary after B02. `ManagedBrowserAcquisition` is the only new B01 production adapter; A02 still owns productive migration and deletion of the legacy Browser route.

## Final B01 managed edge

- `src-tauri/src/browser_runtime/managed.rs:25` — public managed adapter configured with the Job Radar Browser Runtime directory.
- `src-tauri/src/browser_runtime/managed.rs:37` — direct implementation of the phase-neutral `BrowserAcquisition` interface; it does not implement or forward `ProfileBrowserClient`.
- `src-tauri/src/browser_runtime/managed.rs:46` — installed-runtime resolution, owned launch, terminal precedence, and mandatory shutdown.
- `src-tauri/src/browser_runtime/managed.rs:88` — Chromiumoxide page creation/navigation/content acquisition behind the Browser Runtime boundary.
- `src-tauri/src/browser_runtime/managed.rs:134-346` — compiled selector/network-idle waits and interactions; navigation/action admission occurs immediately before the represented effect. Network idle observes injected fetch/XHR activity plus stable resource timing and document readiness under both the authored timeout and caller work boundary.
- `src-tauri/src/browser_runtime/managed.rs:291`, `:306`, and `:323` — Cancellation and Browser work-deadline gates around vendor operations, local wait deadlines, and sleeps.
- `src-tauri/src/browser_runtime/managed.rs:351` — exhaustive owned-launch error translation; cleanup loss alone becomes `BrowserInfrastructureFailure`.
- `src-tauri/src/profile_dsl/runtime/browser_acquisition.rs:45-121` — crate-private control facade projects B01 deadlines, Cancellation, effect admission, and atomic rendered-content admission without exposing the allowance root or Chromiumoxide DTOs.
- `src-tauri/src/browser_runtime/mod.rs:11,25` and `src-tauri/src/lib.rs:13` — final adapter module and public construction surface.
- `src-tauri/tests/browser_managed_adapter.rs:7-29` — deterministic unavailable-runtime outcome through B01.
- `src-tauri/tests/browser_managed_adapter.rs:32-103` — environment-gated, local-data final-interface probe for launch, selector wait, hidden optional click, click-until-gone, content, exact usage, one-over byte rejection, and finalized session state.
- `src-tauri/tests/browser_managed_adapter.rs:105-176` — Unix environment-gated final-interface fault probe makes only the private connected session non-writable, verifies finalization loss maps to `BrowserInfrastructureFailure`, restores permissions, and removes the test residue. Neither probe is a productive route or default network test.

## Owned lifecycle retained under the adapter

- `src-tauri/src/browser_runtime/owned.rs:376-443` — close, complete-tree observation/escalation, handler observation, guard release, and private session finalization. Finalization uses deadline-checked synchronous local filesystem steps, so timeout cannot detach a Tokio blocking removal task past return.
- `src-tauri/src/browser_runtime/owned.rs:464-489`, `:731-766` — handler abort observation and per-stage deadline caps (500/1,000/250/250 ms maximums inside the absolute hard boundaries).
- `src-tauri/src/browser_runtime/owned.rs:497-526` — failed-launch force/reap and filesystem finalization remain bounded separate stages. If force/reap cannot be confirmed, native ownership moves into process-lifetime quarantine instead of invoking another synchronous Drop wait past the hard deadline.
- `src-tauri/src/browser_runtime/owned.rs:778-948` — Unix process-group ownership; completion requires leader reap and group disappearance. A synchronous bounded Drop fallback remains the last safety net.
- `src-tauri/src/browser_runtime/owned.rs:960-1381` — Windows suspended creation, Job Object assignment, descendant-handle observation, termination, root signaling, and zero active Job processes. Windows arm64 is not a pinned runtime target; the native proof target is Windows x64.
- `src-tauri/src/browser_runtime/owned.rs:247,284,301,329,416,948,1224` — unconfirmed process ownership atomically quarantines private session residue.
- `src-tauri/src/browser_runtime/status.rs:225-246` — status cleanup removes only stale install/session workspace and skips process-local active or persistently quarantined sessions.
- Install workspace cleanup in `src-tauri/src/browser_runtime/install.rs:21-89,172-176,244-257` is distinct from acquisition-session cleanup.

## Intentionally retained A02-owned legacy route

The six productive constructors remain unchanged:

- `src-tauri/src/app/commands.rs:530`, `:549`, `:568`, and `:594`.
- `src-tauri/src/search/posting/service.rs:136`.
- `src-tauri/src/search/run/execution.rs:95`.

They still construct `ManagedProfileBrowserClient` (`src-tauri/src/profile_dsl/runtime/browser.rs:105-226`), whose old-seam call remains `render_page_html_with_actions_and_context` at `browser.rs:210` and `src-tauri/src/browser_runtime/mod.rs:22`. The old Chromiumoxide implementation and DTO/error conversion remain in `src-tauri/src/browser_runtime/control.rs:1-591` and `src-tauri/src/browser_runtime/types.rs`; B02 neither wraps nor switches them.

Runtime administration also remains unchanged for A02:

- `src-tauri/src/browser_runtime/status.rs:5-42` still routes the productive check through legacy `control::smoke_test` (`src-tauri/src/browser_runtime/control.rs:24-35`).
- `src-tauri/src/app/commands.rs:411-477` and the Tauri registrations retain install, uninstall, status, and check behavior.
- Archive, download, install, manifest, spec, status, and frontend administration are managed-runtime facilities, not old-seam residue.

## Residual constraints

- `chromiumoxide = "=0.9.1"` remains exact-pinned in `src-tauri/Cargo.toml`; all Chromiumoxide imports are confined to `browser_runtime/{managed,owned,control}.rs`. `control.rs` is retained solely for A02 coexistence.
- `Page::content()` necessarily materializes the complete rendered string before B01 can atomically admit its UTF-8 length. No rendered content reaches the caller before admission, but transient CDP/string allocation is not claimed as bounded.
- Safe quarantined residue and retained unconfirmed native ownership are private process-local lifecycle evidence. They perform no background work and are never exposed in B01 results, Diagnostics, or phase reports.
- The final-interface real probe uses a deterministic `data:` page and requires `JOB_RADAR_BROWSER_RUNTIME_DIR`; default CI performs no browser download and no network-dependent test.
