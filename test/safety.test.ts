import { describe, expect, it } from 'vitest'
import { inspectShare } from '../src/safety.js'

describe('inspectShare', () => {
  it('accepts a minimal conversation-only share', () => {
    expect(
      inspectShare({
        schemaVersion: 'footon.share.v1',
        title: 'A safe thread',
        approvedAt: '2026-08-13T00:00:00.000Z',
        messages: [{ role: 'user', text: 'Explain bounded retries.' }],
        report: { redactions: 0, detectors: ['redact-core', 'footon-secret-patterns'] },
      }),
    ).toEqual({ ok: true })
  })

  it.each([
    ['OpenAI key', 'sk-proj-1234567890abcdefghijklmnopqrstuvwxyz'],
    ['GitHub token', 'ghp_123456789012345678901234567890123456'],
    ['private key', '-----BEGIN PRIVATE KEY-----'],
    ['email', 'alice@example.com'],
    ['home path', '/Users/alice/private/repo'],
  ])('rejects %s', (_name, text) => {
    const result = inspectShare({
      schemaVersion: 'footon.share.v1',
      title: 'Unsafe',
      approvedAt: '2026-08-13T00:00:00.000Z',
      messages: [{ role: 'assistant', text }],
      report: { redactions: 0, detectors: [] },
    })
    expect(result.ok).toBe(false)
  })

  it('rejects tool payloads and unsupported keys', () => {
    expect(
      inspectShare({
        schemaVersion: 'footon.share.v1',
        title: 'Trace',
        approvedAt: '2026-08-13T00:00:00.000Z',
        messages: [{ role: 'tool', text: 'output' }],
        report: { redactions: 0, detectors: [] },
        metadata: { cwd: '/tmp' },
      }).ok,
    ).toBe(false)
  })
})
