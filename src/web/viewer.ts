import { getShare } from '../shares/repository.js'
import MarkdownIt from 'markdown-it'
import type { Env, ShareMessage } from '../types.js'
import { escapeHtml, htmlResponse, page } from './security.js'

const markdown = new MarkdownIt('commonmark', {
  html: false,
  linkify: false,
  typographer: false,
})

const defaultValidateLink = markdown.validateLink.bind(markdown)
markdown.validateLink = (url: string) =>
  defaultValidateLink(url) && !/^\s*(?:javascript|vbscript):/iu.test(url)
markdown.disable('image')

export async function renderShare(env: Env, id: string): Promise<Response> {
  if (!/^[A-Za-z0-9_-]{20,40}$/u.test(id)) return notFound()
  const share = await getShare(env.DB, id)
  if (!share) return notFound()
  const transcript = renderTranscript(share.document.messages)
  const redactions = String(share.document.report.redactions)
  const content = `<article class="viewer"><div class="meta"><h1>${escapeHtml(share.title)}</h1><p class="muted">Shared ${escapeHtml(formatDate(share.createdAt))}. Sanitized locally. ${redactions} redactions.</p></div>${transcript.map}<div class="thread">${transcript.messages}</div></article><script src="/viewer.js?v=4" defer></script>`
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
    const filtered = filterMessageText(message)
    if (!filtered.trim()) continue
    const previous = compacted.at(-1)
    if (message.role === 'assistant' && previous?.role === 'assistant') {
      if (!seen.has(filtered)) previous.text += `\n\n${filtered}`
      seen.add(filtered)
      continue
    }
    compacted.push({ ...message, text: filtered })
    seen = message.role === 'assistant' ? new Set([filtered]) : new Set()
  }
  return compacted
}

function filterMessageText(message: ShareMessage): string {
  return message.role === 'user' ? filterInjectedBlocks(message.text) : message.text
}

function renderMessage(message: ShareMessage, index: number): string {
  const ordinal = String(index + 1)
  const position = ordinal.padStart(3, '0')
  const label = message.role === 'assistant' ? 'AGENT' : message.role
  const accessible = message.role === 'assistant' ? 'agent' : message.role
  return `<section class="message ${message.role}" id="message-${ordinal}" aria-label="${accessible} ${ordinal}"><div class="role"><span>${position}</span>${label}</div>${renderProse(message, ordinal)}</section>`
}

function renderMarker(message: ShareMessage, index: number): string {
  const ordinal = String(index + 1)
  const role = message.role === 'assistant' ? 'agent' : message.role
  const label = `Jump to ${role} message ${ordinal}`
  if (message.role === 'user') {
    return `<li><a class="map-marker user" href="#message-${ordinal}" aria-label="${label}" title="${label}"></a></li>`
  }
  return `<li><span class="map-marker ${message.role}" data-message-id="message-${ordinal}" title="${label}"></span></li>`
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

function renderProse(message: ShareMessage, ordinal: string): string {
  const plain = `<pre class="message-text">${escapeHtml(message.text)}</pre>`
  const rendered = renderProseMarkdown(message.text)
  if (!rendered.ok) return `<div class="message-body">${plain}</div>`
  const id = `message-view-${ordinal}`
  const role = message.role === 'assistant' ? 'agent' : message.role
  return `<div class="message-body"><input class="view-toggle" id="${id}" type="checkbox" aria-label="Show source text for ${role} message ${ordinal}"><label class="render-toggle" for="${id}"><span>Rendered</span><span> | </span><span>Text</span></label><div class="rendered">${rendered.html}</div>${plain}</div>`
}

export function renderProseMarkdown(
  text: string,
  renderer: InstanceType<typeof MarkdownIt> = markdown,
): { ok: true; html: string } | { ok: false } {
  try {
    const tokens = renderer.parse(text, {})
    for (const token of tokens) {
      if (token.type === 'heading_open' || token.type === 'heading_close') {
        token.tag = shiftHeading(token.tag)
      }
    }
    return { ok: true, html: renderer.renderer.render(tokens, renderer.options, {}) }
  } catch {
    return { ok: false }
  }
}

function shiftHeading(tag: string): string {
  const level = Number(tag.slice(1))
  if (!Number.isInteger(level)) return tag
  return `h${String(Math.min(level + 1, 6))}`
}

function filterInjectedBlocks(text: string): string {
  let filtered = text
  for (const heading of [
    '# AGENTS.md instructions',
    '# [DOMAIN_NAME] instructions',
    '# Domain instructions',
    '# domain instructions',
  ]) {
    filtered = stripInstructionBlocks(filtered, heading)
  }
  for (const tag of ['recommended_plugins', 'environment_context', 'codex_internal_context']) {
    filtered = stripTaggedBlocks(filtered, tag)
  }
  if (filtered === text) return text
  return collapseBlankLines(filtered.trim())
}

function stripInstructionBlocks(text: string, heading: string): string {
  let output = text
  for (;;) {
    const start = output.indexOf(heading)
    if (start === -1) return output
    const afterHeading = start + heading.length
    const rest = output.slice(afterHeading)
    const match = /^\s*<INSTRUCTIONS>[\s\S]*?<\/INSTRUCTIONS>/u.exec(rest)
    if (!match?.[0]) return output
    output = output.slice(0, start) + output.slice(afterHeading + match[0].length)
  }
}

function stripTaggedBlocks(text: string, tag: string): string {
  const pattern = new RegExp(`<${tag}(?:\\s[^>]*)?>[\\s\\S]*?<\\/${tag}>`, 'gu')
  return text.replace(pattern, '')
}

function collapseBlankLines(text: string): string {
  return text.replace(/\n{3,}/gu, '\n\n')
}
