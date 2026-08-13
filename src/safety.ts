export type SafetyResult = { ok: true } | { ok: false; code: string; message: string }

interface InspectedRoot {
  schemaVersion: 'footon.share.v1' | 'footon.share.v2'
  title: string
  messages: unknown[]
}

const ROOT_KEYS = ['approvedAt', 'messages', 'report', 'schemaVersion', 'title']
const MESSAGE_KEYS = ['role', 'text']
const REPORT_KEYS = ['detectors', 'redactions']
const SECRET_PATTERNS: RegExp[] = [
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/u,
  /\bsk-(?:proj-|live-)?[A-Za-z0-9_-]{20,}\b/u,
  /\b(?:ghp|github_pat|glpat|xox[baprs])_[A-Za-z0-9_-]{20,}\b/u,
  /\bAKIA[0-9A-Z]{16}\b/u,
  /\bAIza[0-9A-Za-z_-]{30,}\b/u,
  /\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/u,
  /(?:postgres|mysql|mongodb(?:\+srv)?):\/\/[^\s:@]+:[^\s@]+@/iu,
  /\b(?:api[_-]?key|client[_-]?secret|password)\s*[:=]\s*[^\s]{8,}/iu,
]
const PII_PATTERNS: RegExp[] = [
  /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/iu,
  /\b\d{3}-\d{2}-\d{4}\b/u,
  /\b(?:\d[ -]*?){13,19}\b/u,
  /(?:\/Users\/|\/home\/|[A-Z]:\\Users\\)[^\s/\\]+/u,
]

export function inspectShare(value: unknown): SafetyResult {
  const root = inspectRoot(value)
  if (!root.ok) return root
  const share = value as InspectedRoot
  let bytes = textBytes(share.title)
  for (const message of share.messages) {
    const result = inspectMessage(message, share.schemaVersion)
    if (!result.ok) return result
    bytes += textBytes((message as { text: string }).text)
  }
  if (bytes > 1_000_000) return unsafe('size', 'Share text exceeds 1 MB')
  return scanText(share.title) ?? { ok: true }
}

function inspectRoot(value: unknown): SafetyResult {
  if (!isObject(value) || !exactKeys(value, ROOT_KEYS))
    return unsafe('shape', 'Invalid share shape')
  if (!validSchema(value.schemaVersion)) return unsafe('schema', 'Unsupported schema')
  if (!validTitle(value.title) || !validDate(value.approvedAt))
    return unsafe('fields', 'Invalid share fields')
  if (!validMessages(value.messages)) return unsafe('messages', 'A share needs 1 to 2000 items')
  if (!validReport(value.report)) return unsafe('report', 'Invalid sanitization report')
  return { ok: true }
}

function validMessages(value: unknown): value is unknown[] {
  return Array.isArray(value) && value.length > 0 && value.length <= 2_000
}

function inspectMessage(value: unknown, schema: InspectedRoot['schemaVersion']): SafetyResult {
  if (!isObject(value) || !exactKeys(value, MESSAGE_KEYS))
    return unsafe('message_shape', 'Invalid message')
  if (!validRole(value.role, schema)) return unsafe('role', 'Unsupported transcript role')
  if (!validMessageText(value.text)) {
    return unsafe('message_text', 'Invalid message text')
  }
  if (!validActivity(value.role, value.text)) return unsafe('activity', 'Invalid activity summary')
  return scanText(value.text) ?? { ok: true }
}

function validSchema(value: unknown): value is InspectedRoot['schemaVersion'] {
  return value === 'footon.share.v1' || value === 'footon.share.v2'
}

function validRole(value: unknown, schema: InspectedRoot['schemaVersion']): boolean {
  if (value === 'user' || value === 'assistant') return true
  return schema === 'footon.share.v2' && (value === 'tool' || value === 'file')
}

function validActivity(role: unknown, text: string): boolean {
  if (role === 'tool') return /^[A-Za-z0-9_.:-]{1,80}$/u.test(text)
  if (role === 'file') return /^(?:add|update|delete) [A-Za-z0-9][A-Za-z0-9._-]{0,119}$/u.test(text)
  return true
}

function validMessageText(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 100_000
}

function scanText(text: string): SafetyResult | undefined {
  if (SECRET_PATTERNS.some((pattern) => pattern.test(text)))
    return unsafe('secret', 'Possible secret remains')
  if (PII_PATTERNS.some((pattern) => pattern.test(text)))
    return unsafe('pii', 'Possible personal data remains')
  if (containsControl(text)) return unsafe('control', 'Control characters are not allowed')
  return undefined
}

function validReport(value: unknown): boolean {
  return (
    isObject(value) &&
    exactKeys(value, REPORT_KEYS) &&
    Number.isSafeInteger(value.redactions) &&
    typeof value.redactions === 'number' &&
    value.redactions >= 0 &&
    validDetectors(value.detectors)
  )
}

function validDetectors(value: unknown): boolean {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.length <= 16 &&
    value.every((item) => typeof item === 'string' && item.length <= 80)
  )
}

function validTitle(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 160
}

function validDate(value: unknown): value is string {
  return typeof value === 'string' && Number.isFinite(Date.parse(value))
}

function exactKeys(value: Record<string, unknown>, expected: string[]): boolean {
  return Object.keys(value).sort().join('\0') === expected.join('\0')
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function containsControl(value: string): boolean {
  for (const character of value) {
    if (character < ' ' && character !== '\n' && character !== '\t') return true
  }
  return false
}

function textBytes(value: string): number {
  return new TextEncoder().encode(value).byteLength
}

function unsafe(code: string, message: string): SafetyResult {
  return { ok: false, code, message }
}
