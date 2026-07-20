# SCHOTT search-run smoke

This is a manual backend-only development smoke for the current Source/Profile DSL execution path. It is network-dependent and is not part of the default deterministic test suite.

The smoke path creates or reuses an active Search Request with:

- include rules: `Physik`, `Laser`
- exclusion rules: `Praktik(um|ant)`, `Werkstudent`, `Student`, `Masterthesis`, `Ausbildung`
- location: `Mainz`
- radius: `30`
- source: `schott_ag`

Running it overwrites `search-run-result.json` and `search-run-candidates.json` in the repository root.

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

The smoke writes two artifacts in the repository root:

- `search-run-result.json` — final matched postings after Search Request filters.
- `search-run-candidates.json` — raw discovered candidates per executed Source before matching/exclusion filters.

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
- The Search Run executes the compiled policy-bearing `discovery` plan for `schott_ag` and then applies Search Request match and exclusion rules locally.
- Source Run diagnostics remain structured and source-scoped so one Source failure does not hide other Source outcomes.

## Expected validation

- SCHOTT should complete when the sitemap is reachable and produce normalized postings with readable titles, URL, source reference, company derived from `SCHOTT AG`, and `Mainz` where the URL contains that location.
- Final `postings` should not contain titles matching the configured exclusion rules.
- The overall status should be `completed` when SCHOTT succeeds. If SCHOTT fails because the live sitemap is unavailable or changed, the failure should be visible on the `schott_ag` Source Run with structured diagnostics.

Do not add this command to CI or default test scripts; live SCHOTT availability is intentionally human-in-the-loop validation only.
