CREATE TABLE oauth_pending (
  id TEXT PRIMARY KEY,
  request_json TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE TABLE magic_links (
  token_hash TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  pending_id TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  email TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE TABLE auth_attempts (
  rate_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX auth_attempts_rate ON auth_attempts (rate_key, created_at);

CREATE TABLE shares (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  title TEXT NOT NULL,
  document_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE INDEX shares_owner ON shares (owner_id, created_at DESC);
