# Changelog

All notable Footon changes are recorded here. Footon follows Semantic
Versioning and keeps unreleased changes separate from tagged releases.

## [Unreleased]

## [0.2.0] - 2026-08-15

### Added

- Pro service keys for arbitrary named auth and infrastructure systems, with
  one-time secret display, hashing, scoped authority, expiry, last-use tracking,
  and immediate revocation.
- Idempotent remote log reports with per-key isolation, automatic sensitive-text
  redaction, rate limits, 30-day retention, HTTP, CLI, and MCP parity.
- Public shares that remain free forever and private shares restricted to Pro.
- Owner, Editor, and Viewer roles with share rename, member management,
  visibility changes, ownership transfer, blackout, and revoke permissions.
- Passwordless browser access for private share members, with short-lived codes,
  secure sessions, generic responses, throttling, and retention cleanup.
- Matching HTTP, CLI, local MCP, and remote MCP access-management operations.

### Changed

- The canonical CLI installation uses the published crates.io package with
  `cargo install footon --locked`.

## [0.1.0] - 2026-08-15

### Added

- Local Claude Code and Codex transcript parsing with explicit sanitized draft
  review before publishing.
- Independent PII, credential, key, connection-string, local-path, metadata,
  and prompt-injection defenses on the CLI and Cloudflare edge.
- Passwordless terminal sign-in with emailed six-digit codes, PKCE, secure OS
  credential storage, automatic refresh rotation, and server-first sign-out.
- Unlisted public share pages with HTML and Markdown content negotiation,
  local and remote blackouts, owner revocation, and MCP share tools.
- Privacy and terms pages, responsive landing and thread views, D1 migrations,
  and full Rust workspace quality gates.

[Unreleased]: https://github.com/douglance/footon/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/douglance/footon/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/douglance/footon/releases/tag/v0.1.0
