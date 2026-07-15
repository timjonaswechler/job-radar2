# Agent OAuth credential storage

The internal Rust agent module follows the credential-storage behavior pinned in [`docs/research/pi-rust-agent-baseline.md`](../research/pi-rust-agent-baseline.md), narrowed to stored OAuth credentials on macOS.

## Security contract

- Production storage derives the effective user's home directory from the macOS account database, not from environment variables, and uses the application-data tree under `Library/Application Support/de.timjonaswechler.jobradar/agent`.
- Construction fails closed for relative paths, paths below a Git repository, non-regular files, and symlinks. There is no repository-relative, environment-credential, API-key, or alternate-path fallback.
- The credential directory is enforced as `0700`; credential, lock, and temporary files are enforced as `0600` and opened without following symlinks.
- `auth.json` is replaced atomically while an inter-process file lock is held. Save, remove, and expired-token refresh re-read the latest document under that lock so independent processes cannot overwrite another provider's update.
- Expired-token refresh is serialized, the rotated credential is persisted before it is returned, and refresh failure preserves the prior credential for a later retry.
- Status is value-free (`Configured` or `NotConfigured`) and never refreshes. Credential types intentionally implement neither `Debug` nor `Display`.
- Storage and refresh failures map to fixed diagnostics with no source error, path, serialized document, or credential value.

## Verification seam

Tests exercise the same `AuthStorage` interface that the authentication/provider implementation will call. A temporary application-data root is the filesystem adapter; it is always outside the repository and all credential-like values are visibly synthetic. The tests cover lifecycle operations, permissions, concurrent refresh, non-blocking async lock waiting, refresh failure preservation/redaction, malformed JSON, symlinks, relative paths, and repository paths.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent::auth::tests
```
