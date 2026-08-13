import { WorkerEntrypoint } from 'cloudflare:workers'
import { createShare, listShares, revokeShare, sharesCreatedToday } from '../shares/repository.js'
import { inspectShare } from '../safety.js'
import type { AuthProps, Env, ShareDocument } from '../types.js'

export class ApiHandler extends WorkerEntrypoint<Env, AuthProps> {
  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    if (request.method === 'POST' && url.pathname === '/api/shares') return this.create(request)
    if (request.method === 'GET' && url.pathname === '/api/shares') return this.list()
    const match = /^\/api\/shares\/([A-Za-z0-9_-]{20,40})$/u.exec(url.pathname)
    if (request.method === 'DELETE' && match?.[1]) return this.revoke(match[1])
    return json({ error: 'not_found' }, 404)
  }

  private async create(request: Request): Promise<Response> {
    if ((await sharesCreatedToday(this.env.DB, this.ctx.props.userId)) >= 100) {
      return json({ error: 'daily_share_limit', message: 'Daily share limit reached.' }, 429)
    }
    const document = await parseDocument(request)
    if (!document)
      return json({ error: 'invalid_json', message: 'Send one footon.share.v2 document.' }, 400)
    const safety = inspectShare(document)
    if (!safety.ok) return json({ error: safety.code, message: safety.message }, 422)
    const share = await createShare(this.env.DB, this.ctx.props.userId, document)
    return json(
      { id: share.id, url: `https://footon.dev/s/${share.id}`, createdAt: share.createdAt },
      201,
    )
  }

  private async list(): Promise<Response> {
    const shares = await listShares(this.env.DB, this.ctx.props.userId)
    return json({ shares: shares.map(publicShare) })
  }

  private async revoke(id: string): Promise<Response> {
    const revoked = await revokeShare(this.env.DB, this.ctx.props.userId, id)
    return json({ id, revoked }, revoked ? 200 : 404)
  }
}

function publicShare(
  share: Awaited<ReturnType<typeof listShares>>[number],
): Record<string, unknown> {
  return {
    id: share.id,
    title: share.title,
    createdAt: share.createdAt,
    revokedAt: share.revokedAt,
  }
}

async function parseDocument(request: Request): Promise<ShareDocument | null> {
  if (!request.headers.get('content-type')?.toLowerCase().includes('application/json')) return null
  const body = await request.text()
  if (body.length > 1_100_000) return null
  try {
    return JSON.parse(body) as ShareDocument
  } catch {
    return null
  }
}

function json(body: unknown, status = 200): Response {
  return Response.json(body, {
    status,
    headers: { 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' },
  })
}
