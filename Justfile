set shell := ["bash", "-euo", "pipefail", "-c"]

alias clear-db := db-clear
alias squash-migrations := migrations-squash

# List available recipes.
default:
    @just --list

# Print the Tauri app data directory used by the installed/dev app.
app-data-dir:
    @python3 scripts/tauri-app-data-dir.py

# Print the SQLite database path in the Tauri app data directory.
db-path:
    @printf "%s/job_radar.db\n" "$(python3 scripts/tauri-app-data-dir.py)"

# Delete app-data SQLite DB family; keeps app-data/system-profiles/*.json. Set YES=1 to skip the prompt.
db-clear:
    @bash scripts/clear-sqlite-db.sh

# Same as db-clear, without the interactive confirmation prompt.
db-clear-force:
    @YES=1 bash scripts/clear-sqlite-db.sh

# Squash all SQLx migrations into one current-schema migration.
migrations-squash target="src-tauri/migrations/20260609000000_current_schema.sql":
    @bash scripts/squash-migrations.sh "{{target}}"

# Rewrite SQLx migration bookkeeping for the existing DB, preserving data. Refuses if schemas differ.
db-rebaseline-migrations:
    @python3 scripts/rebaseline-sqlx-migrations.py

# Squash migrations, then rebaseline SQLx bookkeeping without deleting table data. Dev-only.
db-preserve-after-squash: migrations-squash db-rebaseline-migrations
    @echo "Done. Existing DB data was preserved. Restart the app."

# Squash migrations, then clear the app-data DB so SQLx migration checksums cannot conflict.
db-reset-after-squash: migrations-squash db-clear-force
    @echo "Done. Start the app again to recreate the DB from the squashed migration."
