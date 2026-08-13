import { getShare } from '../shares/repository.js'
import type { Env, ShareMessage } from '../types.js'
import { escapeHtml, htmlResponse, page } from './security.js'

export async function renderShare(env: Env, id: string): Promise<Response> {
  if (!/^[A-Za-z0-9_-]{20,40}$/u.test(id)) return notFound()
  const share = await getShare(env.DB, id)
  if (!share) return notFound()
  const transcript = renderTranscript(share.document.messages)
  const redactions = String(share.document.report.redactions)
  const content = `<article class="viewer"><div class="meta"><p class="prompt">$ footon view --unlisted</p><h1>${escapeHtml(share.title)}</h1><p class="muted">shared ${escapeHtml(formatDate(share.createdAt))} // sanitized locally // ${redactions} redactions</p></div>${transcript.map}<div class="thread">${transcript.messages}</div></article>`
  return htmlResponse(page(share.title, content))
}

export function renderTranscript(messages: ShareMessage[]): { map: string; messages: string } {
  const compacted = compactMessages(messages)
  const rendered = renderTimeline(compacted)
  const markers = compacted.map(renderMarker).join('')
  const count = String(compacted.length)
  const map = `<nav class="minimap" aria-label="Thread minimap"><div class="map-head">map <span>${count}</span></div><ol>${markers}</ol><div class="map-key"><i class="user-key"></i>user <i class="tool-key"></i>tool <i class="file-key"></i>file</div></nav>`
  return { map, messages: rendered }
}

function renderTimeline(messages: ShareMessage[]): string {
  let call = 0
  return messages
    .map((message, index) => {
      const boundary = message.role === 'assistant' ? renderCallBoundary((call += 1)) : ''
      return boundary + renderMessage(message, index)
    })
    .join('')
}

function renderCallBoundary(call: number): string {
  const label = String(call).padStart(2, '0')
  return `<div class="call-break" role="separator" aria-label="LLM call ${label}"><span>llm_call ${label}</span></div>`
}

export function compactMessages(messages: ShareMessage[]): ShareMessage[] {
  const compacted: ShareMessage[] = []
  let seen = new Set<string>()
  for (const message of messages) {
    const previous = compacted.at(-1)
    if (message.role === 'assistant' && previous?.role === 'assistant') {
      if (!seen.has(message.text)) previous.text += `\n\n${message.text}`
      seen.add(message.text)
      continue
    }
    compacted.push({ ...message })
    seen = message.role === 'assistant' ? new Set([message.text]) : new Set()
  }
  return compacted
}

function renderMessage(message: ShareMessage, index: number): string {
  const ordinal = String(index + 1)
  const position = ordinal.padStart(2, '0')
  return `<section class="message ${message.role}" id="message-${ordinal}"><div class="role"><span>${position}</span>${message.role}</div><p>${escapeHtml(message.text)}</p></section>`
}

function renderMarker(message: ShareMessage, index: number): string {
  const ordinal = String(index + 1)
  const label = `Jump to ${message.role} message ${ordinal}`
  return `<li><a class="map-marker ${message.role}" href="#message-${ordinal}" aria-label="${label}" title="${label}"></a></li>`
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
