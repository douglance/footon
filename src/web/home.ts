import { CSS } from './style.js'
import { htmlResponse, page } from './security.js'

export function renderHome(): Response {
  const content = `<section class="panel"><p class="prompt">footon share</p><h1>Share the work. Keep the secrets.</h1><p class="lede">Claude and Codex threads become clean, unlisted links. Raw transcripts never leave your machine.</p><div class="actions"><a class="button" href="/install">Install CLI</a><a class="button secondary" href="/connect">Connect agent</a></div><dl class="facts"><div><dt>scan</dt><dd>local and edge</dd></div><div><dt>upload</dt><dd>approved prose only</dd></div><div><dt>access</dt><dd>unlisted and revocable</dd></div></dl></section>`
  return htmlResponse(page('Safe agent threads', content))
}

export function renderInstall(): Response {
  const content = `<p class="prompt">footon install</p><h1>Install the CLI</h1><p class="lede">Install the Rust client. Draft locally; publish explicitly.</p><pre><code>cargo install --git https://github.com/douglance/footon footon
footon draft thread.jsonl --title "Public title" --output footon-draft.json
FOOTON_TOKEN=... footon publish footon-draft.json</code></pre><p>Drafting never uses the network. Publishing is a separate, explicit command.</p>`
  return htmlResponse(page('Install', content))
}

export function renderConnect(): Response {
  const content = `<p class="prompt">agent mcp add footon</p><h1>Connect an agent</h1><p class="lede">Add this endpoint to any OAuth-capable agent.</p><pre><code>https://footon.dev/mcp</code></pre><p>Passwordless email sign-in grants only the share scopes you approve.</p>`
  return htmlResponse(page('Connect an agent', content))
}

export function renderCss(): Response {
  return new Response(CSS, {
    headers: {
      'content-type': 'text/css; charset=utf-8',
      'cache-control': 'public, max-age=3600',
      'x-content-type-options': 'nosniff',
    },
  })
}
