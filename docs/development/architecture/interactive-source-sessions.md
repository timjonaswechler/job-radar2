# Interactive Source Sessions production architecture

This specification resolves [issue #311](https://github.com/timjonaswechler/job-radar2/issues/311) and turns the accepted browser-host research, [ADR 0018](../adr/0018-guided-source-repair-domain-and-protocol.md), and the approved Guided Split prototype into production module seams and delivery slices.

## Destination

Guided Source Repair repairs one existing concrete Custom Source through bounded user evidence. The user works in a dedicated split workspace: Source context remains on the left, while trusted browser chrome, one visible page, Element Evidence, and Repair Step actions occupy the right. No picker action mutates the Source. A proposal can be applied only after compilation, a complete candidate Source Live Check through managed Chrome, review, and exact confirmation.

The architecture uses two intentionally different browser hosts:

- a Tauri child system Webview for the ephemeral, user-driven Interactive Source Session; and
- the existing managed Chrome for Testing runtime as the only productive Browser Acquisition implementation used by Source Live Checks and Search Runs.

They share typed intent, bounded evidence, and declarative Source data. They never share a browser process, page object, user-data directory, cookies, storage, CDP target, or lifecycle owner. There is no common browser-host interface because the two modules are not interchangeable.

## Ownership and dependency direction

```text
React Guided Source Repair workspace
  -> validated TypeScript Tauri adapters
    -> Desktop Interactive Source Session owner
       -> one child system Webview + fixed guest script
    -> sources::repair
       -> installed Source state + candidate Source Live Check
          -> source-engine compiler/execution
             -> existing Desktop HTTP/managed-Chrome adapters
```

Dependency direction remains Desktop → `sources` → `source-engine`. `search-resolution` continues to consume only the deliberate `source-engine::execution` interface.

### React

The trusted main Webview owns:

- the dedicated `/sources/repair` workspace and route lifecycle;
- Source context, Repair Steps, progress, Evidence preview, review, and user-facing Diagnostics;
- editable URL, Back, Forward, Reload, origin display, focus actions, and browser status;
- the accessible resizable split and logical page rectangle;
- subscription, sequence filtering, snapshot recovery, stale-response suppression, and cleanup through feature orchestration.

React does not receive a Tauri Webview handle, parse guest JSON directly, infer repairability from Diagnostic text, or apply a Source through the ordinary edit command.

### Desktop/Tauri

A Desktop `interactive_source_session` module owns:

- exactly one child system Webview for each active session;
- creation, label, ephemeral storage configuration, bounds, visibility, focus, and close;
- authoritative HTTP(S) navigation, popup, unsupported-scheme, and download gates;
- fixed versioned guest-script installation and backend-directed result retrieval;
- page/session generations, picker terminal ordering, heartbeat, and teardown;
- redacted `infrastructure` Diagnostics and targeted event delivery to the trusted `main` Webview.

Tauri handles, WRY types, guest callback details, and platform-specific behavior remain private. The module does not depend on or implement productive Browser Acquisition.

### `sources::repair`

One Tauri-free deep module owns:

- Repair Draft loading, persistence, revisions, limits, quarantine, and receipts;
- deterministic Diagnostic-to-Repair-Step projection;
- answer admission, dependency invalidation, and safe Element Evidence;
- typed Source-authored intent instead of JSON patches;
- unpersisted candidate preparation and complete Source Live Check orchestration;
- immutable Repair Proposals and fingerprints;
- write-ahead, generation-checked, status-preserving Source replacement;
- report-pending recovery and idempotent confirmation.

Its host-facing interface exposes use cases such as `open`, `change`, `stage`, `confirm`, `snapshot`, and `discard`; exact local names may be refined. It does not expose paths, compiler plans, Effective Source Profiles, raw browser data, or internal persistence operations.

### `source-engine`

`source-engine` remains unaware of Guided Source Repair. It owns Source Behavior Language semantics, compilation, selector grammar, immutable Execution Plans, productive execution, and the canonical Structured Diagnostic schema. The required generic additions are the `infrastructure` Diagnostic category and a small canonical CSS selector-validation interface returning a typed accepted/rejected result without exposing matcher plans.

No profile key, host name, ATS family, or profile-specific Rust mapping may enter repair behavior.

## Source scope and initial capability

The repository has Built-in Source Profiles but no Built-in concrete Sources. Guided Source Repair therefore operates only on existing Custom Sources. It does not introduce a Built-in Source catalogue or copy-to-draft path.

A session remains Source-owned. It does not create an unowned Source from an arbitrary URL. Because Source documents have no universal entry-point field, `open` accepts a transient, user-confirmed HTTP(S) URL. The trusted chrome may seed it from a supported `sourceConfig.startUrl` or the initiating Diagnostic's request URL. It must not infer a visible entry point from an API fetch URL.

The first repair capability is deliberately bounded to existing declarative structure:

- required Source Config values through typed manual entry;
- the selector of an existing CSS `select` primitive in an HTML Discovery or Detail Strategy;
- existing or missing canonical extraction fields in an existing HTML Strategy using `css_text` or `css_attribute`;
- Discovery provider URL, title, company, locations, and description;
- Detail title, company, locations, and description.

Repair may author Direct Source Specialization for a profile-selected Source or change an existing Source-owned Access Path. It does not synthesize fetch, parse, pagination, policy, a new Strategy, or an entire Access Path. Missing prerequisite structure is non-repairable and blocks staging with a Structured Diagnostic.

`sources::repair`, not React, decides repairability. It projects stable Diagnostic code, JSON Pointer, strategy key, typed details, and compiler provenance into a Repair Step with an explicit `discovery` or `detail` phase. Message text never controls repairability.

## Protocol and data shapes

Keep three representations separate:

1. the untrusted guest callback envelope;
2. the trusted main-Webview Tauri transport DTO;
3. `sources::repair::ElementEvidence`.

Each transition validates and narrows data. Raw guest JSON is never deserialized directly into a persisted domain type.

### Guest and transport limits

The pre-parse boundary is independent of the 256 KiB persisted Draft limit. The initial canonical ceilings are:

| Value | Limit and admission rule |
| --- | --- |
| Raw guest callback | 64 KiB UTF-8 JSON; reject before deserialization when larger or invalid UTF-8 |
| Trusted session event or snapshot | 128 KiB serialized JSON |
| Opaque identity or nonce | 64 ASCII characters |
| Input/current/document URL | 4,096 UTF-8 bytes; HTTP(S) only |
| Persisted evidence URL | 4,096 UTF-8 bytes after removing user info, query, and fragment; reject if still larger |
| Document title | 512 Unicode characters |
| Element tag | 64 lowercase ASCII characters |
| Selector candidates | at most eight; 2,048 UTF-8 bytes each; no NUL/control characters |
| Match count | saturate at 10,000 and carry an explicit truncation flag |
| Safe attributes | at most sixteen entries; 64 ASCII bytes for the fixed name and 512 Unicode characters for the sanitized value |
| Normalized preview | 512 Unicode characters |
| Element Evidence Diagnostics | at most sixteen; generated by trusted Rust, never accepted from guest JSON |
| Interactive Diagnostic | 512-character message; at most sixteen top-level detail keys and 4 KiB serialized details |
| Geometry | one four-number rectangle; finite values only, clamped to the parent client area |

The safe attribute-name allowlist is exactly `id`, `class`, `href`, `role`, `aria-label`, `name`, `title`, `itemprop`, `rel`, `type`, `data-testid`, `data-test`, `data-qa`, `data-cy`, `data-automation-id`, and `data-field`. Rust accepts `href` only after reducing it to scheme/host/port/path; guest-side reduction is treated as untrusted input and repeated at admission. The guest envelope rejects unknown fields and never accepts form values, `value`, `srcdoc`, event-handler attributes, style, password-field evidence, cookies, storage, authorization material, or guest-authored Diagnostics.

### Identities

The protocol carries opaque typed identities for:

- Interactive Source Session;
- session/document generation;
- Repair Draft and revision;
- Repair Step;
- Picker operation;
- exact Source/Profile generation;
- Repair Proposal fingerprint;
- monotonically increasing event sequence.

Every picker request and terminal result echoes session, generation, step, and operation identity. Late or duplicate results cannot settle an operation twice.

### Tauri synchronization

Use typed commands for user intent, a targeted session event stream for live changes, and a snapshot command for initial load and resynchronization. React subscribes before opening a session. It rejects stale sequences and recovers gaps through a snapshot.

The session command family covers `open`, `navigate`, `beginPicker`, `cancelPicker`, `close`, focus intent, and visibility. Draft discard belongs only to the separate `sources::repair` transport. Bounds updates are a separate Desktop layout concern rather than Source-repair domain intent.

### Picker outcomes

The closed Picker Outcome remains:

- `Selected(ElementEvidence)`;
- `NotPresent`;
- `CannotDetermine`;
- `Cancelled(reason)`;
- `Failed(kind)`.

Navigation invalidates the page before cancelling an active picker. A new picker cancels the previous one. Teardown wins over pending work.

## Child Webview host

### Chrome and bounds

React owns the browser toolbar and reports rounded logical CSS-pixel bounds for the reserved page rectangle. It uses the accessible resizable-panel primitive and coalesces `ResizeObserver`, layout, and window changes to at most one update per animation frame.

Rust validates finite, nonnegative, bounded values, clamps them to the parent client area, and is the only owner of native `set_bounds`, show, hide, focus, and close. Empty, off-screen, minimized, or unusably small geometry hides the child and cancels an active picker while retaining the session and Draft. Leaving the route, changing Source, closing the workspace, or shutting down closes the child.

A React Dialog or Drawer cannot safely cover a native child. The final review replaces the browser region after an acknowledged hide. Any global overlay that intersects the child follows the same hide-before-open rule.

### Navigation

Only top-level `http:` and `https:` destinations are accepted. Origin changes are visible. `file:`, `javascript:`, `data:`, `blob:`, Tauri/application protocols, unknown schemes, and downloads are denied. New-window requests are denied or, after the same validation, redirected into the single page.

Back/Forward use fixed history operations with conservative availability derived from page-load and bundled History API observations. No platform-native Webview handle escapes the adapter to improve history behavior.

### Guest injection and result retrieval

The fixed application-owned top-frame script is bundled with Desktop. It owns in-page highlighting, click suppression while picking, bounded selector generation, immediate preview, keyboard interaction, Escape cancellation, and cleanup. It self-guards against subframe execution.

Backend-directed `eval_with_callback` is the intended result transport. Rust evaluates only fixed expressions and limits callback bytes before parsing. The script catches exceptions into a typed envelope because callback exceptions are not portable.

A guest-result command is allowed only if packaged proof shows that Tauri can restrict one command to the generated child label without wildcard remote-origin authority or ordinary main-Webview permissions. Otherwise the affected platform remains unavailable. No generic eval, event bus, plugin, filesystem permission, or app command is exposed to the guest.

### Heartbeat and terminal cleanup

Tauri has no portable renderer-crash event. Rust therefore runs a fixed, data-free generation heartbeat at conservative bounded intervals; active picker polling serves the same purpose. Repeated failures fail and close the session with one typed host error, attempt child cleanup, retain the Draft, and make the session unusable. If a Picker is active, that Picker additionally settles exactly once as `Failed(host_unresponsive)` with its existing operation identity; an idle session does not invent a Picker outcome.

Picker cleanup and child-session teardown are distinct serialized paths. Navigation, supersession, timeout, explicit Picker cancellation, and temporarily unusable geometry settle only the active Picker and remove guest interaction while retaining the child session. Close, route exit, Source change, shutdown, unmount, and heartbeat failure tear down the child session. Cleanup failure is mapped at Desktop composition to an `infrastructure` Diagnostic rather than silently leaving a native child.

## Security and privacy

- Pin the tested Tauri/Tauri-build versions, enable `unstable`, commit `src-tauri/Cargo.lock`, and document the upgrade gate.
- Replace window-wide capability targeting with `webviews: ["main"]`; generated guest labels match no ordinary capability.
- Emit session events only to the trusted main Webview.
- Use the strongest supported nonpersistent/incognito mode plus a private session directory where applicable; clear browsing data and owned files during teardown.
- Refuse to open with an `infrastructure` Diagnostic when ephemeral behavior cannot be guaranteed.
- Never persist or log HTML, screenshots, cookies, storage, credentials, authorization data, form values, password fields, or unrestricted computed styles.
- Persisted evidence URLs retain only bounded scheme, host, port, and path; fragments and query values are removed. Authored Source Config remains governed by its own schema.
- Accept only top-level DOM evidence. Cross-origin frames, closed shadow roots, and unsupported frame/shadow paths produce explicit Diagnostics.
- Enforce the canonical engine CSS grammar and recheck candidate uniqueness in the current page. Managed-Chrome validation remains mandatory because guest code and the remote page share one JavaScript environment and system-Webview/Chrome drift is expected.

## Structured Diagnostics

Add `infrastructure` to the canonical Structured Diagnostic categories. It covers native host availability, Webview creation, ephemeral-storage guarantees, bounds, callback, heartbeat, and cleanup failures. `runtime` remains reserved for productive Source execution.

Interactive-host codes use the `interactive_source_*` namespace, stable severity, an empty or domain-relevant JSON Pointer, and bounded typed details. URL details are sanitized. Unsupported platforms and CPU architectures are represented as Diagnostics, not generic strings. The isolated host proof may expose a private typed host error before integration; issue #348 owns its one-way mapping to the canonical category added by #344. This keeps #344 and #345 independently executable.

## Repair Draft persistence and recovery

### Storage

`sources::repair` stores one atomically replaced, versioned JSON envelope per concrete Source under the app-data directory. It uses the existing `sources` persistence and mutation ownership rather than SQLite or a generic repository abstraction.

Limits are:

- 4,096 Repair Draft/receipt documents;
- one active Draft or receipt per concrete Source;
- 256 KiB per complete serialized envelope;
- 64 MiB aggregate;
- 64 Repair Steps;
- eight selector candidates per Element Evidence;
- sixteen safe attributes per Element Evidence;
- 512 Unicode characters per text preview;
- 100 Diagnostics per document and 16,384 aggregate.

The staged candidate, complete Check Report, apply journal, and terminal receipt count toward the same 256 KiB envelope. Staging that would exceed a limit fails atomically and preserves the prior editable Draft. Corrupt or oversized documents are quarantined from productive use and surfaced through bounded Diagnostics.

A Source/Profile generation change marks an editable or staged Draft stale; no automatic rebase occurs. Closing a session, cancellation, child failure, or shutdown preserves the latest valid Draft.

### Write-ahead application

Confirmation uses a replay-safe state machine:

```text
editable -> staged -> applying -> applied_report_pending -> applied
                    \-> stale
editable|staged -> stale|discarded
```

1. Atomically transition `staged` to `applying`, retaining the proposal fingerprint, Draft revision, exact base Source/Profile generation, exact candidate document fingerprint, candidate Source, and checked report.
2. Under the existing installed-Source mutation coordinator:
   - if the full base generation matches, atomically replace the Source while preserving Source Status;
   - if the current authored Source document already matches the candidate fingerprint, recover the prior replacement;
   - otherwise transition to stale without writing.
3. Atomically transition to `applied_report_pending`.
4. Persist the derived Check Report separately.
5. Atomically transition to the compact `applied` receipt.

A crash at every boundary is replayable. The operation does not claim a transaction across Draft, Source, and report. A later repair may replace an applied receipt; explicit discard removes a non-applying Draft or receipt according to the typed contract.

No synchronous lock is held across compilation, HTTP, or browser awaits. Stage from one immutable installed Snapshot, then compare Draft revision and Source/Profile generation again before application.

## Candidate Source Live Check

Refactor the current installed-key-only Live Check implementation so one private prepared-input path serves both:

- existing installed Source `run`/activation wrappers; and
- unpersisted repair candidates.

Candidate checking compiles once against one admitted Profile snapshot, runs complete Discovery and optional one-candidate Detail through the existing HTTP and managed-Chrome adapters, and returns a complete report without writing a temporary Source or Check Report. The repair module owns the public candidate workflow; compiler products remain private.

A system-Webview selection that fails managed-Chrome validation returns the user to the affected Repair Step. Acceptance is never weakened to accommodate engine drift.

## Frontend structure and accessibility

Use separate transport modules for Interactive Source Session and Source repair. Each receives `unknown`, validates closed unions at runtime, and can be replaced with a fake in feature tests. Do not widen ordinary installed-Source inventory with generations or compiler material.

The feature owns a pure discriminated state model, one orchestration hook, and focused rendering modules. Do not copy the prototype's monolithic state or stylesheet.

The workspace provides explicit **Focus page** and **Return to repair controls** actions. F6 may cycle regions only after packaged proof. The guest owns Escape and picker keyboard interaction while focused. Terminal outcomes return focus to the relevant trusted control. A trusted `aria-live` region mirrors current instructions and status; guest visuals are never the only explanation.

## Validation surfaces

### Automated

- `source-engine` contracts for canonical selector validation and the `infrastructure` Diagnostic schema.
- External `sources` repair tests for state transitions, deterministic Step projection, evidence and persistence limits, stale generations, candidate checks, write ordering, failure injection, report-pending recovery, and confirmation replay.
- Desktop owner tests with a fake child adapter for navigation ordering, bounds validation, visibility, heartbeat, payload admission, URL redaction, and exactly-once teardown.
- Deterministic guest-script tests against local DOM fixtures for selector ranking, safe attributes, preview bounds, pointer/keyboard behavior, Escape, top-frame guards, and cleanup.
- TypeScript decoder tests and React behavior tests for snapshot recovery, sequence filtering, cancellation, revalidation, review, report-pending recovery, route cleanup, focus intent, and manual/Diagnostic entry.
- Security assertions for trusted-Webview-only capability targeting and absence of guest permissions.

Every implementation issue follows focused checks, `just quick`, and the full `just verify` handoff gate.

### Packaged application

PR CI continues to run deterministic contracts and build/upload macOS, Windows, and Linux bundles. Bundle creation is not behavioral proof.

Platform enablement requires a dated packaged-app run against deterministic loopback fixtures covering:

- child creation, split/resize/scale/minimize/restore bounds, hide/show, and no invisible input interception;
- URL editing, redirects, same-document history, origin changes, popup policy, denied schemes, and denied downloads;
- picker preview, retry, Escape, navigation cancellation, stale callback rejection, heartbeat failure, and cleanup;
- route exit, Source change, child failure, app shutdown/restart, Draft resume, and absence of browser-state persistence;
- candidate compile/full managed-Chrome check, mismatch return-to-step, review, apply, and report-pending recovery;
- keyboard/focus/screen-reader behavior across the trusted and native Webviews.

Required host modes are macOS/WKWebView, Windows/WebView2, Linux/WebKitGTK on X11, and Linux/WebKitGTK on Wayland. Initial native evidence may be recorded manually until faithful dedicated GUI automation exists.

Enablement is per target. The initial supported CPU targets follow the managed Browser Runtime: macOS arm64/x64, Windows x64, and Linux x64. Other targets return an `infrastructure` Diagnostic. Signing, notarization, and release distribution are outside this specification.

## Migration and removal

- Do not recreate `SourceOnboarding`; replace that wording with integration through `sources::{installed, detection, live_check, repair}`.
- Deepen the existing installed-Source mutation coordinator; do not construct a second `Store` or Source writer.
- Replace Snapshot-coupled Live Check internals with one prepared-input implementation; do not retain two complete-check algorithms.
- Generalize existing atomic persistence inside `sources`; do not add repair-specific atomic-write code.
- Keep ordinary installed Source transport free of generation, compiler outcome, plan, and Effective Source Profile data.
- Narrow Tauri capabilities before remote navigation.
- Keep `ManagedBrowserAcquisition` unchanged as the productive adapter; do not add a second productive browser route.
- Do not merge or copy the throwaway prototype implementation. Remove spike/debug routes, temporary host models, and duplicate fixtures when production integration lands.
- Residue checks must find no profile-specific repair mapping, guest app capability, generic JSON patch, temporary Source document, shared browser lifecycle, or prototype code in production.

## Delivery order

Seven ordinary implementation issues execute this specification:

1. [#344 — Add generic repair Diagnostics and canonical CSS selector validation](https://github.com/timjonaswechler/job-radar2/issues/344);
2. [#346 — Implement the Guided Source Repair protocol and bounded Draft persistence](https://github.com/timjonaswechler/job-radar2/issues/346);
3. [#347 — Add candidate Source Live Check and replay-safe repair application](https://github.com/timjonaswechler/job-radar2/issues/347);
4. [#345 — Prove and harden the Tauri child Webview host for Interactive Source Sessions](https://github.com/timjonaswechler/job-radar2/issues/345);
5. [#348 — Implement the Element Picker and typed Interactive Source Session integration](https://github.com/timjonaswechler/job-radar2/issues/348);
6. [#349 — Deliver the production Guided Source Repair workspace](https://github.com/timjonaswechler/job-radar2/issues/349);
7. [#350 — Enable Interactive Source Sessions through packaged platform gates](https://github.com/timjonaswechler/job-radar2/issues/350).

Issues #344 and #345 are the initial parallel frontier. Issue #345 proves private typed host failures without depending on the canonical Diagnostic addition; #348 performs that mapping after both lines converge. The repair protocol depends on #344; candidate application depends on the repair protocol; picker integration depends on the repair protocol and host; the UI depends on candidate application and picker integration; packaged enablement depends on the complete UI flow.
