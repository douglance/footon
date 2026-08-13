import { describe, expect, it } from 'vitest'
import { hashToken, normalizeEmail, tokenFromBytes } from '../src/auth/crypto.js'

describe('magic-link primitives', () => {
  it('normalizes email without accepting malformed input', () => {
    expect(normalizeEmail(' Doug.Lance+Test@Example.COM ')).toBe('doug.lance+test@example.com')
    expect(() => normalizeEmail('not-an-email')).toThrow('valid email')
  })

  it('encodes URL-safe tokens and stores only their hash', async () => {
    const token = tokenFromBytes(new Uint8Array([251, 255, 239, 1, 2, 3]))
    expect(token).toMatch(/^[A-Za-z0-9_-]+$/)
    expect(await hashToken(token)).toMatch(/^[a-f0-9]{64}$/)
    expect(await hashToken(token)).not.toContain(token)
  })
})
