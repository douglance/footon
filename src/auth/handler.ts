import type { AuthRequest } from '@cloudflare/workers-oauth-provider'
import { hashToken, normalizeEmail, randomToken } from './crypto.js'
import { sendMagicLink } from './email.js'
import { authorizationIdentity } from './identity.js'
import { consentPage, loginPage, messagePage } from './pages.js'
import {
  consumeMagicLink,
  createSession,
  deletePending,
  authenticatePending,
  issueMagicLink,
  loadPending,
  readSession,
  readPendingIdentity,
  savePending,
  sessionCookie,
} from './store.js'
import { verifyTurnstile } from './turnstile.js'
import type { Env } from '../types.js'

export async function handleDefault(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url)
  if (request.method === 'GET') return handleGet(request, env, url.pathname)
  if (request.method === 'POST') return handlePost(request, env, url.pathname)
  return messagePage('Not found', 'The requested page does not exist.', 404)
}

function handleGet(request: Request, env: Env, path: string): Promise<Response> | Response {
  if (path === '/authorize') return authorizeGet(request, env)
  if (path === '/auth/verify') return verifyLink(request, env)
  return messagePage('Not found', 'The requested page does not exist.', 404)
}

function handlePost(request: Request, env: Env, path: string): Promise<Response> | Response {
  if (path === '/authorize') return authorizePost(request, env)
  if (path === '/auth/request') return requestLink(request, env)
  return messagePage('Not found', 'The requested page does not exist.', 404)
}

async function authorizeGet(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url)
  const resume = url.searchParams.get('resume')
  const oauth = resume ? await loadPending(env.DB, resume) : await parseOAuth(request, env)
  if (!oauth)
    return messagePage('Invalid request', 'The authorization request is invalid or expired.', 400)
  const pendingId = resume ?? (await savePending(env.DB, oauth))
  const [session, verified] = await Promise.all([
    readSession(env.DB, request),
    readPendingIdentity(env.DB, pendingId),
  ])
  if (!authorizationIdentity(session, verified)) return loginPage(env, pendingId)
  return showConsent(env, pendingId, oauth)
}

async function authorizePost(request: Request, env: Env): Promise<Response> {
  const form = await request.formData()
  const pendingId = stringField(form, 'pending')
  const csrf = stringField(form, 'csrf')
  if (!pendingId || !csrf || csrf !== cookie(request, '__Host-footon_csrf')) {
    return messagePage('Request expired', 'Start the connection again.', 400)
  }
  const [oauth, session, verified] = await Promise.all([
    loadPending(env.DB, pendingId),
    readSession(env.DB, request),
    readPendingIdentity(env.DB, pendingId),
  ])
  const identity = authorizationIdentity(session, verified)
  if (!oauth || !identity) return messagePage('Request expired', 'Sign in and try again.', 401)
  const { redirectTo } = await env.OAUTH_PROVIDER.completeAuthorization({
    request: oauth,
    userId: identity.userId,
    metadata: {},
    scope: oauth.scope.filter((scope) => scope === 'shares:read' || scope === 'shares:write'),
    props: identity,
  })
  await deletePending(env.DB, pendingId)
  return Response.redirect(redirectTo, 302)
}

async function requestLink(request: Request, env: Env): Promise<Response> {
  const form = await request.formData()
  const pendingId = stringField(form, 'pending')
  const challenge = stringField(form, 'cf-turnstile-response') ?? ''
  if (!pendingId || !(await loadPending(env.DB, pendingId)))
    return messagePage('Request expired', 'Start again.', 400)
  if (!(await verifyTurnstile(request, env, challenge)))
    return messagePage('Check failed', 'Try the form again.', 400)
  try {
    const email = normalizeEmail(stringField(form, 'email') ?? '')
    if (!(await withinRateLimit(env.DB, email, request)))
      return messagePage('Try later', 'Too many sign-in requests.', 429)
    const token = await issueMagicLink(env.DB, email, pendingId)
    await sendMagicLink(env, email, `${new URL(request.url).origin}/auth/verify?token=${token}`)
  } catch {
    return messagePage('Email not sent', 'Check the address and try again.', 400)
  }
  return messagePage('Check your email', 'Open the one-time link within 10 minutes.')
}

async function verifyLink(request: Request, env: Env): Promise<Response> {
  const token = new URL(request.url).searchParams.get('token')
  const magic = token ? await consumeMagicLink(env.DB, token) : null
  if (!magic) return messagePage('Link expired', 'Request a new sign-in link.', 400)
  await authenticatePending(env.DB, magic.pendingId, magic.email)
  const session = await createSession(env.DB, magic.email)
  return new Response(null, {
    status: 302,
    headers: {
      location: `${new URL(request.url).origin}/authorize?resume=${magic.pendingId}`,
      'set-cookie': sessionCookie(session),
    },
  })
}

async function showConsent(env: Env, pendingId: string, oauth: AuthRequest): Promise<Response> {
  const client = await env.OAUTH_PROVIDER.lookupClient(oauth.clientId)
  if (!client) return messagePage('Unknown client', 'This MCP client is not registered.', 400)
  return consentPage(pendingId, randomToken(18), client)
}

async function parseOAuth(request: Request, env: Env): Promise<AuthRequest | null> {
  try {
    return await env.OAUTH_PROVIDER.parseAuthRequest(request)
  } catch {
    return null
  }
}

async function withinRateLimit(db: D1Database, email: string, request: Request): Promise<boolean> {
  const key = await hashToken(`${email}:${request.headers.get('CF-Connecting-IP') ?? 'unknown'}`)
  const row = await db
    .prepare(
      "SELECT COUNT(*) AS count FROM auth_attempts WHERE rate_key = ? AND created_at >= datetime('now', '-1 hour')",
    )
    .bind(key)
    .first<{ count: number }>()
  if ((row?.count ?? 0) >= 5) return false
  await db
    .prepare('INSERT INTO auth_attempts (rate_key, created_at) VALUES (?, ?)')
    .bind(key, new Date().toISOString())
    .run()
  return true
}

function stringField(form: FormData, name: string): string | null {
  const value = form.get(name)
  return typeof value === 'string' ? value : null
}

function cookie(request: Request, name: string): string | undefined {
  return request.headers
    .get('cookie')
    ?.split(';')
    .map((part) => part.trim().split('='))
    .find(([key]) => key === name)?.[1]
}
