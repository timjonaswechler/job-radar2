# Source Live Checks as operational confidence

Job Radar uses concrete Source Live Checks as the user-facing confidence signal for source operability.

The product rule is:

> **Source Profiles describe reusable access behavior. Concrete Sources are live checked.**

A Source Profile is declarative reusable access behavior: detection hints, Source Config schema, Access Paths, DSL strategies, support summary, known issues, and validation diagnostics.

A Source is the concrete configured endpoint the user wants to use. Its current operability is represented by the latest Source Live Check Report for that Source.

## Consequences

- The user-facing check action belongs to concrete Sources.
- Source details expose `Prüfen`, `Prüfen & Aktivieren`, and `Prüfen & Reaktivieren` live-check flows.
- Source Live Check Reports are overwriteable derived reports with freshness/staleness detection.
- Profile details show profile metadata, Access Paths, support notes, and validation diagnostics.
- Production-agent guidance creates at most a Source Profile JSON and optional Source JSON, then uses Source Live Check for operational confidence.
- Support levels use non-operational language: `stable`, `best_effort`, `experimental`, and `unsupported`.

## Source Onboarding amendment

Source setup and lifecycle admission are owned by one Tauri-free `SourceOnboarding` module. Its small interface covers Detection, authored Source changes, report freshness, status-neutral Source Live Checks, and check-and-activate.

Authored creation and definition-revision inputs do not contain arbitrary `SourceStatus` or derived diagnostics. Creation always persists `draft`; revision preserves the current status. Explicit inactive changes can select only `draft` or `disabled`. The module selects activation versus reactivation from the persisted status, rejects an already active Source before external work, always runs a new complete Source Live Check for admission, persists the exact checked fingerprints, and changes status only after the report is durable. Tauri Commands are caller-side transport adapters rather than an alternative orchestration route.

The module remains internal to the desktop crate for now. Its interface is Tauri-free so moving it into a host-independent application crate later is mechanical if a second host or stable adjacent ownership seams justify that crate.
