# Contributing to Job Radar

## Start here

1. Read [`README.md`](README.md) for setup and repository orientation.
2. Use the central [`documentation portal`](docs/index.md) and its [`Development`](docs/development/README.md) entry point.
3. Read [`CONTEXT.md`](CONTEXT.md) before changing domain terminology.
4. Read [`AGENTS.md`](AGENTS.md) for repository-specific engineering rules.

GitHub Issues are the authoritative request and planning surface. Issue workflow and triage configuration are maintained under `docs/development/agents/` for the installed engineering skills.

## Validation

Use the repository's [`quick → focused → full`](docs/development/validation.md) loop. Run quick checks and the narrowest relevant target while developing, then run the complete handoff gate once before handing work over:

```bash
just quick
just rust-test bundled_source_profiles workday:: # example focused target
just verify                                   # required before handoff
```

Quick and focused checks are not substitutes for `just verify` or CI. The Search Run smoke is manual and network-dependent:

```bash
npm run smoke:search-run -- --app-data-dir "/path/to/app-data"
```

See [`docs/development/search-run-smoke.md`](docs/development/search-run-smoke.md) before running it.

## Repository hygiene

- Keep Rust integration tests and their fixtures under `src-tauri/tests/`. Dependency-free executable script self-tests may stay beside the script they verify.
- Keep shipped Source Profile data under `src-tauri/resources/profiles/`.
- Keep durable decisions in ADRs or PRDs rather than issue-completion reports.
- Do not commit credentials, provider payloads, local app data, generated reports, or temporary handoff artifacts.
