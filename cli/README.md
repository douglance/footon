# footon CLI

`footon` reads Claude Code or Codex JSONL locally, keeps only user and assistant
prose, removes privileged/tool/reasoning content, and writes a sanitized draft
plus a separate report. It performs no network request during `draft`.

```sh
cargo run --manifest-path cli/Cargo.toml -- draft thread.jsonl \
  --title "Public title" --output footon-draft.json

cargo run --manifest-path cli/Cargo.toml -- signin you@example.com

cargo run --manifest-path cli/Cargo.toml -- publish footon-draft.json

cargo run --manifest-path cli/Cargo.toml -- signout
```

`signin` performs the OAuth authorization-code flow entirely in the terminal.
It sends a six-digit code to the supplied email address, prompts for that code
on standard input, and stores the refreshable session in the operating system
credential store. The code is never accepted in process arguments, so it does
not enter shell history. Normal command output never contains the access token,
refresh token, authorization code, verifier, or state.

Authenticated commands use the stored session and rotate an expiring access
token automatically. `FOOTON_TOKEN` remains an explicit, non-persistent
override for automation and recovery. `signout` revokes the stored refresh
token before deleting the local session.

`publish` sets `approvedAt` at invocation time and sends the exact
`footon.share.v2` document to the HTTPS share endpoint.

Safety is layered: `redact-core` 0.9.1 handles PII and high-signal credential
shapes, while Footon's independent local pattern pack incorporates the broad
provider-prefix, assignment, connection-string, private-key, and path families
used by Gitleaks and Kingfisher. Footon never validates a detected credential
against a live service.

Incurs supplies `--help`, `--llms`, `--schema`, and `--mcp`; local agents can
run `footon --mcp` to call the same typed commands over stdio MCP.

Incurs 0.5.2 is pinned from crates.io, so a fresh Footon checkout builds without
an adjacent repository. Before publishing, run
`cargo package --manifest-path cli/Cargo.toml --list` and then
`cargo package --manifest-path cli/Cargo.toml`.
