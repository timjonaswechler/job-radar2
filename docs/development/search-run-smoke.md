# Generic search-run smoke

This is a manual backend-only development smoke for the current Source Behavior Language execution path. It is network-dependent and is not part of the deterministic test suite.

The smoke requires one or more existing local Sources. It creates or reuses an active Search Request with a generic title regex (`.+`), no exclusions, and no location filter, then runs the selected Sources through the production Search Run path.

After the atomic SQLite transaction commits, it overwrites the non-authoritative `search-run-result.json` summary in the repository root.

## Command

Use the app data directory that contains the local development `job_radar.db` and pass every Source explicitly:

```bash
npm run smoke:search-run -- \
  --app-data-dir "/path/to/app-data" \
  --source-key fixture_source
```

You can set the directory through an environment variable and select multiple Sources:

```bash
JOB_RADAR_SMOKE_APP_DATA_DIR="/path/to/app-data" \
  npm run smoke:search-run -- \
  --source-key fixture_source_one \
  --source-key fixture_source_two
```

The command does not create, guess, or embed a concrete Source. Omitting `--source-key` is an error.

Selected draft Sources are normally skipped, matching normal Search Run behavior. For local smoke validation you can execute them without changing their persisted Source Status:

```bash
npm run smoke:search-run -- \
  --app-data-dir "/path/to/app-data" \
  --source-key fixture_source \
  --allow-draft
```

## Artifact

The smoke writes one bounded artifact in the repository root:

- `search-run-result.json` — terminal facts, finalized merged-posting count, and committed per-Source Resolution completion/count/usage/remainder summaries. Candidate Diagnostic samples use the backend cap of 10.

It never writes raw Candidates, provider payloads, hints, or postingMeta. SQLite is authoritative: artifact failure after commit does not roll back the Search Run, Matches, or postings, and transaction failure produces no new authoritative artifact.

## Current execution flow

- The application loads built-in Source Profiles and the explicitly selected local Source documents.
- Source validation derives `validationState` from schema, registry, and Profile Compiler diagnostics; Source status remains the user-controlled lifecycle state.
- At Search Run start, each selected Access Path and Source Config compiles into a typed Execution Plan.
- The Search Run calls Candidate Resolution exactly once per executed Source. Candidate Resolution executes the compiled Discovery and lazy Detail plans, normalizes provider values, applies the generic smoke Search Request, and releases only committed finalized values.
- Discovery and Detail share the cumulative 64 MiB Browser-rendered-byte ceiling while HTTP response bytes remain a separate allowance dimension.
- Browser Runtime unavailability is reported only if a Browser Strategy actually executes.
- Only committed finalized values enter cross-Source merging and atomic Search Run/Match persistence.

## Expected validation

- Every explicitly selected Source has a visible Source Run outcome.
- The overall status is `completed` when all selected Sources succeed, `completed_with_errors` for partial failure, or `failed` when no Source completes successfully.
- Verify one durable `search_runs` row, one `matches` row per final merged posting, and corresponding durable posting/source rows.
- The summary exposes exact committed Resolution counts, usage, remainder, and at most 10 Diagnostic samples.

Do not add this command to CI or default test scripts; live Source availability is intentionally human-in-the-loop validation only.
