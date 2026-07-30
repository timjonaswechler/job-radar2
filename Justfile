set shell := ["bash", "-euo", "pipefail", "-c"]

alias clear-db := db-clear
alias squash-migrations := migrations-squash

# List available recipes.
default:
    @just --list

# Run the fast hermetic loop: frontend typechecks/tests and Rust checking without bundling or linking all tests.
[group('Development loops')]
quick: frontend-check frontend-test rust-check

# Typecheck frontend application, configuration, and test code without building a production bundle.
[group('Development loops')]
frontend-check:
    npm run typecheck
    npm run typecheck:test

# Run hermetic frontend tests once; optional Vitest filters and arguments are forwarded unchanged.
[group('Development loops')]
[positional-arguments]
frontend-test *args:
    npm run test:frontend -- "$@"

# Check all workspace Rust test targets without linking or running their test programs.
[group('Development loops')]
rust-check:
    cargo check --manifest-path src-tauri/Cargo.toml --workspace --tests

# Run one desktop-package integration-test target; optional Cargo test filters and arguments are forwarded unchanged.
[group('Development loops')]
[positional-arguments]
rust-test target *args:
    target="$1"; shift; cargo test --manifest-path src-tauri/Cargo.toml --package job-radar --test "$target" "$@"

# Run one integration-test target owned by a specific workspace package.
[group('Development loops')]
[positional-arguments]
rust-crate-test package target *args:
    package="$1"; target="$2"; shift 2; cargo test --manifest-path src-tauri/Cargo.toml --package "$package" --test "$target" "$@"

# Run Rust library unit tests; optional Cargo test filters and arguments are forwarded unchanged.
[group('Development loops')]
[positional-arguments]
rust-unit *args:
    cargo test --manifest-path src-tauri/Cargo.toml --lib "$@"

# Run the portable persistent Agent Chat proof through Rust, Tauri, and React public seams.
[group('Development loops')]
agent-chat-proof:
    cargo test --manifest-path src-tauri/Cargo.toml --package agent
    cargo test --manifest-path src-tauri/Cargo.toml --package job-radar --test desktop agent::chat_application::
    npm run test:frontend -- src/features/agent-chat/tests

# Move one synthetic session through production macOS storage, locking, and Trash adapters.
[group('Development loops')]
agent-chat-macos-smoke:
    cargo test --manifest-path src-tauri/Cargo.toml --package agent native_macos_storage_locking_and_trash_smoke -- --ignored --nocapture

# Run the complete handoff gate (frontend, documentation, Rust, and whitespace validation).
[group('Development loops')]
verify:
    npm run check:frontend
    npm run check:markdown-links
    cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
    cargo test --manifest-path src-tauri/Cargo.toml --workspace
    git diff --check

# Build the Tauri desktop package for the current platform; optional Tauri arguments are forwarded unchanged.
[group('Development loops')]
[positional-arguments]
package *args:
    npm run tauri -- build "$@"

# Print the line count of every tracked Rust source file and the total.
[group('Repository maintenance')]
loc extension:
    @git ls-files '*.{{ extension }}' | while IFS= read -r file; do wc -l "$file"; done | awk '{ total += $1; print } END { if (NR == 0) print "No Rust files found."; else printf "%7d total\n", total }'

# Print the Tauri app data directory used by the installed/dev app.
[group('Local app data and database')]
app-data-dir:
    @python3 scripts/database/tauri-app-data-dir.py

# Print the SQLite database path in the Tauri app data directory.
[group('Local app data and database')]
db-path:
    @printf "%s/job_radar.db\n" "$(python3 scripts/database/tauri-app-data-dir.py)"

# Delete app-data SQLite DB family; keeps custom source/profile JSON. Set YES=1 to skip the prompt.
[group('Local app data and database')]
db-clear:
    @bash scripts/database/clear-sqlite-db.sh

# Same as db-clear, without the interactive confirmation prompt.
[group('Local app data and database')]
db-clear-force:
    @YES=1 bash scripts/database/clear-sqlite-db.sh

# Squash all SQLx migrations into one current-schema migration.
[group('Local app data and database')]
migrations-squash target="src-tauri/migrations/20260609000000_current_schema.sql":
    @bash scripts/database/squash-migrations.sh "{{ target }}"

# Rewrite SQLx migration bookkeeping for the existing DB, preserving data. Refuses if schemas differ.
[group('Local app data and database')]
db-rebaseline-migrations:
    @python3 scripts/database/rebaseline-sqlx-migrations.py

# Squash migrations, then rebaseline SQLx bookkeeping without deleting table data. Dev-only.
[group('Local app data and database')]
db-preserve-after-squash: migrations-squash db-rebaseline-migrations
    @echo "Done. Existing DB data was preserved. Restart the app."

# Squash migrations, then clear the app-data DB so SQLx migration checksums cannot conflict.
[group('Local app data and database')]
db-reset-after-squash: migrations-squash db-clear-force
    @echo "Done. Start the app again to recreate the DB from the squashed migration."

# Validate internal links and heading anchors in tracked Markdown files.
[group('Repository maintenance')]
docs-check:
    npm run check:markdown-links

# Validate the reviewed Primitive residue manifest.
[group('Repository maintenance')]
primitive-residue:
    @bash scripts/checks/primitive-residue.sh

# Emit the current Primitive residue evidence for deliberate reclassification.
[group('Repository maintenance')]
primitive-residue-emit:
    @bash scripts/checks/primitive-residue.sh --emit

# Regenerate the bundled geolocation seed database from local GeoNames inputs.
[group('Repository maintenance')]
geo-seed:
    @python3 scripts/generators/generate-seed.py
