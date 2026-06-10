CREATE TABLE system_profiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  description TEXT NULL,
  adapter_key TEXT NOT NULL,
  definition_schema_version INTEGER NOT NULL,
  definition_json TEXT NOT NULL DEFAULT '{}',
  source_config_schema_json TEXT NOT NULL DEFAULT '{}',
  built_in INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL,
  validation_error TEXT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (key <> '' AND key NOT GLOB '*[^a-z0-9_]*'),
  CHECK (adapter_key <> '' AND adapter_key NOT GLOB '*[^a-z0-9_]*'),
  CHECK (definition_schema_version > 0),
  CHECK (json_valid(definition_json)),
  CHECK (json_valid(source_config_schema_json)),
  CHECK (built_in IN (0, 1)),
  CHECK (status IN ('draft', 'active', 'disabled', 'invalid'))
);

ALTER TABLE sources
  ADD COLUMN system_profile_id INTEGER NULL REFERENCES system_profiles(id) ON DELETE RESTRICT;

CREATE INDEX idx_system_profiles_adapter_key ON system_profiles(adapter_key);
CREATE INDEX idx_sources_system_profile_id ON sources(system_profile_id);
