# footon CLI

`footon` reads Claude Code or Codex JSONL locally, keeps only user and assistant
prose, removes privileged/tool/reasoning content, and writes a sanitized draft
plus a separate report. It performs no network request during `draft`.

```sh
cargo run --manifest-path cli/Cargo.toml -- draft thread.jsonl \
  --title "Public title" --output footon-draft.json

FOOTON_TOKEN=... cargo run --manifest-path cli/Cargo.toml -- \
  publish footon-draft.json
```

`publish` is the only network mutation. It sets `approvedAt` at invocation time
and sends the exact `footon.share.v1` document to the HTTPS share endpoint.
The bearer token is read from `FOOTON_TOKEN` and is never written to disk.

Safety is layered: `redact-core` 0.9.1 handles PII and high-signal credential
shapes, while Footon's independent local pattern pack incorporates the broad
provider-prefix, assignment, connection-string, private-key, and path families
used by Gitleaks and Kingfisher. Footon never validates a detected credential
against a live service.

Incurs supplies `--help`, `--llms`, `--schema`, and `--mcp`; local agents can
run `footon --mcp` to call the same two typed commands over stdio MCP.

Incurs 0.5.2 is pinned from crates.io, so a fresh Footon checkout builds without
an adjacent repository. Before publishing, run
`cargo package --manifest-path cli/Cargo.toml --list` and then
`cargo package --manifest-path cli/Cargo.toml`.
