#!/usr/bin/env python3
"""Rebaseline SQLx migration bookkeeping for an already-migrated SQLite DB.

This is a development-only helper for migration squashing: it preserves table data
and rewrites `_sqlx_migrations` to match the current migration files, but only if
the existing DB schema already matches the schema produced by those migrations.
"""

from __future__ import annotations

import difflib
import hashlib
import json
import os
import platform
import re
import sqlite3
import sys
import tempfile
import time
from pathlib import Path
from typing import Iterable


MIGRATIONS_TABLE_SQL = """
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
)
"""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def tauri_app_data_dir(root: Path) -> Path:
    override = os.environ.get("JOB_RADAR_APP_DATA_DIR")
    if override:
        return Path(override).expanduser()

    config_path = root / "src-tauri" / "tauri.conf.json"
    with config_path.open("r", encoding="utf-8") as handle:
        identifier = json.load(handle)["identifier"]

    home = Path.home()
    system = platform.system()
    if system == "Darwin":
        return home / "Library" / "Application Support" / identifier
    if system == "Linux":
        return Path(os.environ.get("XDG_DATA_HOME", home / ".local" / "share")) / identifier
    if system == "Windows":
        return Path(os.environ.get("APPDATA", home / "AppData" / "Roaming")) / identifier

    raise RuntimeError(
        f"Unsupported platform {system!r}; set JOB_RADAR_APP_DATA_DIR explicitly."
    )


def database_path(root: Path) -> Path:
    override = os.environ.get("JOB_RADAR_DB_PATH")
    if override:
        return Path(override).expanduser()

    return tauri_app_data_dir(root) / os.environ.get("JOB_RADAR_DB_NAME", "job_radar.db")


def migrations_dir(root: Path) -> Path:
    configured = Path(os.environ.get("MIGRATIONS_DIR", root / "src-tauri" / "migrations"))
    if configured.is_absolute():
        return configured
    return root / configured


def migration_files(directory: Path) -> list[Path]:
    files = sorted(path for path in directory.glob("*.sql") if not path.name.endswith(".down.sql"))
    if not files:
        raise RuntimeError(f"No .sql migrations found in {directory}")
    return files


def parse_migration(path: Path) -> tuple[int, str, bytes]:
    match = re.match(r"^(\d+)_(.+)\.sql$", path.name)
    if not match:
        raise RuntimeError(
            f"Migration filename must look like <version>_<description>.sql: {path.name}"
        )

    version = int(match.group(1))
    description = match.group(2).replace("_", " ")
    checksum = hashlib.sha384(path.read_bytes()).digest()
    return version, description, checksum


def build_schema_db(files: Iterable[Path]) -> Path:
    temp_dir = Path(tempfile.mkdtemp(prefix="job-radar-rebaseline-"))
    schema_db = temp_dir / "schema.db"
    connection = sqlite3.connect(schema_db)
    try:
        connection.execute("PRAGMA foreign_keys = ON")
        for path in files:
            connection.executescript(path.read_text(encoding="utf-8"))
        connection.commit()
    finally:
        connection.close()
    return schema_db


def normalize_sql(sql: str) -> str:
    normalized = re.sub(r"\s+", " ", sql.strip())
    normalized = re.sub(r"\s+([(),])", r"\1", normalized)
    normalized = re.sub(r"([(),])\s+", r"\1", normalized)
    return normalized


def schema_rows(path: Path) -> list[tuple[str, str, str, str]]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        rows = connection.execute(
            """
            SELECT type, name, tbl_name, sql
            FROM sqlite_schema
            WHERE sql IS NOT NULL
              AND name NOT LIKE 'sqlite_%'
              AND name <> '_sqlx_migrations'
            ORDER BY
              CASE type
                WHEN 'table' THEN 0
                WHEN 'view' THEN 1
                WHEN 'index' THEN 2
                WHEN 'trigger' THEN 3
                ELSE 4
              END,
              name
            """
        ).fetchall()
    finally:
        connection.close()

    return [(kind, name, table, normalize_sql(sql)) for kind, name, table, sql in rows]


def format_schema_rows(rows: list[tuple[str, str, str, str]]) -> list[str]:
    return [f"{kind}\t{name}\t{table}\t{sql}" for kind, name, table, sql in rows]


def assert_schema_matches(existing_db: Path, expected_db: Path) -> None:
    existing = format_schema_rows(schema_rows(existing_db))
    expected = format_schema_rows(schema_rows(expected_db))
    if existing == expected:
        return

    diff = "\n".join(
        difflib.unified_diff(
            expected,
            existing,
            fromfile="expected-from-current-migrations",
            tofile="existing-database",
            lineterm="",
        )
    )
    raise RuntimeError(
        "Existing database schema does not match the current migration schema; "
        "refusing to rewrite SQLx migration metadata.\n\n"
        f"{diff}"
    )


def backup_database(existing_db: Path, root: Path) -> Path:
    timestamp = time.strftime("%Y%m%d-%H%M%S")
    backup_dir = Path(os.environ.get("JOB_RADAR_BACKUP_DIR", root / "backups" / "db")) / timestamp
    backup_dir.mkdir(parents=True, exist_ok=True)
    backup_path = backup_dir / existing_db.name

    source = sqlite3.connect(f"file:{existing_db}?mode=ro", uri=True)
    target = sqlite3.connect(backup_path)
    try:
        source.backup(target)
    finally:
        target.close()
        source.close()

    return backup_path


def rebaseline(existing_db: Path, files: list[Path], migrations: list[tuple[int, str, bytes]]) -> None:
    connection = sqlite3.connect(existing_db, timeout=5)
    try:
        connection.execute(MIGRATIONS_TABLE_SQL)
        connection.execute("BEGIN")
        connection.execute("DELETE FROM _sqlx_migrations")
        connection.executemany(
            """
            INSERT INTO _sqlx_migrations
              (version, description, success, checksum, execution_time)
            VALUES (?, ?, 1, ?, -1)
            """,
            migrations,
        )
        connection.commit()
    except Exception:
        connection.rollback()
        raise
    finally:
        connection.close()


def main() -> int:
    root = repo_root()
    db = database_path(root)
    directory = migrations_dir(root)

    if not db.exists():
        print(f"Database not found: {db}", file=sys.stderr)
        return 1

    files = migration_files(directory)
    migrations = [parse_migration(path) for path in files]

    print(f"SQLite database: {db}")
    print(f"Migrations directory: {directory}")
    print("Checking existing schema against current migrations...")

    try:
        expected_db = build_schema_db(files)
        assert_schema_matches(db, expected_db)
        backup_path = backup_database(db, root)
        rebaseline(db, files, migrations)
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    print(f"Backup written: {backup_path}")
    print(f"Rebaselined _sqlx_migrations with {len(migrations)} migration(s):")
    for version, description, _checksum in migrations:
        print(f"  - {version}_{description.replace(' ', '_')}")
    print("Existing table data was preserved.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
