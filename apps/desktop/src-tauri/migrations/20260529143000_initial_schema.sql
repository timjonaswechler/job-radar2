-- Initial Job Radar phase-one schema.
-- Domain terms follow CONTEXT.md; identifiers avoid umlauts for portability.

CREATE TABLE app_metadata (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);

CREATE TABLE jobquellen (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  quellsystem TEXT NOT NULL,
  config_json TEXT NOT NULL DEFAULT '{}',
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  delay_ms INTEGER NOT NULL DEFAULT 1000 CHECK (delay_ms >= 0),
  limit_per_suchlauf INTEGER CHECK (limit_per_suchlauf IS NULL OR limit_per_suchlauf >= 0),
  retry_limit INTEGER NOT NULL DEFAULT 3 CHECK (retry_limit >= 0),
  backoff_ms INTEGER NOT NULL DEFAULT 1000 CHECK (backoff_ms >= 0),
  stop_on_blocking INTEGER NOT NULL DEFAULT 1 CHECK (stop_on_blocking IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (json_valid(config_json))
);

CREATE TABLE suchanfragen (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  suchbegriff TEXT,
  location_text TEXT,
  radius_km INTEGER CHECK (radius_km IS NULL OR radius_km >= 0),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE suchanfrage_jobquellen (
  suchanfrage_id TEXT NOT NULL REFERENCES suchanfragen(id) ON DELETE CASCADE,
  jobquelle_id TEXT NOT NULL REFERENCES jobquellen(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (suchanfrage_id, jobquelle_id)
);

CREATE TABLE trefferregeln (
  id TEXT PRIMARY KEY NOT NULL,
  suchanfrage_id TEXT NOT NULL REFERENCES suchanfragen(id) ON DELETE CASCADE,
  operation TEXT NOT NULL CHECK (operation IN ('title_contains', 'title_does_not_contain')),
  value TEXT NOT NULL CHECK (length(trim(value)) > 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE ausschlussbegriffe (
  id TEXT PRIMARY KEY NOT NULL,
  suchanfrage_id TEXT REFERENCES suchanfragen(id) ON DELETE CASCADE,
  term TEXT NOT NULL CHECK (length(trim(term)) > 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_ausschlussbegriffe_global_term
  ON ausschlussbegriffe (lower(term))
  WHERE suchanfrage_id IS NULL;

CREATE UNIQUE INDEX idx_ausschlussbegriffe_suchanfrage_term
  ON ausschlussbegriffe (suchanfrage_id, lower(term))
  WHERE suchanfrage_id IS NOT NULL;

CREATE TABLE stellenanzeigen (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  normalized_title TEXT NOT NULL,
  company TEXT NOT NULL,
  primary_location TEXT,
  region TEXT,
  arbeitsmodell TEXT NOT NULL DEFAULT 'unknown'
    CHECK (arbeitsmodell IN ('remote', 'hybrid', 'on_site', 'unknown')),
  status TEXT NOT NULL DEFAULT 'neu'
    CHECK (status IN ('neu', 'interessant', 'spaeter_ansehen', 'ausgeblendet', 'in_bewerbung_umgewandelt')),
  description_plain_text TEXT NOT NULL DEFAULT '' CHECK (length(description_plain_text) <= 200000),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_stellenanzeigen_practical_identity
  ON stellenanzeigen (
    lower(company),
    normalized_title,
    coalesce(lower(primary_location), ''),
    coalesce(lower(region), ''),
    arbeitsmodell
  );

CREATE TABLE suchlaeufe (
  id TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL DEFAULT 'laeuft'
    CHECK (status IN ('laeuft', 'abgeschlossen', 'abgeschlossen_mit_fehlern', 'abgebrochen', 'fehlgeschlagen')),
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  finished_at TEXT,
  current_suchanfrage_id TEXT REFERENCES suchanfragen(id) ON DELETE SET NULL,
  current_jobquelle_id TEXT REFERENCES jobquellen(id) ON DELETE SET NULL,
  found_fundstellen_count INTEGER NOT NULL DEFAULT 0 CHECK (found_fundstellen_count >= 0),
  new_stellenanzeigen_count INTEGER NOT NULL DEFAULT 0 CHECK (new_stellenanzeigen_count >= 0),
  excluded_treffer_count INTEGER NOT NULL DEFAULT 0 CHECK (excluded_treffer_count >= 0),
  error_count INTEGER NOT NULL DEFAULT 0 CHECK (error_count >= 0),
  error_summary TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK ((status = 'laeuft' AND finished_at IS NULL) OR (status <> 'laeuft'))
);

CREATE UNIQUE INDEX idx_suchlaeufe_only_one_running
  ON suchlaeufe (status)
  WHERE status = 'laeuft';

CREATE TABLE fundstellen (
  id TEXT PRIMARY KEY NOT NULL,
  stellenanzeige_id TEXT NOT NULL REFERENCES stellenanzeigen(id) ON DELETE CASCADE,
  jobquelle_id TEXT NOT NULL REFERENCES jobquellen(id) ON DELETE RESTRICT,
  suchlauf_id TEXT REFERENCES suchlaeufe(id) ON DELETE SET NULL,
  result_url TEXT NOT NULL,
  canonical_url TEXT,
  external_id TEXT,
  title_snapshot TEXT,
  company_snapshot TEXT,
  location_snapshot TEXT,
  arbeitsmodell_snapshot TEXT CHECK (arbeitsmodell_snapshot IS NULL OR arbeitsmodell_snapshot IN ('remote', 'hybrid', 'on_site', 'unknown')),
  found_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_fundstellen_result_url
  ON fundstellen (result_url);

CREATE UNIQUE INDEX idx_fundstellen_canonical_url
  ON fundstellen (canonical_url)
  WHERE canonical_url IS NOT NULL;

CREATE UNIQUE INDEX idx_fundstellen_jobquelle_external_id
  ON fundstellen (jobquelle_id, external_id)
  WHERE external_id IS NOT NULL;

CREATE TABLE ausgeschlossene_treffer (
  id TEXT PRIMARY KEY NOT NULL,
  suchlauf_id TEXT REFERENCES suchlaeufe(id) ON DELETE SET NULL,
  suchanfrage_id TEXT REFERENCES suchanfragen(id) ON DELETE SET NULL,
  jobquelle_id TEXT REFERENCES jobquellen(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  company TEXT,
  result_url TEXT,
  matched_ausschlussbegriff TEXT NOT NULL,
  retained_until TEXT NOT NULL,
  found_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE bewerbungen (
  id TEXT PRIMARY KEY NOT NULL,
  stellenanzeige_id TEXT NOT NULL UNIQUE REFERENCES stellenanzeigen(id) ON DELETE RESTRICT,
  status TEXT NOT NULL DEFAULT 'neu'
    CHECK (status IN ('neu', 'unterlagen_vorbereiten', 'beworben', 'rueckmeldung', 'erstgespraech', 'technisches_interview', 'angebot', 'abgelehnt', 'zurueckgezogen', 'archiviert')),
  notes TEXT NOT NULL DEFAULT '',
  applied_on TEXT,
  next_reminder_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE erinnerungen (
  id TEXT PRIMARY KEY NOT NULL,
  reminder_type TEXT NOT NULL CHECK (reminder_type IN ('suchlauf_starten', 'bewerbung_nachfassen', 'interview', 'custom')),
  title TEXT NOT NULL,
  due_at TEXT NOT NULL,
  done_at TEXT,
  bewerbung_id TEXT REFERENCES bewerbungen(id) ON DELETE CASCADE,
  stellenanzeige_id TEXT REFERENCES stellenanzeigen(id) ON DELETE CASCADE,
  suchlauf_id TEXT REFERENCES suchlaeufe(id) ON DELETE SET NULL,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_jobquellen_active ON jobquellen (active);
CREATE INDEX idx_suchanfragen_active ON suchanfragen (active);
CREATE INDEX idx_trefferregeln_suchanfrage ON trefferregeln (suchanfrage_id);
CREATE INDEX idx_stellenanzeigen_status ON stellenanzeigen (status);
CREATE INDEX idx_fundstellen_stellenanzeige ON fundstellen (stellenanzeige_id);
CREATE INDEX idx_fundstellen_suchlauf ON fundstellen (suchlauf_id);
CREATE INDEX idx_ausgeschlossene_treffer_retained_until ON ausgeschlossene_treffer (retained_until);
CREATE INDEX idx_bewerbungen_status ON bewerbungen (status);
CREATE INDEX idx_erinnerungen_due_open ON erinnerungen (due_at) WHERE done_at IS NULL;
