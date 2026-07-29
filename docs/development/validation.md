# Development validation loop

The repository-level `Justfile` provides one local interface for frontend and Rust validation. It delegates to the canonical npm and Cargo commands; it does not duplicate their configuration.

## Quick → focused → full

Use three levels while implementing:

1. **Quick:** run `just quick` after small changes. It typechecks application/configuration and frontend test code, runs the hermetic Vitest suite once, and runs `cargo check --workspace --tests`. It does not create a Vite or Tauri bundle and does not link or execute every Rust test program.
2. **Focused:** run the narrowest relevant frontend or Rust target while iterating. These commands complement `quick`; they do not replace the full gate.
3. **Full:** run `just verify` once before handing work over. This executes the complete frontend check (including its production bundle), validates internal Markdown links, checks Rust formatting, runs the complete Cargo test suite, and checks the Git diff for whitespace errors.

`just verify` is the required local handoff gate even when all quick and focused checks passed. The CI workflow remains authoritative for credential safeguards and cross-platform desktop packaging.

## Commands

```bash
just quick

# Focus one frontend file/name through Vitest.
just frontend-test settings

# Focus one visible Cargo integration-test target, then optionally one test name.
just rust-test bundled_source_profiles
just rust-test bundled_source_profiles successfactors::

# Pass Cargo test-harness arguments after `--`.
just rust-test bundled_source_profiles successfactors:: -- --nocapture

# Focus an integration target owned by a workspace crate.
just rust-crate-test agent contracts model_registry::

# Focus desktop-package library tests.
just rust-unit search::run::tests

# Required before handoff.
just verify

# Package only after validation when a desktop artifact is needed.
just package
```

`rust-test` intentionally requires a desktop-package integration target under `src-tauri/tests/`; `rust-crate-test` requires both the owning workspace package and one of its integration targets. Neither falls back to the complete Cargo suite. Optional filters and arguments are passed to Cargo unchanged. `rust-unit` is the explicit desktop-package library-unit-test path. Run `just --list` for validation, database, migration, app-data, and maintenance recipes.

The underlying recipes are:

| Recipe | Delegated command(s) |
|---|---|
| `frontend-check` | `npm run typecheck`, `npm run typecheck:test` |
| `frontend-test [args]` | `npm run test:frontend -- [args]` |
| `rust-check` | `cargo check --manifest-path src-tauri/Cargo.toml --workspace --tests` |
| `rust-test <target> [args]` | `cargo test --manifest-path src-tauri/Cargo.toml --package job-radar --test <target> [args]` |
| `rust-crate-test <package> <target> [args]` | `cargo test --manifest-path src-tauri/Cargo.toml --package <package> --test <target> [args]` |
| `rust-unit [args]` | `cargo test --manifest-path src-tauri/Cargo.toml --lib [args]` |
| `docs-check` | dependency-free internal Markdown destination and heading-anchor validation |
| `verify` | frontend full check, Markdown links, `cargo fmt --check`, `cargo test --workspace`, `git diff --check` |
| `package [args]` | `npm run tauri -- build [args]` |

## Rust test ownership

Test location follows the interface under test:

- `src-tauri/crates/<crate>/tests/` contains black-box contracts owned by that crate.
- `src-tauri/crates/<crate>/src/contract_tests/` contains broad crate contracts that require test-only internal seams; narrow private-helper tests stay beside their implementation under `#[cfg(test)]`.
- `src-tauri/tests/` contains only tests owned by the desktop package. One `desktop` target contains logically separated Agent, Browser, Geo, HTTP, and Source Profile DSL modules; `source_application` exercises Source application behavior, and `bundled_source_profiles` verifies shipped product resources.
- Generic Source Profile DSL tests belong under `src-tauri/crates/source-profile-dsl/tests/`, grouped into the four Cargo targets `compiler`, `detection`, `primitives`, and `runtime`. Leaf test files live in matching subdirectories.
- External Agent black-box contracts share the `contracts` target and remain separated into authentication-storage and model-registry modules.
- Shared deterministic payloads remain under the owning package's `tests/fixtures/` directory.

Cargo target names communicate a stable test surface, while module filters provide detail. Prefer a small number of ownership-oriented targets such as `desktop`, `contracts`, `compiler`, or `runtime`; avoid one Cargo target per leaf test file.

## Platform notes

The Justfile development interface is supported locally on macOS and requires `just`, Bash, Node.js/npm, and the stable Rust toolchain. The delegated npm and Cargo commands remain usable directly on other CI-supported systems. `just package` builds only for the current host and requires that host's native Tauri dependencies; see [Platform builds](../../README.md#plattform-builds), especially before packaging on Linux. Database and app-data recipes support macOS, Linux, and Windows path conventions, but running the Justfile itself on Windows requires a Bash environment.
