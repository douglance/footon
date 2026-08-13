import { CSS } from './style.js'
import { htmlResponse, page } from './security.js'

export function renderHome(): Response {
  const content = `<h1>Share the work.<br>Keep the secrets.</h1><p class="lede">footon turns Claude and Codex threads into clean, unlisted links. The raw thread stays on your machine. Only the conversation you approve is uploaded.</p><div class="actions"><a class="button" href="/install">Install the CLI</a><a class="button secondary" href="/connect">Connect an agent</a></div><p class="muted">Every share is scanned locally, checked again at the edge, and revocable by its owner.</p>`
  return htmlResponse(page('Safe agent threads', content))
}

export function renderInstall(): Response {
  const content = `<h1>Install the CLI</h1><p class="lede">Build the Rust client from this checkout, then keep raw transcripts local.</p><pre><code>cargo install --path cli
footon draft thread.jsonl --title "Public title" --output footon-draft.json
FOOTON_TOKEN=... footon publish footon-draft.json</code></pre><p>Drafting never uses the network. Publishing is a separate, explicit command.</p>`
  return htmlResponse(page('Install', content))
}

export function renderConnect(): Response {
  const content = `<h1>Connect an agent</h1><p class="lede">Add this remote MCP endpoint to any OAuth-capable agent:</p><pre><code>https://footon.dev/mcp</code></pre><p>Your client opens passwordless email sign-in, asks for approval, then receives only the share scopes you approve.</p>`
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
