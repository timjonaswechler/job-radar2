# Development validation loop

The repository-level `Justfile` provides one local interface for frontend and Rust validation. It delegates to the canonical npm and Cargo commands; it does not duplicate their configuration.

## Quick → focused → full

Use three levels while implementing:

1. **Quick:** run `just quick` after small changes. It typechecks application/configuration and frontend test code, runs the hermetic Vitest suite once, and runs `cargo check --tests`. It does not create a Vite or Tauri bundle and does not link or execute every Rust test program.
2. **Focused:** run the narrowest relevant frontend or Rust target while iterating. These commands complement `quick`; they do not replace the full gate.
3. **Full:** run `just verify` once before handing work over. This executes the complete frontend check (including its production bundle), validates internal Markdown links, checks Rust formatting, runs the complete Cargo test suite, and checks the Git diff for whitespace errors.

`just verify` is the required local handoff gate even when all quick and focused checks passed. The CI workflow remains authoritative for credential safeguards and cross-platform desktop packaging.

## Commands

```bash
just quick

# Focus one frontend file/name through Vitest.
just frontend-test settings

# Focus one visible Cargo integration-test target, then optionally one test name.
just rust-test profile_dsl_profiles
just rust-test profile_dsl_profiles successfactors::

# Pass Cargo test-harness arguments after `--`.
just rust-test profile_dsl_profiles successfactors:: -- --nocapture

# Focus library unit tests.
just rust-unit agent::auth::tests

# Required before handoff.
just verify

# Package only after validation when a desktop artifact is needed.
just package
```

`rust-test` intentionally requires the integration-test target name shown by Cargo, corresponding to a target under `src-tauri/tests/`; it never falls back to the complete Cargo suite. Optional filters and arguments are passed to Cargo unchanged. `rust-unit` is the explicit library-unit-test path. Run `just --list` for validation, database, migration, app-data, and maintenance recipes.

The underlying recipes are:

| Recipe | Delegated command(s) |
|---|---|
| `frontend-check` | `npm run typecheck`, `npm run typecheck:test` |
| `frontend-test [args]` | `npm run test:frontend -- [args]` |
| `rust-check` | `cargo check --manifest-path src-tauri/Cargo.toml --tests` |
| `rust-test <target> [args]` | `cargo test --manifest-path src-tauri/Cargo.toml --test <target> [args]` |
| `rust-unit [args]` | `cargo test --manifest-path src-tauri/Cargo.toml --lib [args]` |
| `docs-check` | dependency-free internal Markdown destination and heading-anchor validation |
| `verify` | frontend full check, Markdown links, `cargo fmt --check`, full `cargo test`, `git diff --check` |
| `package [args]` | `npm run tauri -- build [args]` |

## Platform notes

The Justfile development interface is supported locally on macOS and requires `just`, Bash, Node.js/npm, and the stable Rust toolchain. The delegated npm and Cargo commands remain usable directly on other CI-supported systems. `just package` builds only for the current host and requires that host's native Tauri dependencies; see [Platform builds](../../README.md#plattform-builds), especially before packaging on Linux. Database and app-data recipes support macOS, Linux, and Windows path conventions, but running the Justfile itself on Windows requires a Bash environment.
