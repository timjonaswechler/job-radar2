# SCHOTT search-run smoke

This is a manual backend-only development smoke for the current Source/Profile DSL execution path. It is network-dependent and is not part of the default deterministic test suite.

The smoke path creates or reuses an active Search Request with:

- include rules: `Physik`, `Laser`
- exclusion rules: `Praktik(um|ant)`, `Werkstudent`, `Student`, `Masterthesis`, `Ausbildung`
- location: `Mainz`
- radius: `30`
- source: `schott_ag`

After the atomic SQLite transaction commits, it overwrites the non-authoritative `search-run-result.json` summary in the repository root.

## Command

Use the app data directory that contains the local development `job_radar.db`:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data"
```

You can also set the directory through an environment variable:

```bash
JOB_RADAR_SMOKE_APP_DATA_DIR="/path/to/app-data" npm run smoke:search-run
```

By default the smoke targets the development SCHOTT smoke Source key `schott_ag`. To run the same smoke Search Request against existing local Sources, pass one or more Source keys:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data" --source-key schott --source-key openai
```

The smoke writes one bounded artifact in the repository root:

- `search-run-result.json` — terminal facts, finalized merged-posting count, and committed per-Source Resolution completion/count/usage/remainder summaries. Candidate Diagnostic samples use the backend cap of 10.

It never writes raw Candidates, provider payloads, hints, or postingMeta. SQLite is authoritative: artifact failure after commit does not roll back the Search Run, Matches, or postings, and transaction failure produces no new authoritative artifact.

Selected draft Sources are normally skipped, matching normal Search Run behavior. For local smoke validation you can execute draft Sources without changing their persisted Source Status:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data" --source-key schott --allow-draft
```

For a fresh development database, allow the smoke command to create the local SCHOTT smoke Source if it is missing:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data" --ensure-schott-source
```

`--ensure-schott-source` writes only the local development Source document `sources/schott_ag.json`. The Source selects Source Profile `successfactors`, Access Path `rmk_sitemap_html`, and Source Config:

```json
{
  "baseUrl": "https://join.schott.com",
  "sitemapUrl": "https://join.schott.com/sitemap.xml"
}
```

## Current execution flow

- The Source Profile registry loads built-in Source Profiles and the local `schott_ag` Source document.
- Source validation derives `validationState` from schema, registry, and Profile Compiler diagnostics; Source status remains the user-controlled `active` lifecycle state.
- At Search Run start, the selected Source Profile Access Path and Source Config compile into a typed Execution Plan.
- The Search Run calls Q01 Candidate Resolution exactly once for the executed Source. Q01 executes the compiled policy-bearing Discovery and lazy Detail plans, normalizes provider values, and performs final Search Request matching (including the prepared 30 km Geo filter) before releasing committed finalized values.
- Discovery and Detail share the cumulative 64 MiB Browser-rendered-byte ceiling while HTTP response bytes remain a separate allowance dimension. Search Run does not invent a tighter product allowance or reconstruct the phase report.
- Browser Runtime unavailability is reported as a typed acquisition failure only if a Browser Strategy actually executes. Runtime installation, status, and uninstall remain independent administration operations.
- Only committed finalized values enter the existing cross-Source merger and the one atomic Search Run/Match persistence call. Source abort and Cancellation release no Resolution payload. The bounded artifact is written only after commit.

## Expected validation

- SCHOTT should complete when the sitemap is reachable and provider values supply trustworthy title, company, and location evidence. A readable URL segment or URL-derived `Mainz` is never canonical evidence; authorized hints may reject only.
- Final `postings` should not contain titles matching the configured exclusion rules.
- Verify one durable `search_runs` row, one `matches` row per final merged posting, and corresponding durable posting/source rows. The summary should expose exact committed Q01 counts, usage, remainder, and at most 10 samples.
- The overall status should be `completed` when SCHOTT succeeds. Live-source failure remains visible on the `schott_ag` Source Run and may produce `completed_with_errors` when other Sources commit.

Do not add this command to CI or default test scripts; live SCHOTT availability is intentionally human-in-the-loop validation only.
