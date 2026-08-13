import { getShare } from '../shares/repository.js'
import type { Env, ShareMessage } from '../types.js'
import { escapeHtml, htmlResponse, page } from './security.js'

export async function renderShare(env: Env, id: string): Promise<Response> {
  if (!/^[A-Za-z0-9_-]{20,40}$/u.test(id)) return notFound()
  const share = await getShare(env.DB, id)
  if (!share) return notFound()
  const messages = share.document.messages.map(renderMessage).join('')
  const redactions = String(share.document.report.redactions)
  const content = `<article><div class="meta"><p class="muted">Unlisted thread · shared ${escapeHtml(formatDate(share.createdAt))}</p><h1>${escapeHtml(share.title)}</h1><p>Sanitized locally. ${redactions} redactions applied.</p></div><div class="thread">${messages}</div></article>`
  return htmlResponse(page(share.title, content))
}

function renderMessage(message: ShareMessage): string {
  return `<section class="message"><div class="role">${message.role}</div><p>${escapeHtml(message.text)}</p></section>`
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('en-US', { dateStyle: 'long', timeZone: 'UTC' }).format(
    new Date(value),
  )
}

function notFound(): Response {
  return htmlResponse(
    page(
      'Share unavailable',
      '<h1>Share unavailable</h1><p>This link is invalid or has been revoked.</p>',
    ),
    404,
  )
}
