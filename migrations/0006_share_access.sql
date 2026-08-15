ALTER TABLE shares
  ADD COLUMN general_access TEXT NOT NULL DEFAULT 'anyone_with_link'
  CHECK (general_access IN ('anyone_with_link', 'restricted'));

CREATE TABLE IF NOT EXISTS share_members (
  id TEXT PRIMARY KEY,
  share_id TEXT NOT NULL,
  email TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('viewer', 'editor')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (share_id, email),
  FOREIGN KEY (share_id) REFERENCES shares(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS share_members_share_idx
  ON share_members (share_id, created_at);

CREATE TABLE IF NOT EXISTS share_viewer_challenges (
  ticket_hash TEXT PRIMARY KEY,
  share_id TEXT NOT NULL,
  email TEXT NOT NULL,
  verification_code_hash TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  FOREIGN KEY (share_id) REFERENCES shares(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS share_viewer_challenges_expiry_idx
  ON share_viewer_challenges (expires_at);

CREATE TABLE IF NOT EXISTS share_browser_sessions (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  email TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS share_browser_sessions_expiry_idx
  ON share_browser_sessions (expires_at, revoked_at);

CREATE TABLE IF NOT EXISTS share_auth_attempts (
  rate_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS share_auth_attempts_rate_idx
  ON share_auth_attempts (rate_key, created_at);
