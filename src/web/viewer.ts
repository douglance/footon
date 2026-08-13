import { getShare } from '../shares/repository.js'
import type { Env, ShareMessage } from '../types.js'
import { escapeHtml, htmlResponse, page } from './security.js'

export async function renderShare(env: Env, id: string): Promise<Response> {
  if (!/^[A-Za-z0-9_-]{20,40}$/u.test(id)) return notFound()
  const share = await getShare(env.DB, id)
  if (!share) return notFound()
  const transcript = renderTranscript(share.document.messages)
  const redactions = String(share.document.report.redactions)
  const content = `<article class="viewer"><div class="meta"><h1>${escapeHtml(share.title)}</h1><p class="muted">Shared ${escapeHtml(formatDate(share.createdAt))}. Sanitized locally. ${redactions} redactions.</p></div>${transcript.map}<div class="thread">${transcript.messages}</div></article><script src="/viewer.js?v=2" defer></script>`
  return htmlResponse(page(share.title, content))
}

export function renderTranscript(messages: ShareMessage[]): { map: string; messages: string } {
  const compacted = compactMessages(messages)
  const rendered = renderTimeline(compacted)
  const markers = compacted.map(renderMarker).join('')
  const map = `<nav class="minimap" aria-label="Thread minimap"><div class="map-viewport" aria-hidden="true"></div><ol>${markers}</ol></nav>`
  return { map, messages: rendered }
}

function renderTimeline(messages: ShareMessage[]): string {
  let index = 0
  let output = ''
  while (index < messages.length) {
    const message = messages[index]
    if (!message) break
    if (message.role === 'assistant') {
      const end = activityEnd(messages, index + 1)
      output += renderCall(message, messages.slice(index + 1, end), index)
      index = end
      continue
    }
    const end = activityEnd(messages, index)
    if (end > index) {
      output += renderActivityRun(messages.slice(index, end), index)
      index = end
      continue
    }
    output += renderMessage(message, index)
    index += 1
  }
  return output
}

function renderCall(message: ShareMessage, activity: ShareMessage[], index: number): string {
  const body = renderMessage(message, index)
  const tools = renderActivityRun(activity, index + 1)
  return `<section class="call-block">${body}${tools}</section>`
}

function activityEnd(messages: ShareMessage[], start: number): number {
  let end = start
  while (messages[end]?.role === 'tool' || messages[end]?.role === 'file') end += 1
  return end
}

function renderActivityRun(messages: ShareMessage[], start: number): string {
  if (messages.length === 0) return ''
  const rows = messages.map((message, offset) => renderActivity(message, start + offset)).join('')
  return `<ol class="activity-run" aria-label="Tool and file activity">${rows}</ol>`
}

function renderActivity(message: ShareMessage, index: number): string {
  const ordinal = String(index + 1)
  const position = ordinal.padStart(3, '0')
  return `<li class="message ${message.role}" id="message-${ordinal}"><div class="role"><span>${position}</span>${message.role}</div><p>${escapeHtml(message.text)}</p></li>`
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
  const position = ordinal.padStart(3, '0')
  const label = message.role === 'assistant' ? '' : message.role
  const accessible = message.role === 'assistant' ? 'assistant' : message.role
  return `<section class="message ${message.role}" id="message-${ordinal}" aria-label="${accessible} ${ordinal}"><div class="role"><span>${position}</span>${label}</div><p>${escapeHtml(message.text)}</p></section>`
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
