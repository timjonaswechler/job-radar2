# Provider-neutral agent authentication storage

The internal Rust agent authentication module ports the credential contract pinned in [`docs/research/pi-auth-model-registry-behavior.md`](../research/pi-auth-model-registry-behavior.md). It stores provider-keyed `api_key` and `oauth` entries under the portable `agents/` application-data root while deliberately strengthening Pi's persistence and reload guarantees.

## Credential contract

- `auth.json` is keyed by validated Provider ID. An `api_key` entry contains `type`, `key`, and optional provider-scoped `env`; an `oauth` entry contains `type`, `access`, `refresh`, exact millisecond `expires`, and preserved provider-owned metadata.
- API-key values are either non-empty direct values or exact whole-value `$ENV_VAR` references. Provider-scoped `env` values take precedence over injected ambient values. Missing or empty references fail closed and never fall through to another credential source.
- Leading `!command`, `${...}`, interpolation, and arbitrary command execution are rejected. The module has no command runner.
- Resolution order is an optional runtime override followed by the stored provider entry. A stored entry owns the result: an invalid reference or failed OAuth refresh is a redacted error, not permission to try another source. `models.json` fallback belongs to the separate model-registry module.
- Status and logout are generic and provider-scoped. Status is value-free. Secret-bearing stored and resolved types implement neither `Debug` nor `Display`.

## Reload and request snapshots

- Construction publishes one validated authentication snapshot. Manual edits become visible only after explicit `reload`; there is no filesystem watcher.
- Reload validates a complete candidate before publication. Invalid JSON or any invalid entry publishes an unavailable state for all authentication, so an older cached secret cannot be reused. A later valid reload restores the complete snapshot.
- Resolution clones the published provider entry before asynchronous work. Successful mutations and refreshes publish the exact complete document that was atomically persisted; later requests observe that snapshot.

## Storage and refresh guarantees

- Production storage derives the effective user's home directory from the macOS account database, not environment variables, and uses `Library/Application Support/de.timjonaswechler.jobradar/agents` through the application-data path abstraction.
- Construction fails closed for relative paths, paths below a Git repository, non-regular files, and symlinks. There is no repository-relative or alternate-path fallback.
- The credential directory is enforced as `0700`; credential, lock, and temporary files are enforced as `0600` and opened without following symlinks.
- Set, logout, and refresh acquire the inter-process lock, re-read and validate the latest complete document, change only one Provider entry, write a private temporary file, sync it, rename it atomically, verify protection, and sync the directory.
- OAuth is valid only while `now < expires`; equality is expired. Refresh is serialized across processes, double-checks the latest protected document after taking the lock, persists rotation before returning it, and preserves the prior credential on failure.
- Storage, reload, resolution, and refresh failures map to fixed diagnostics with no source error, path, serialized document, Provider metadata, environment value, or credential value.

The existing OpenAI Codex authentication module temporarily uses compatibility methods on this storage module. Moving that Provider onto the generic authentication and model-registry contracts is tracked separately.

## Verification seam

Tests exercise the same `AuthStorage` interface future Provider and configuration modules call. A temporary application-data root is the filesystem adapter; values are conspicuously synthetic and environment lookup is injected rather than reading developer credentials. Coverage includes both tagged variants, direct/reference resolution, no-fallback ownership, command rejection, transactional reload, provider-scoped mutation, exact-expiry concurrent refresh, atomic persistence, migration, permissions, symlinks, and redacted failures.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::auth::tests
cargo test --manifest-path src-tauri/Cargo.toml --test agent_data_root
npm run test:agent-credential-safety
```
