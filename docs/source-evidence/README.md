# Source Evidence Catalog

This catalog records evidence gathered while auditing reusable Source Profiles and concrete Sources.

It supports:

- Source Profile robustness audits in GitHub issue #33;
- real location-format evidence for GitHub issue #57;
- selection of concrete Sources for bounded Source Live Checks;
- separation of documented, synthetic, observed, and live-checked evidence.

The catalog is evidence documentation. It is not a Source registry, does not make a public endpoint a Built-in Source, and does not assign operational status to a Source Profile.

## Evidence classes

| Evidence class | What it proves | What it does not prove |
|---|---|---|
| Official vendor documentation | Documented API shape and vendor-provided examples | Current behavior of a concrete Source |
| Deterministic repository fixture | The Profile DSL and runtime handle the committed input as tested | That a provider currently emits the fixture values |
| Public endpoint observation | A concrete endpoint emitted the recorded raw value at the stated time | That Job Radar successfully compiled and executed the corresponding Source |
| Source Live Check | A concrete Source compiled and passed or failed a bounded live smoke | General operability of every Source using the same Source Profile |

A direct HTTP request is a public endpoint observation, not a Source Live Check. A stale Source Live Check Report retains its historical result but is not current operational evidence.

## Catalog

| Source Profile | Evidence page | Current state |
|---|---|---|
| `greenhouse` | [greenhouse.md](greenhouse.md) | Official docs, synthetic fixtures, five public endpoint observations, and three fresh passing temporary Source Live Checks recorded |
| `workday` | [workday.md](workday.md) | Synthetic fixtures, five public endpoint observations, observed 20-item CXS pagination, and three fresh passing bounded-smoke Source Live Checks recorded |
| `successfactors` | [successfactors.md](successfactors.md) | Official SAP references, deterministic fixtures, four public endpoint observations, and four fresh temporary Source Live Checks recorded; two passed and two failed |

Create additional pages only when there is evidence to record.

## Evidence record template

Each concrete observation should contain:

```md
### <Source name or evidence subject>

- Source Profile: `<profile-key>`
- Evidence class: official docs | deterministic fixture | public endpoint observation | Source Live Check
- Entry URL: <public URL or not applicable>
- Source Config: <public, non-secret fields needed to identify the Source>
- Checked at: <ISO date or timestamp>
- Source Live Check result: passed | failed | not applicable
- Source Live Check report state: fresh | stale | unknown | not applicable
- Detail checked: yes | no | not applicable
- Evidence reference: <URL, repository path, Check Report reference, or commit>

| Raw provider value | Current normalized output | Provenance | Notes |
|---|---|---|---|
| `<raw location>` | `["<current output>"]` | official docs / fixture / endpoint / live check | limitations |
```

For a Source Live Check, record the concrete Source key, check date, persisted result, and derived freshness state separately.

## Recording rules

- Prefer official vendor documentation and vendor demo boards before arbitrary company career pages.
- Record the exact evidence class and check date for every claim.
- Keep raw provider values separate from current normalized output and desired future semantics.
- Mark synthetic or anonymized fixtures explicitly; do not present them as provider observations.
- Do not commit complete API responses merely for catalog documentation. Add a minimal sanitized fixture only when a regression test needs it.
- Do not record secrets, authentication material, personal data beyond transient public job fields, or private app-data paths.
- Public board identifiers such as a Greenhouse `boardSlug` may be recorded when they are already exposed by the public endpoint.
- Do not silently overwrite old observations. Mark them stale or add a newer dated observation when behavior changes.
- Link actionable profile defects to a focused follow-up issue. Feed browser-dependent findings into #44 and location evidence into #57.
