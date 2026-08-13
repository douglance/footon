import type { ClientInfo } from '@cloudflare/workers-oauth-provider'
import type { Env } from '../types.js'
import { escapeHtml, htmlResponse, page } from '../web/security.js'

export function loginPage(env: Env, pendingId: string): Response {
  const widget = env.TURNSTILE_SITE_KEY
    ? `<div class="cf-turnstile" data-sitekey="${escapeHtml(env.TURNSTILE_SITE_KEY)}"></div><script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>`
    : ''
  const content = `<h1>Sign in</h1><p>We’ll email a one-time link. No password.</p><form method="post" action="/auth/request"><input type="hidden" name="pending" value="${escapeHtml(pendingId)}"><label for="email">Email address</label><input id="email" name="email" type="email" autocomplete="email" required>${widget}<button type="submit">Email sign-in link</button></form>`
  return htmlResponse(page('Sign in', content), 200, turnstileCsp(Boolean(env.TURNSTILE_SITE_KEY)))
}

export function consentPage(pendingId: string, csrf: string, client: ClientInfo): Response {
  const name = client.clientName ?? 'Your MCP client'
  const content = `<h1>Connect ${escapeHtml(name)}?</h1><p>This client will be able to create, list, and revoke your footon shares. It cannot read raw local threads.</p><form method="post" action="/authorize"><input type="hidden" name="pending" value="${escapeHtml(pendingId)}"><input type="hidden" name="csrf" value="${escapeHtml(csrf)}"><button type="submit">Connect ${escapeHtml(name)}</button></form>`
  return htmlResponse(page('Authorize agent', content), 200, { 'set-cookie': csrfCookie(csrf) })
}

export function messagePage(title: string, message: string, status = 200): Response {
  return htmlResponse(
    page(title, `<h1>${escapeHtml(title)}</h1><p>${escapeHtml(message)}</p>`),
    status,
  )
}

function csrfCookie(token: string): string {
  return `__Host-footon_csrf=${token}; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age=600`
}

function turnstileCsp(enabled: boolean): HeadersInit {
  if (!enabled) return {}
  return {
    'content-security-policy':
      "default-src 'none'; script-src https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
  }
}
