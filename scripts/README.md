# Repository scripts

Scripts are grouped by the development task they own. Stable user-facing commands remain in `package.json` or the `Justfile`; callers should prefer those interfaces over invoking implementation files directly.

## Security checks

[`security/`](security/) contains the dependency-free Agent credential scanner and its self-test. Run them through the stable package commands:

```bash
npm run test:agent-credential-safety
npm run check:agent-credentials
```

CI intentionally runs both commands before `npm ci`, builds, or tests.

## Local database and migrations

[`database/`](database/) owns local SQLite app-data discovery, guarded database deletion, SQLx migration squashing, and migration rebaselining. Use the grouped Justfile recipes listed by `just --list`, including `app-data-dir`, `db-path`, `db-clear`, `migrations-squash`, and `db-rebaseline-migrations`.

The destructive `db-clear` recipe retains its interactive confirmation. `db-clear-force` remains the explicit non-interactive variant, and the existing `clear-db` and `squash-migrations` Just aliases remain supported. The scripts are development-only and must not be used as application migration logic.

## Generators

[`generators/`](generators/) contains reproducible generators for committed or bundled project resources. `just geo-seed` runs `generators/generate-seed.py` to rebuild the bundled geolocation seed from local GeoNames inputs.

## Repository checks

[`checks/`](checks/) contains non-security repository invariants. `primitive-residue.sh` is exposed through `just primitive-residue` and `just primitive-residue-emit` and is also exercised by Rust integration tests.

## Development utilities

[`development/`](development/) contains optional source-maintenance tools that are not routine validation commands. `rust-module-split.py` is owned by maintainers performing reviewed Rust module decomposition for the #166 implementation family and later oversized flat Rust modules. Its repeatable use case is to list top-level Rust items, produce and manually edit a JSON split plan, dry-run exact item placement, and only then write reviewed module files. It remains at its current path so active or follow-up module-split work does not lose its entry point.

Frontend tests run directly through the canonical Vitest commands in `package.json`; no frontend test wrapper, aggregator launcher, or compatibility alias remains under `scripts/`.

Add a new script to the narrowest owning directory. If developers or CI need to call it regularly, expose it through `package.json` or the `Justfile` and document that command instead of the implementation path.
