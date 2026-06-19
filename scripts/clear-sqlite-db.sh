#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_data_dir="$(${PYTHON:-python3} "$repo_root/scripts/tauri-app-data-dir.py")"
db_name="${JOB_RADAR_DB_NAME:-job_radar.db}"

if [[ -n "${JOB_RADAR_DB_PATH:-}" ]]; then
  db_path="$JOB_RADAR_DB_PATH"
else
  db_path="$app_data_dir/$db_name"
fi

files=(
  "$db_path"
  "$db_path-wal"
  "$db_path-shm"
  "$db_path-journal"
)

echo "Tauri app data dir: $app_data_dir"
echo "SQLite database file family to delete:"
for file in "${files[@]}"; do
  echo "  - $file"
done
echo "Custom source/profile JSON documents are not deleted. Close the app before continuing."

if [[ "${YES:-0}" != "1" ]]; then
  if [[ ! -t 0 ]]; then
    echo "No interactive stdin available. Re-run with YES=1, or use: just db-clear-force" >&2
    exit 1
  fi

  read -r -p "Type 'delete' to continue: " confirmation
  if [[ "$confirmation" != "delete" ]]; then
    echo "Aborted."
    exit 1
  fi
fi

removed=0
for file in "${files[@]}"; do
  if [[ -e "$file" ]]; then
    rm -f -- "$file"
    echo "deleted: $file"
    removed=1
  else
    echo "missing: $file"
  fi
done

if [[ "$removed" == "0" ]]; then
  echo "No SQLite files found."
else
  echo "SQLite database cleared. Next app start will recreate it via migrations/seeding."
fi
