-- Current development schema for Job Radar.
-- During early development this migration is intentionally squashed instead of
-- keeping a history of intermediate schema experiments.

CREATE TABLE app_metadata (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY NOT NULL,
  value_json TEXT NOT NULL DEFAULT 'null',
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (json_valid(value_json))
);

CREATE TABLE browser_profiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  description TEXT NULL,
  name_i18n_key TEXT NULL,
  description_i18n_key TEXT NULL,
  definition_path TEXT NULL,
  definition_hash TEXT NULL,
  definition_schema_version INTEGER NOT NULL,
  definition_json TEXT NOT NULL DEFAULT '{}',
  source_config_schema_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL,
  validation_error TEXT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (key <> '' AND key NOT GLOB '*[^a-z0-9_]*'),
  CHECK (definition_schema_version > 0),
  CHECK (json_valid(definition_json)),
  CHECK (json_valid(source_config_schema_json)),
  CHECK (status IN ('draft', 'active', 'disabled', 'invalid'))
);

CREATE TABLE sources (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key TEXT UNIQUE NOT NULL,
  adapter_key TEXT NOT NULL,
  browser_profile_id INTEGER NULL REFERENCES browser_profiles(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  description TEXT NULL,
  source_config_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL,
  validation_error TEXT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (key <> '' AND key NOT GLOB '*[^a-z0-9_]*'),
  CHECK (adapter_key <> '' AND adapter_key NOT GLOB '*[^a-z0-9_]*'),
  CHECK (json_valid(source_config_json)),
  CHECK (status IN ('draft', 'active', 'disabled', 'invalid'))
);

CREATE INDEX idx_sources_adapter_key ON sources(adapter_key);
CREATE INDEX idx_sources_browser_profile_id ON sources(browser_profile_id);
