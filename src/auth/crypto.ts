const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/u
const RUNTIME_CRYPTO = crypto

export function normalizeEmail(input: string): string {
  const email = input.trim().toLowerCase()
  if (email.length > 254 || !EMAIL.test(email)) throw new Error('Enter a valid email address')
  return email
}

export function randomToken(bytes = 32): string {
  const value = new Uint8Array(bytes)
  RUNTIME_CRYPTO.getRandomValues(value)
  return tokenFromBytes(value)
}

export function tokenFromBytes(bytes: Uint8Array): string {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  let binary = ''
  let index = 0
  while (index < view.byteLength) {
    binary += String.fromCharCode(view.getUint8(index))
    index += 1
  }
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

export async function hashToken(token: string): Promise<string> {
  const digest = await RUNTIME_CRYPTO.subtle.digest('SHA-256', new TextEncoder().encode(token))
  const view = new DataView(digest)
  let hex = ''
  let index = 0
  while (index < view.byteLength) {
    hex += view.getUint8(index).toString(16).padStart(2, '0')
    index += 1
  }
  return hex
}

export async function userId(email: string): Promise<string> {
  return `usr_${(await hashToken(email)).slice(0, 32)}`
}
