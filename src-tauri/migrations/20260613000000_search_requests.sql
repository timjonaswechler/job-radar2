CREATE TABLE search_requests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  status TEXT NOT NULL,
  include_rules_json TEXT NOT NULL DEFAULT '[]',
  exclude_rules_json TEXT NOT NULL DEFAULT '[]',
  locations_json TEXT NOT NULL DEFAULT '[]',
  radius_km INTEGER NULL,
  source_ids_json TEXT NOT NULL DEFAULT '[]',
  validation_error TEXT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (status IN ('draft', 'active', 'disabled', 'invalid')),
  CHECK (json_valid(include_rules_json)),
  CHECK (json_valid(exclude_rules_json)),
  CHECK (json_valid(locations_json)),
  CHECK (json_valid(source_ids_json)),
  CHECK (radius_km IS NULL OR radius_km >= 0)
);
