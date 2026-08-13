# footon

Footon turns Claude and Codex JSONL transcripts into sanitized, unlisted links.
Raw transcripts remain local. Only user and assistant prose from an explicitly
approved draft can be published.

```text
raw JSONL -> Rust draft + local scanners -> explicit approval -> Cloudflare edge rescan -> unlisted link
```

The remote MCP endpoint is `https://footon.dev/mcp`. It uses OAuth 2.1 with
passwordless email sign-in and exposes only `share_create`, `share_list`, and
`share_revoke`.

The application is Rust end to end. Topcoat owns route and method matching and
builds the Tailwind stylesheet; workers-rs provides the Cloudflare D1, email,
Turnstile, and runtime bindings. Node remains only for Wrangler packaging and
deployment.

## CLI

```sh
cargo install --git https://github.com/douglance/footon footon
footon draft thread.jsonl --title "Public title" --output footon-draft.json
FOOTON_TOKEN=... footon publish footon-draft.json
footon fetch https://footon.dev/s/example
```

Drafting makes no network request. Publishing is a separate command and refuses
plain HTTP except for localhost tests. See [cli/README.md](cli/README.md) for the
full contract.

Public share URLs use HTTP content negotiation. Browsers receive the dense HTML
reader by default. Agents can send `Accept: text/markdown`; `footon fetch` does
this and writes the exact Markdown response to stdout. Explicit equal-quality
HTML and Markdown preferences resolve to Markdown.

## OAuth migration boundary

The Rust OAuth service uses new D1 tables and the scopes `shares:read` and
`shares:write`. Existing OAuth clients, authorization codes, access tokens, and
refresh tokens are intentionally not migrated. Existing share rows remain in
the original `shares` table and remain readable, including v1 share documents.

## Development

```sh
npm install
npm run check
npm run dev
```

`npm run check` formats, lints, tests, and builds the complete Rust workspace.
Apply D1 migrations before running a fresh local Worker or deploying:

```sh
npx wrangler d1 migrations apply footon --local
```

## Safety boundary

The official CLI combines `redact-core` with independent credential, key,
connection-string, path, and metadata removal. The Worker validates the exact
`footon.share.v2` shape and rejects remaining high-signal secrets and personal
data. No scanner can prove arbitrary text is secret-free, so explicit local
review remains mandatory.
