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
