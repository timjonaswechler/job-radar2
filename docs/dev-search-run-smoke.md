# SCHOTT + StepStone search-run smoke

This is a manual backend-only development smoke for issue #19. It is network-dependent and is not part of the default deterministic test suite.

The smoke path creates or reuses an active Suchanfrage with:

- include rules: `Physik`, `Laser`
- Ausschlussregeln: `Praktikum`, `Werkstudent`, `Schülerpraktikum`
- location: `Mainz`
- radius: `30`
- sources: `schott_careers`, `stepstone_de`

Running it overwrites `search-run-result.json` in the repository root.

## Command

Use the app data directory that contains the local development `job_radar.db`:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data"
```

You can also set the directory through an environment variable:

```bash
JOB_RADAR_SMOKE_APP_DATA_DIR="/path/to/app-data" npm run smoke:search-run
```

For a fresh development database, allow the smoke command to create the local SCHOTT smoke Quelle if it is missing:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data" --ensure-schott-source
```

`stepstone_de` is seeded by the backend. `--ensure-schott-source` creates only the local development source `schott_careers` with adapter `declarative_sitemap_inventory` and source config:

```json
{
  "url": "https://join.schott.com/sitemap.xml",
  "recursive": false
}
```

## Expected validation

- SCHOTT should complete when the sitemap is reachable and produce normalized postings with readable titles, URL, source reference, company derived from `SCHOTT Karriere`, and `Mainz` where the URL contains that location.
- StepStone success is acceptable when available.
- StepStone browser/HTTP failure is acceptable only when `sourceRuns` contains an explicit `stepstone_de` error and the overall status is `completed_with_errors` if SCHOTT completed.
- Final `postings` should not contain titles with the configured exclusion terms (`Praktikum`, `Werkstudent`, `Schülerpraktikum`).

Do not add this command to CI or default test scripts; live SCHOTT/StepStone availability is intentionally human-in-the-loop validation only.
