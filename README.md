# footon

footon turns Claude and Codex JSONL transcripts into sanitized, unlisted links.
Raw transcripts remain local. Only user and assistant prose from an explicitly
approved draft can be published.

```text
raw JSONL -> Rust draft + local scanners -> explicit approval -> Cloudflare edge rescan -> unlisted link
```

The remote MCP endpoint is `https://footon.dev/mcp`. It uses OAuth 2.1 with
passwordless email sign-in and exposes only `share_create`, `share_list`, and
`share_revoke`.

## CLI

```sh
cargo install --git https://github.com/douglance/footon footon
footon draft thread.jsonl --title "Public title" --output footon-draft.json
FOOTON_TOKEN=... footon publish footon-draft.json
```

Drafting makes no network request. Publishing is a separate command and refuses
plain HTTP except for localhost tests. See [cli/README.md](cli/README.md) for the
full contract.

## Development

```sh
npm install
npm run check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The Worker limits complexity to 8, functions to 45 lines, and files to 220
lines. The Rust crate denies Clippy `all`, `pedantic`, cognitive complexity,
large functions, warnings, and unsafe code.

## Safety boundary

The official CLI combines `redact-core` with independent credential, key,
connection-string, path, and metadata removal. The Worker validates the exact
`footon.share.v1` shape and rejects remaining high-signal secrets and personal
data. No scanner can prove arbitrary text is secret-free, so explicit local
review remains mandatory.
