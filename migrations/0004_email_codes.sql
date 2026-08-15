ALTER TABLE oauth_magic_links_v2
  ADD COLUMN verification_code_hash TEXT;

ALTER TABLE oauth_magic_links_v2
  ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
