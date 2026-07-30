# Research: Interactive Source Session browser host

## Question

Which browser-host architecture can safely and portably deliver Job Radar's integrated, single-tab **Interactive Source Session** and **Element Picker** without creating a second productive Browser Acquisition lifecycle?

This report answers [Determine the browser host architecture for Interactive Source Sessions](https://github.com/timjonaswechler/job-radar2/issues/308), part of the wayfinding map [Wayfind Interactive Source Sessions and Guided Source Repair](https://github.com/timjonaswechler/job-radar2/issues/44).

## Reviewed revisions

- t3code commit [`9dd425b2234c062b4767583e42d4b2c1aabab15d`](https://github.com/pingdotgg/t3code/tree/9dd425b2234c062b4767583e42d4b2c1aabab15d), retrieved 2026-07-30.
- Local Tauri checkout commit [`3f5d3984bc8916b5dd31289b19284637ede37e3d`](https://github.com/tauri-apps/tauri/tree/3f5d3984bc8916b5dd31289b19284637ede37e3d), which identifies itself as Tauri `2.11.5`, reviewed at `/Users/tim-jonaswechler/GitHub-Projekte/test/tauri`.
- Job Radar baseline commit `6dd0effea70e30b71c6acd659bdc22cc331384a9`, including the current Browser Runtime and Profile DSL contracts.

These revisions are pinned because all three implementations are active codebases and Tauri's multiwebview API is explicitly unstable.

## Answer

Use **two browser hosts with one domain protocol**:

1. A UI-owned Tauri child `Webview` hosts the visible Interactive Source Session inside the main window's right-hand split. It is a single ephemeral system-Webview page with Job Radar-owned browser chrome and picker code.
2. The existing managed Chrome for Testing/`chromiumoxide` Browser Runtime remains the only productive `BrowserAcquisition` implementation. It continues to own bounded navigation, waits/interactions, rendered-content admission, Cancellation, and whole-process-tree teardown for Source Live Checks and Search Runs.
3. The hosts share only typed, bounded domain messages and declarative artifacts: navigation state, picker purpose, selected-element evidence, repair-draft updates, Diagnostics, and the eventual Direct Source Specialization or Source-owned Access Path. They do **not** share a page object, browser process, user-data directory, cookies, storage, CDP target, or lifecycle owner.
4. Evidence selected in the system Webview is advisory authoring evidence. Before final application, the complete draft must compile and a Source Live Check must execute it through managed Chrome. This is the necessary guard against WKWebView/WebView2/WebKitGTK versus Chrome DOM and behavior drift.

This is not a second Browser Acquisition route. The Interactive Source Session is a bounded, user-driven authoring surface that cannot be called by Detection, Discovery, Detail, Source Live Check, or Search Run execution. Its output becomes ordinary authored Source data only after review and validation.

## Product boundary established by the map

The browser host must support the already-agreed Guided Source Repair flow:

- one integrated browser area, not a general-purpose browser and not a Search Run executor;
- editable HTTP(S) URL, back, forward, reload, cross-host navigation with visible origin changes, and no tabs;
- manual entry from Source editing and contextual entry from Diagnostics;
- repair of only the incomplete or failing phase, followed by a full-Source check;
- a guided overlay that asks for one missing element at a time, shows the current deterministic data and immediate selection preview, and permits retry;
- explicit `not present` and `cannot determine reliably` outcomes, with required unresolved fields blocking application;
- one reviewable repair draft, atomically applied only after final user confirmation and successful compiler/live-check validation;
- Source-only authoring: Direct Source Specialization for a profile-selected Source or a Source-owned Access Path when no Source Profile fits; never a Source Profile mutation;
- a Built-in Source is copied to a new editable draft Source before repair;
- resumable structured repair draft, while browser state, HTML, screenshots, and selected page content remain ephemeral;
- deterministic core behavior without an AI Provider; optional agent assistance has no privileged compiler or live-check path;
- authenticated sessions, cookie transfer, login, and manually solved challenges are future work, not part of this destination.

## t3code: useful reference and non-portable implementation

t3code implements the desired interaction as an Electron browser preview. Its renderer mounts an Electron `<webview>` with a dedicated persistent partition, a picker preload, and security preferences; CSS places the guest inside the preview surface. The application adds its own chrome with editable URL input, back/forward/reload, picker state, and other controls. The desktop manager registers the guest `WebContents`, owns navigation state and history, redirects `window.open` back into the same guest, focuses the page for picking, and cancels a pick when top-level navigation or destruction begins. [hosted webview](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/web/src/browser/HostedBrowserWebview.tsx) · [browser chrome](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/web/src/components/preview/PreviewChromeRow.tsx) · [desktop manager](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/Manager.ts)

The picker is an isolated application bundle loaded as the guest preload. It installs capture-phase pointer and keyboard listeners, highlights hovered/selected elements, suppresses page clicks during annotation, supports Escape cancellation, generates element context with `react-grab`, and submits a structured draft to the main process. The main process owns the terminal result, validates its shape, adds a screenshot separately, and removes listeners on completion, cancellation, navigation, or destruction. [picker preload](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/PickPreload.ts) · [payload validator](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/PickedElementPayload.ts) · [typed contracts](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/packages/contracts/src/ipc.ts)

The transferable lessons are:

- treat picking as an explicit, cancellable session with one backend owner and exactly one terminal result;
- keep browser chrome, guest-page interaction, host lifecycle, payload validation, and application workflow as separate modules;
- install only fixed application-owned guest code;
- validate guest output again at the trusted host boundary;
- cancel on navigation, destruction, frontend unmount, explicit cancel, and superseding selection;
- maintain typed navigation and picker state rather than deriving it opportunistically in React;
- keep screenshot creation separate from DOM selection. Job Radar can therefore omit screenshots without weakening the picker contract.

t3code's implementation cannot be transplanted. Electron ships one Chromium runtime and exposes `<webview>`, `WebContents`, `ipcRenderer`, `navigationHistory`, session partitions, and preload sandbox controls that Tauri does not reproduce. t3code also intentionally uses `contextIsolation=false` so `react-grab` can inspect React developer hooks, compensating with Electron renderer sandboxing and disabled Node integration. Job Radar does not need React component/source attribution on arbitrary career sites and should not weaken isolation for it. [t3code Webview preferences](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/WebviewPreferences.ts)

The previous Orca-only framing in this report is therefore superseded. Its still-valid protocol lessons—one owner, typed states, strict cleanup, bounded payloads, and optional capture as a separate derivative—are preserved above, but t3code is the closer current reference for integrated browser chrome and picker UX.

## Tauri multiwebview host

The local `multiwebview` example proves that one native Tauri `Window` can own multiple child Webviews, including remote `WebviewUrl::External` pages. It creates four independently positioned child Webviews through `Window::add_child`. The example is enabled only with `cargo run --example multiwebview --features unstable`; Tauri's manifest marks the example as requiring `unstable`. [example source](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/examples/multiwebview/main.rs) · [example README](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/examples/multiwebview/README.md) · [example manifest](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/crates/tauri/Cargo.toml)

At the reviewed revision, the Rust APIs needed for an Interactive Source Session exist:

- `Window::add_child` creates a desktop child Webview and returns its handle;
- `Webview::set_bounds`, `set_position`, `set_size`, `show`, `hide`, `set_focus`, and `close` support native split layout and lifecycle;
- `Webview::navigate`, `reload`, and `url` support URL chrome;
- `WebviewBuilder::on_navigation` can reject navigation before it proceeds;
- `on_new_window` can deny popups or route them intentionally;
- `on_download` can reject requested downloads;
- `on_page_load` and `on_document_title_changed` provide navigation state;
- `initialization_script` runs application-owned JavaScript in each top-level document after the global object exists but before page parsing and page scripts;
- `eval` and the newer `eval_with_callback` allow the Rust host to drive a narrow guest script and retrieve a JSON-serialized result;
- `incognito` and `data_directory` exist, but their support and semantics vary by platform.

[Tauri `Window::add_child`](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/crates/tauri/src/window/mod.rs) · [Tauri `WebviewBuilder` and `Webview`](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/crates/tauri/src/webview/mod.rs)

The integration is a native overlay, not a DOM child. Job Radar's existing `main` Webview can continue rendering the whole React shell while a second native child Webview is positioned over the reserved browser panel. The React splitter reports logical panel bounds to a Rust session owner, which applies `set_bounds`; window resize, scale-factor change, sidebar/layout change, hide/show, and teardown must all resynchronize or close the child. `auto_resize` alone cannot express an arbitrary draggable split.

Tauri does not currently expose portable `go_back`, `go_forward`, or `canGoBack`/`canGoForward` methods on its Rust `Webview`. An MVP can execute fixed `history.back()`/`history.forward()` scripts and maintain conservative navigation state from page-load events plus a bundled History API observer. The spike must prove redirects, same-document navigation, and disabled-button state on all target platforms; it must not reach into WKWebView/WebView2/WebKitGTK directly merely to copy Electron's `navigationHistory` API.

### Platform and packaging consequences

Tauri uses the operating system's Webview: WKWebView on macOS, WebView2 on Windows, and WebKitGTK on Linux. The engine is dynamically provided rather than Job Radar's pinned Chrome for Testing. That keeps the bundle small and makes an in-window child view practical, but it makes DOM/rendering behavior platform-dependent. [Tauri process model](https://v2.tauri.app/concept/process-model/) · [WRY platform considerations](https://github.com/tauri-apps/wry#platform-considerations)

Enabling `tauri = { version = "2", features = ["unstable"] }` is required. Because Job Radar currently declares broad `version = "2"` and does not commit a Cargo lockfile, the implementation should pin a tested Tauri minor or minimum before relying on the unstable API, especially if it uses `eval_with_callback`, which appears in the reviewed 2.11.5 source. No second browser binary should be packaged: the interactive host uses the same system-Webview dependency that already renders Job Radar, while Chrome for Testing remains the separately installed managed runtime.

The packaging gate must exercise packaged builds on macOS, Windows, and Linux rather than relying only on dev mode. Linux needs explicit X11 and Wayland coverage because WRY's child-view integration differs between native child windows and GTK container paths. The feature should remain unavailable with a structured Diagnostic on any platform where child Webview creation, injection, or cleanup cannot meet the contract.

## Current Job Radar Browser Runtime

ADR 0003 and the current implementation define a deliberately different lifecycle. `ManagedBrowserAcquisition` resolves only the pinned installed Chrome for Testing runtime, creates one owned Chromium process tree and private ephemeral session directory, connects with `chromiumoxide`, opens a page, performs one bounded request's compiled navigation/waits/interactions, admits rendered UTF-8 content to the caller's allowance, and shuts down before returning. Chromium is launched with `--headless=new`. Cleanup failure can override an otherwise successful operation. [ADR 0003](../adr/0003-managed-browser-runtime.md) · [`ManagedBrowserAcquisition`](../../../src-tauri/src/browser_runtime/managed.rs) · [owned Chromium lifecycle](../../../src-tauri/src/browser_runtime/owned.rs)

That contract is intentionally request-shaped and headless. Turning its private `Page` or `OwnedChromiumSession` into a long-lived UI object would violate its current ownership, deadline, byte-admission, and terminal-cleanup guarantees. Removing `--headless=new` only makes a separate Chromium window visible; it does not embed Chromium in a Tauri panel.

The managed runtime remains authoritative where reproducibility matters. Browser fetch is one declarative Profile DSL fetch mode, not a profile type or interactive session. The DSL forbids arbitrary JavaScript, credentials, cookies, login flows, CAPTCHA bypass, and unbounded browser interactions. The Interactive Source Session must not become an indirect way to add any of those behaviors to an Execution Plan. [Profile DSL PRD](../prd/declarative-source-profile-dsl.md)

## Recommended architecture seam

```text
React Guided Source Repair UI (trusted app Webview)
  |  typed commands/events: open, bounds, navigate, begin/cancel pick
  v
InteractiveSourceSession owner (Rust/Tauri application module)
  |-- owns exactly one child system Webview and its generation
  |-- filters navigation/new-window/download requests
  |-- injects one fixed versioned picker script
  |-- validates and normalizes one bounded terminal selection
  `-- emits navigation/picker state and repair-draft evidence
                 |
                 v
        Source repair draft (Source-only authored intent)
                 |
        compile + complete Source Live Check
                 v
ManagedBrowserAcquisition (pinned headless Chrome; unchanged)
```

The public interface should describe user intent and outcomes, not Tauri handles:

```text
InteractiveSourceSessionId
SessionGeneration
NavigationRequest { sessionId, expectedGeneration, url }
NavigationState { url, title?, loading, canGoBack?, canGoForward?, originChanged }
PickerRequest { sessionId, expectedGeneration, repairStepId, purpose, timeoutMs }
PickerOutcome = Selected(ElementEvidence) | NotPresent | CannotDetermine | Cancelled | Failed
ElementEvidence {
  documentUrl,
  tag,
  selectorCandidates[{ selector, matchCount, stabilityFlags }],
  safeAttributes,
  normalizedTextPreview,
  bounds,
  frameKind,
  diagnostics
}
```

Every collection and string needs an explicit maximum. `sessionId`, generation, current top-level URL, repair-step identity, and active picker nonce must match at the Rust boundary. Page HTML, form values, password fields, cookies, storage, authorization data, screenshots, and unrestricted computed styles must not cross it.

### Guest-to-host result transport

Two transports are technically viable and must be proven in the spike:

1. **Backend-directed read with `eval_with_callback` (preferred).** The initialization script owns the overlay and places one bounded result in guest session state. Rust periodically or after an explicit app action evaluates a fixed expression and receives the JSON-serialized result through the native callback. The remote page receives no Tauri command permission. Page code can still tamper with any same-page JavaScript state, so this transport reduces authority but does not remove the need for validation. It requires Tauri 2.11.5-era API availability and cross-platform tests for callback delivery, navigation races, and polling cleanup.
2. **One dedicated remote-origin command.** The guest calls a single picker-result command scoped to the picker Webview and allowed HTTP(S) origins. Rust validates the complete capability tuple and payload. This gives immediate push delivery but broad dynamic cross-host navigation makes remote capability configuration difficult and increases the IPC attack surface.

Do not expose a generic `eval`, event bus, filesystem/plugin permission, or main-window capability to the remote guest. Tauri capabilities can target individual Webview labels, and its own guidance says multiwebview windows should use `webviews` rather than `windows` for fine-grained access. Job Radar's current capability targets `windows: ["main"]`; before adding the child, it should be narrowed to the trusted `main` Webview label and the picker Webview should match no ordinary application capability. Remote API access is denied by default and must stay denied unless the architecture explicitly selects and scopes the second transport. [Tauri capabilities](https://v2.tauri.app/security/capabilities/) · [capability schema guidance](https://v2.tauri.app/reference/acl/capability/) · [runtime authority](https://v2.tauri.app/security/runtime-authority/)

## Navigation, cancellation, and cleanup

The Rust owner should enforce the following state machine:

- only `http:` and `https:` top-level URLs are accepted;
- cross-host navigation is allowed and reported as an origin change;
- `file:`, `javascript:`, `data:`, `blob:` as a top-level destination, Tauri/app protocols, and unknown schemes are rejected;
- downloads are denied through `on_download` in the first version;
- `window.open`/`target=_blank` is denied as a new window and, after validation, may be loaded in the same single page;
- a navigation start increments the document generation, cancels the active pick, invalidates unconfirmed page evidence, and retains only the structured repair draft;
- page load reinjects/activates the fixed picker version for the new generation;
- starting a new picker cancels the previous one;
- explicit cancel, Escape, React unmount, panel close, Source change, app-window close, child crash, and shutdown each produce at most one terminal outcome and remove guest overlays/listeners;
- the child Webview is closed before its owner is released; a close failure becomes a structured infrastructure Diagnostic rather than an invisible stale view;
- no page data is logged. URLs in Diagnostics should omit fragments and redact query values where they may carry secrets.

`on_navigation`, `on_new_window`, and `on_download` are the authoritative Rust gates. The guest script may improve UX but cannot be the security boundary because the page can mutate its own JavaScript and DOM.

## Selector and repair semantics

The earlier selector findings remain valid, with one important change: the system-Webview result cannot be considered runtime proof.

- Generate several ranked candidates and verify each in the current document.
- Prefer stable semantic attributes and stable container anchoring; use generated-looking IDs/classes and structural positions only with warnings.
- Record a bounded evidence preview and current match count, not raw DOM or a full HTML path.
- Top-level document only is the safe first target. Cross-origin iframes, closed shadow roots, and selectors requiring a richer frame/shadow path should return explicit unsupported Diagnostics.
- Every accepted field selection updates only the repair draft and immediate preview. Retry replaces the step's unconfirmed evidence.
- Final application still requires schema validation, Profile Compiler validation, and a full Source Live Check through managed Chrome.

Engine drift is expected: a site may serve different markup or execute differently under WKWebView, WebView2, WebKitGTK, and Chrome. A selector that is unique in the Interactive Source Session can therefore fail in the Source Live Check. The UI must explain this as a validation mismatch and return the user to the affected step rather than silently weakening acceptance.

## Future authenticated sessions

Tauri exposes Webview cookie and browsing-data methods, but that does not make authentication portable to managed Chrome. Platform stores, cookie partitioning, WebAuthn/passkeys, client certificates, SameSite behavior, challenge state, and protected storage differ across system Webviews and Chrome. Exporting cookies would also create a new sensitive-data boundary that the current Profile DSL explicitly excludes.

For this destination, use an ephemeral unauthenticated system Webview and do not inspect or transfer cookies. A later effort may choose either:

- a separate visible managed-Chrome authorization session whose browser context can be explicitly handed to a new authenticated acquisition contract; or
- a security-reviewed session broker with origin-scoped consent, storage, expiry, revocation, redaction, and deletion semantics.

Neither path should be implied by the MVP architecture, and manual challenge solving must never become automated CAPTCHA bypass.

## Options

### A. Tauri child system Webview plus shared domain protocol — viable, recommended

**Strengths:** Delivers the requested integrated split; reuses Tauri's native multiwebview path; needs no second packaged engine; keeps Search Runs and live checks on the existing bounded runtime; supports fixed early picker injection and native navigation/download gates.

**Costs:** Unstable Tauri API; native bounds synchronization; no portable native history-state API; OS-engine differences; a second non-productive browser lifecycle must still be modeled and cleaned up; final managed-Chrome validation is mandatory.

### B. Separate headed managed-Chrome window — viable fallback, not the destination

Launch the pinned runtime without headless mode and control it through CDP. This maximizes parity with Source Live Check and offers a plausible future authenticated-session path.

It does not embed into the Tauri split, window focus/placement is inferior, and attempting to parent or embed the Chrome window would require platform-specific unsupported work. Retain this only as a fallback if the child-Webview spike fails a supported platform.

### C. Stream managed Chrome into the React panel — technically possible, reject

CDP screenshots/screencast plus forwarded pointer/keyboard input could visually imitate an embedded browser. It would add latency, accessibility failures, input-method and focus complexity, large frame traffic, screenshot handling contrary to current privacy decisions, and a new long-lived CDP lifecycle. It solves the engine mismatch by creating a much larger product and security problem.

### D. Platform-native Webview bridges or embedded CEF — reject

Direct WKWebView/WebView2/WebKitGTK code could fill history or IPC gaps, while CEF could unify the renderer. Both bypass Tauri's portable seam, multiply packaging and test obligations, and create exactly the parallel browser ownership the project is trying to avoid. Reconsider only after a bounded multiwebview prototype demonstrates an unfixable Tauri limitation.

### E. Reuse the current `ManagedBrowserAcquisition` page/session — reject

The adapter's successful result is rendered text after bounded work and mandatory teardown. A long-lived visible page cannot be borrowed without changing its public contract and violating its terminal cleanup invariants. Interactive authoring is not an Acquisition request.

## Risks and required proofs

| Risk | Consequence | Required proof or mitigation |
| --- | --- | --- |
| `unstable` multiwebview API changes | Upgrade breakage | Pin a tested Tauri minor/minimum; isolate Tauri calls behind one application module. |
| Native overlay bounds drift | Browser covers wrong UI or intercepts clicks | Prototype splitter, resize, scale-factor, title-bar, minimize/restore, hide/show, and teardown behavior. |
| Remote guest gains app privileges | Source page can call filesystem/app commands | Capability targets trusted Webview label only; no remote capability for guest; prefer backend-directed callback transport. |
| Page tampers with injected script/result | False or oversized evidence | Fixed script, per-generation nonce, strict Rust validation, payload ceilings, current-URL and step matching, managed-Chrome revalidation. |
| Navigation races picker completion | Stale selection applied to new document | Increment generation and settle cancellation before accepting later payloads. |
| System-Webview/Chrome divergence | Repair passes preview but fails productive runtime | Treat picker as advisory; require fresh full Source Live Check before apply. |
| History controls inconsistent | Incorrect back/forward availability | Cross-platform spike around redirects and History API; conservative disabled-state UX. |
| Iframe/shadow content | Selector cannot be replayed | Top-level DOM only in MVP; explicit unsupported Diagnostic. |
| Webview crash/leak | Invisible input interception or stale native child | Backend owner closes child and emits terminal infrastructure Diagnostic; packaged crash/reopen test. |
| Future auth assumptions leak into MVP | Cookies/credentials cross trust boundaries | Ephemeral unauthenticated context; no cookie APIs, export, persistence, or Profile DSL fields. |

## Recommended next architecture decision

Adopt option A as the target and use a small cross-platform technical prototype to decide its last implementation-level fork: backend-directed `eval_with_callback` result retrieval versus one narrowly scoped result command. The prototype should prove, in order:

1. the existing main React Webview plus one external child Webview can form a draggable right-hand split without input or resize artifacts;
2. HTTP(S)-only navigation, same-page popup routing, download denial, editable URL, reload, and conservative back/forward behavior;
3. fixed top-frame script installation before page code, hover/click/retry/Escape, navigation cancellation, and bounded typed result delivery with no guest app capability;
4. teardown on close, unmount, crash, and app shutdown;
5. the same accepted selector can be compiled into a Source-only draft and checked against a deterministic local site through `ManagedBrowserAcquisition` without sharing browser state;
6. packaged smoke behavior on macOS, Windows, Linux/X11, and Linux/Wayland.

If proof 1 or 3 fails on a supported platform, use option B as an explicit degraded fallback and return to the architecture ticket. Do not respond by exposing platform-native handles throughout the application or weakening the managed Browser Acquisition contract.

## Sources

- [t3code at reviewed commit](https://github.com/pingdotgg/t3code/tree/9dd425b2234c062b4767583e42d4b2c1aabab15d)
- [t3code hosted browser Webview](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/web/src/browser/HostedBrowserWebview.tsx)
- [t3code browser chrome](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/web/src/components/preview/PreviewChromeRow.tsx)
- [t3code picker preload](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/PickPreload.ts)
- [t3code picker session owner](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/Manager.ts)
- [t3code typed IPC contracts](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/packages/contracts/src/ipc.ts)
- [t3code guest payload validator](https://github.com/pingdotgg/t3code/blob/9dd425b2234c062b4767583e42d4b2c1aabab15d/apps/desktop/src/preview/PickedElementPayload.ts)
- [Tauri `multiwebview` example at reviewed revision](https://github.com/tauri-apps/tauri/tree/3f5d3984bc8916b5dd31289b19284637ede37e3d/examples/multiwebview)
- [Tauri child `Window` API at reviewed revision](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/crates/tauri/src/window/mod.rs)
- [Tauri `WebviewBuilder`/`Webview` API at reviewed revision](https://github.com/tauri-apps/tauri/blob/3f5d3984bc8916b5dd31289b19284637ede37e3d/crates/tauri/src/webview/mod.rs)
- [Tauri process model](https://v2.tauri.app/concept/process-model/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri runtime authority](https://v2.tauri.app/security/runtime-authority/)
- [WRY child Webviews and platform considerations](https://github.com/tauri-apps/wry#child-webviews)
- [Job Radar managed Browser Runtime decision](../adr/0003-managed-browser-runtime.md)
- [Job Radar declarative Profile DSL](../prd/declarative-source-profile-dsl.md)
- [Job Radar managed Browser Acquisition](../../../src-tauri/src/browser_runtime/managed.rs)
- [Job Radar owned Chromium lifecycle](../../../src-tauri/src/browser_runtime/owned.rs)
