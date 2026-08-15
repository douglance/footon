CREATE TABLE IF NOT EXISTS service_keys (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  name TEXT NOT NULL,
  system TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  token_prefix TEXT NOT NULL,
  scope TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS service_keys_owner_idx
  ON service_keys (owner_id, created_at);

CREATE INDEX IF NOT EXISTS service_keys_expiry_idx
  ON service_keys (expires_at, revoked_at);

CREATE TABLE IF NOT EXISTS remote_log_reports (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  key_id TEXT NOT NULL,
  system TEXT NOT NULL,
  environment TEXT NOT NULL,
  level TEXT NOT NULL CHECK (level IN ('debug', 'info', 'warn', 'error', 'critical')),
  event TEXT NOT NULL,
  summary TEXT NOT NULL,
  redactions INTEGER NOT NULL DEFAULT 0,
  source_event_id TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  received_at TEXT NOT NULL,
  UNIQUE (key_id, source_event_id),
  FOREIGN KEY (key_id) REFERENCES service_keys(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS remote_log_reports_owner_idx
  ON remote_log_reports (owner_id, received_at DESC);

CREATE INDEX IF NOT EXISTS remote_log_reports_key_idx
  ON remote_log_reports (key_id, received_at DESC);
