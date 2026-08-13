import type { Env } from '../types.js'

interface TurnstileResponse {
  success: boolean
}

export async function verifyTurnstile(request: Request, env: Env, token: string): Promise<boolean> {
  if (!env.TURNSTILE_SECRET) return new URL(request.url).hostname === 'localhost'
  const body = new FormData()
  body.set('secret', env.TURNSTILE_SECRET)
  body.set('response', token)
  const remote = request.headers.get('CF-Connecting-IP')
  if (remote) body.set('remoteip', remote)
  const response = await fetch('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
    method: 'POST',
    body,
  })
  if (!response.ok) return false
  const result: TurnstileResponse = await response.json()
  return result.success
}
