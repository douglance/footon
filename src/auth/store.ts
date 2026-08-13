import type { AuthRequest } from '@cloudflare/workers-oauth-provider'
import { hashToken, randomToken, userId } from './crypto.js'

export async function savePending(db: D1Database, request: AuthRequest): Promise<string> {
  const id = randomToken(18)
  await db
    .prepare('INSERT INTO oauth_pending (id, request_json, expires_at) VALUES (?, ?, ?)')
    .bind(id, JSON.stringify(request), expires(10))
    .run()
  return id
}

export async function loadPending(db: D1Database, id: string): Promise<AuthRequest | null> {
  const row = await db
    .prepare('SELECT request_json FROM oauth_pending WHERE id = ? AND expires_at > ?')
    .bind(id, now())
    .first<{ request_json: string }>()
  return row ? (JSON.parse(row.request_json) as AuthRequest) : null
}

export async function deletePending(db: D1Database, id: string): Promise<void> {
  await db.prepare('DELETE FROM oauth_pending WHERE id = ?').bind(id).run()
}

export async function deleteExpiredAuth(db: D1Database): Promise<void> {
  const timestamp = now()
  await db.batch([
    db.prepare('DELETE FROM oauth_pending WHERE expires_at <= ?').bind(timestamp),
    db.prepare('DELETE FROM magic_links WHERE expires_at <= ?').bind(timestamp),
    db.prepare('DELETE FROM sessions WHERE expires_at <= ?').bind(timestamp),
    db.prepare("DELETE FROM auth_attempts WHERE created_at <= datetime('now', '-1 day')").bind(),
  ])
}

export async function issueMagicLink(
  db: D1Database,
  email: string,
  pendingId: string,
): Promise<string> {
  const token = randomToken()
  await db
    .prepare(
      'INSERT INTO magic_links (token_hash, email, pending_id, expires_at) VALUES (?, ?, ?, ?)',
    )
    .bind(await hashToken(token), email, pendingId, expires(10))
    .run()
  return token
}

export async function consumeMagicLink(
  db: D1Database,
  token: string,
): Promise<{ email: string; pendingId: string } | null> {
  const tokenHash = await hashToken(token)
  const row = await db
    .prepare('SELECT email, pending_id FROM magic_links WHERE token_hash = ? AND expires_at > ?')
    .bind(tokenHash, now())
    .first<{ email: string; pending_id: string }>()
  if (!row) return null
  await db.prepare('DELETE FROM magic_links WHERE token_hash = ?').bind(tokenHash).run()
  return { email: row.email, pendingId: row.pending_id }
}

export async function createSession(db: D1Database, email: string): Promise<string> {
  const token = randomToken()
  await db
    .prepare('INSERT INTO sessions (token_hash, user_id, email, expires_at) VALUES (?, ?, ?, ?)')
    .bind(await hashToken(token), await userId(email), email, expires(30 * 24 * 60))
    .run()
  return token
}

export async function readSession(
  db: D1Database,
  request: Request,
): Promise<{ userId: string; email: string } | null> {
  const token = cookie(request, '__Host-footon_session')
  if (!token) return null
  const row = await db
    .prepare('SELECT user_id, email FROM sessions WHERE token_hash = ? AND expires_at > ?')
    .bind(await hashToken(token), now())
    .first<{ user_id: string; email: string }>()
  return row ? { userId: row.user_id, email: row.email } : null
}

export function sessionCookie(token: string): string {
  return `__Host-footon_session=${token}; HttpOnly; Secure; Path=/; SameSite=Lax; Max-Age=2592000`
}

function cookie(request: Request, name: string): string | undefined {
  return request.headers
    .get('cookie')
    ?.split(';')
    .map((part) => part.trim().split('='))
    .find(([key]) => key === name)?.[1]
}

function now(): string {
  return new Date().toISOString()
}

function expires(minutes: number): string {
  return new Date(Date.now() + minutes * 60_000).toISOString()
}
