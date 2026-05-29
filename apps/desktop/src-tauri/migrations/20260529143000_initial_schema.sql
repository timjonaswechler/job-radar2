-- Initial Job Radar phase-one schema.
-- Database identifiers are English. German domain/UI labels are mapped in Rust/React.

CREATE TABLE IF NOT EXISTS app_metadata (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);

CREATE TABLE job_sources (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  source_system TEXT NOT NULL,
  config_json TEXT NOT NULL DEFAULT '{}',
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  delay_ms INTEGER NOT NULL DEFAULT 1000 CHECK (delay_ms >= 0),
  run_limit INTEGER CHECK (run_limit IS NULL OR run_limit >= 0),
  retry_limit INTEGER NOT NULL DEFAULT 3 CHECK (retry_limit >= 0),
  backoff_ms INTEGER NOT NULL DEFAULT 1000 CHECK (backoff_ms >= 0),
  stop_on_blocking INTEGER NOT NULL DEFAULT 1 CHECK (stop_on_blocking IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (json_valid(config_json))
);

CREATE TABLE search_queries (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  search_term TEXT,
  location_text TEXT,
  radius_km INTEGER CHECK (radius_km IS NULL OR radius_km >= 0),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE search_query_sources (
  search_query_id TEXT NOT NULL REFERENCES search_queries(id) ON DELETE CASCADE,
  job_source_id TEXT NOT NULL REFERENCES job_sources(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (search_query_id, job_source_id)
);

CREATE TABLE match_rules (
  id TEXT PRIMARY KEY NOT NULL,
  search_query_id TEXT NOT NULL REFERENCES search_queries(id) ON DELETE CASCADE,
  operation TEXT NOT NULL CHECK (operation IN ('title_contains', 'title_does_not_contain')),
  value TEXT NOT NULL CHECK (length(trim(value)) > 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE exclusion_terms (
  id TEXT PRIMARY KEY NOT NULL,
  search_query_id TEXT REFERENCES search_queries(id) ON DELETE CASCADE,
  term TEXT NOT NULL CHECK (length(trim(term)) > 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_exclusion_terms_global_term
  ON exclusion_terms (lower(term))
  WHERE search_query_id IS NULL;

CREATE UNIQUE INDEX idx_exclusion_terms_search_query_term
  ON exclusion_terms (search_query_id, lower(term))
  WHERE search_query_id IS NOT NULL;

CREATE TABLE postings (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  normalized_title TEXT NOT NULL,
  company TEXT NOT NULL,
  primary_location TEXT,
  region TEXT,
  work_model TEXT NOT NULL DEFAULT 'unknown'
    CHECK (work_model IN ('remote', 'hybrid', 'on_site', 'unknown')),
  status TEXT NOT NULL DEFAULT 'new'
    CHECK (status IN ('new', 'interesting', 'review_later', 'hidden', 'converted_to_application')),
  description_plain_text TEXT NOT NULL DEFAULT '' CHECK (length(description_plain_text) <= 200000),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_postings_practical_identity
  ON postings (
    lower(company),
    normalized_title,
    coalesce(lower(primary_location), ''),
    coalesce(lower(region), ''),
    work_model
  );

CREATE TABLE search_runs (
  id TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL DEFAULT 'running'
    CHECK (status IN ('running', 'completed', 'completed_with_errors', 'cancelled', 'failed')),
  started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  finished_at TEXT,
  current_search_query_id TEXT REFERENCES search_queries(id) ON DELETE SET NULL,
  current_job_source_id TEXT REFERENCES job_sources(id) ON DELETE SET NULL,
  found_findings_count INTEGER NOT NULL DEFAULT 0 CHECK (found_findings_count >= 0),
  new_postings_count INTEGER NOT NULL DEFAULT 0 CHECK (new_postings_count >= 0),
  excluded_findings_count INTEGER NOT NULL DEFAULT 0 CHECK (excluded_findings_count >= 0),
  error_count INTEGER NOT NULL DEFAULT 0 CHECK (error_count >= 0),
  error_summary TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK ((status = 'running' AND finished_at IS NULL) OR (status <> 'running'))
);

CREATE UNIQUE INDEX idx_search_runs_only_one_running
  ON search_runs (status)
  WHERE status = 'running';

CREATE TABLE findings (
  id TEXT PRIMARY KEY NOT NULL,
  posting_id TEXT NOT NULL REFERENCES postings(id) ON DELETE CASCADE,
  job_source_id TEXT NOT NULL REFERENCES job_sources(id) ON DELETE RESTRICT,
  search_run_id TEXT REFERENCES search_runs(id) ON DELETE SET NULL,
  result_url TEXT NOT NULL,
  canonical_url TEXT,
  external_id TEXT,
  title_snapshot TEXT,
  company_snapshot TEXT,
  location_snapshot TEXT,
  work_model_snapshot TEXT CHECK (work_model_snapshot IS NULL OR work_model_snapshot IN ('remote', 'hybrid', 'on_site', 'unknown')),
  found_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_findings_result_url
  ON findings (result_url);

CREATE UNIQUE INDEX idx_findings_canonical_url
  ON findings (canonical_url)
  WHERE canonical_url IS NOT NULL;

CREATE UNIQUE INDEX idx_findings_job_source_external_id
  ON findings (job_source_id, external_id)
  WHERE external_id IS NOT NULL;

CREATE TABLE excluded_findings (
  id TEXT PRIMARY KEY NOT NULL,
  search_run_id TEXT REFERENCES search_runs(id) ON DELETE SET NULL,
  search_query_id TEXT REFERENCES search_queries(id) ON DELETE SET NULL,
  job_source_id TEXT REFERENCES job_sources(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  company TEXT,
  result_url TEXT,
  matched_exclusion_term TEXT NOT NULL,
  retained_until TEXT NOT NULL,
  found_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE applications (
  id TEXT PRIMARY KEY NOT NULL,
  posting_id TEXT NOT NULL UNIQUE REFERENCES postings(id) ON DELETE RESTRICT,
  status TEXT NOT NULL DEFAULT 'new'
    CHECK (status IN ('new', 'preparing_documents', 'applied', 'response', 'first_interview', 'technical_interview', 'offer', 'rejected', 'withdrawn', 'archived')),
  notes TEXT NOT NULL DEFAULT '',
  applied_on TEXT,
  next_reminder_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE reminders (
  id TEXT PRIMARY KEY NOT NULL,
  reminder_type TEXT NOT NULL CHECK (reminder_type IN ('start_search_run', 'follow_up_application', 'interview', 'custom')),
  title TEXT NOT NULL,
  due_at TEXT NOT NULL,
  done_at TEXT,
  application_id TEXT REFERENCES applications(id) ON DELETE CASCADE,
  posting_id TEXT REFERENCES postings(id) ON DELETE CASCADE,
  search_run_id TEXT REFERENCES search_runs(id) ON DELETE SET NULL,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_job_sources_active ON job_sources (active);
CREATE INDEX idx_search_queries_active ON search_queries (active);
CREATE INDEX idx_match_rules_search_query ON match_rules (search_query_id);
CREATE INDEX idx_postings_status ON postings (status);
CREATE INDEX idx_findings_posting ON findings (posting_id);
CREATE INDEX idx_findings_search_run ON findings (search_run_id);
CREATE INDEX idx_excluded_findings_retained_until ON excluded_findings (retained_until);
CREATE INDEX idx_applications_status ON applications (status);
CREATE INDEX idx_reminders_due_open ON reminders (due_at) WHERE done_at IS NULL;
