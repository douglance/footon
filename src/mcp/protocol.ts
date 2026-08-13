import { createShare, listShares, revokeShare } from '../shares/repository.js'
import { inspectShare } from '../safety.js'
import type { AuthProps, Env, ShareDocument } from '../types.js'
import { TOOL_DEFINITIONS } from './tools.js'

interface RpcRequest {
  jsonrpc: string
  id?: string | number
  method?: string
  params?: unknown
}

export async function handleMcp(request: Request, auth: AuthProps, env: Env): Promise<Response> {
  if (request.method !== 'POST') return new Response('Method not allowed', { status: 405 })
  if (!isJson(request)) {
    return rpcHttpError(null, -32600, 'Content-Type must be application/json', 415)
  }
  const body = await readRequest(request)
  if (!body) return rpcHttpError(null, -32700, 'Parse error', 400)
  if (body.jsonrpc !== '2.0' || body.id === undefined || !body.method) {
    return rpcHttpError(body.id ?? null, -32600, 'Invalid Request', 400)
  }
  const result = await dispatch(body, auth, env)
  return Response.json({ jsonrpc: '2.0', id: body.id, ...result }, { headers: noStoreHeaders() })
}

async function dispatch(
  body: RpcRequest,
  auth: AuthProps,
  env: Env,
): Promise<Record<string, unknown>> {
  if (body.method === 'initialize') return { result: initialize(body.params) }
  if (body.method === 'ping') return { result: {} }
  if (body.method === 'tools/list') return { result: { tools: TOOL_DEFINITIONS } }
  if (body.method === 'tools/call') return await callTool(body.params, auth, env)
  return { error: { code: -32601, message: 'Method not found' } }
}

function initialize(params: unknown): Record<string, unknown> {
  const requested =
    isObject(params) && typeof params.protocolVersion === 'string' ? params.protocolVersion : ''
  const protocolVersion = ['2026-07-28', '2025-11-25'].includes(requested)
    ? requested
    : '2026-07-28'
  return {
    protocolVersion,
    capabilities: { tools: { listChanged: false } },
    serverInfo: { name: 'footon', version: '0.1.0' },
    instructions:
      'Publish only documents produced and explicitly approved by the local footon sanitizer.',
  }
}

async function callTool(
  params: unknown,
  auth: AuthProps,
  env: Env,
): Promise<Record<string, unknown>> {
  if (!isObject(params) || typeof params.name !== 'string' || !isObject(params.arguments)) {
    return toolError('Invalid tool call')
  }
  try {
    if (params.name === 'share_create') return await create(params.arguments.document, auth, env)
    if (params.name === 'share_list') return await list(auth, env)
    if (params.name === 'share_revoke') return await revoke(params.arguments.id, auth, env)
    return toolError('Unknown tool')
  } catch {
    return toolError('The share operation failed. Retry with the same sanitized document.')
  }
}

async function create(
  document: unknown,
  auth: AuthProps,
  env: Env,
): Promise<Record<string, unknown>> {
  const safety = inspectShare(document)
  if (!safety.ok) return toolError(`Rejected: ${safety.message}`)
  const share = await createShare(env.DB, auth.userId, document as ShareDocument)
  return toolResult({
    id: share.id,
    url: `https://footon.dev/s/${share.id}`,
    createdAt: share.createdAt,
  })
}

async function list(auth: AuthProps, env: Env): Promise<Record<string, unknown>> {
  const shares = await listShares(env.DB, auth.userId)
  return toolResult({
    shares: shares.map((share) => ({
      id: share.id,
      title: share.title,
      createdAt: share.createdAt,
      revokedAt: share.revokedAt,
    })),
  })
}

async function revoke(id: unknown, auth: AuthProps, env: Env): Promise<Record<string, unknown>> {
  if (typeof id !== 'string') return toolError('A share id is required')
  return toolResult({ id, revoked: await revokeShare(env.DB, auth.userId, id) })
}

function toolResult(data: unknown): Record<string, unknown> {
  return {
    result: { content: [{ type: 'text', text: JSON.stringify(data) }], structuredContent: data },
  }
}

function toolError(message: string): Record<string, unknown> {
  return { result: { content: [{ type: 'text', text: message }], isError: true } }
}

async function readRequest(request: Request): Promise<RpcRequest | null> {
  const text = await request.text()
  if (text.length > 1_100_000) return null
  try {
    return JSON.parse(text) as RpcRequest
  } catch {
    return null
  }
}

function rpcHttpError(id: unknown, code: number, message: string, status: number): Response {
  return Response.json(
    { jsonrpc: '2.0', id, error: { code, message } },
    { status, headers: noStoreHeaders() },
  )
}

function noStoreHeaders(): HeadersInit {
  return { 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isJson(request: Request): boolean {
  return request.headers.get('content-type')?.toLowerCase().includes('application/json') ?? false
}
