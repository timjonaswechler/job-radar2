# Simplify checks around concrete Source Live Checks

Job Radar will not expose Source Profile Verification or Fixture Packs as a user-facing product workflow.

The product rule is now:

> **Source Profiles describe reusable access behavior. Concrete Sources are live checked.**

The previous fixture-backed Profile Verification direction was useful as a regression-testing idea, but it made normal custom Source/Profile creation feel too heavy and created a misleading confidence signal. Offline fixture replay can pass while the real production source has changed, is empty, localized differently, blocked, rate-limited, or has different detail behavior.

## Consequences

- The user-facing check action belongs to concrete Sources, not abstract Source Profiles.
- Source details keep `Prüfen`, `Prüfen & Aktivieren`, and `Prüfen & Reaktivieren` live-check flows.
- Source Live Check Reports remain overwriteable derived reports with freshness/staleness detection.
- Profile details show profile metadata, Access Paths, support notes, and validation diagnostics, but no Profile Verification action/report.
- Fixture Packs and Fixture Manifests are not app-data artifacts and are not required for production custom profiles.
- Production-agent guidance should create at most a Source Profile JSON and optional Source JSON, then use Source Live Check for operational confidence.
- Support levels use non-verification language: `stable`, `best_effort`, `experimental`, and `unsupported`.
