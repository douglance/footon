import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { createTestHarness, type TestHarness } from 'wrangler'

const server: TestHarness = createTestHarness({
  root: process.cwd(),
  workers: [{ configPath: './wrangler.jsonc' }],
})
type HarnessResponse = Awaited<ReturnType<TestHarness['fetch']>>

beforeAll(async () => {
  await server.listen()
  await server.getWorker().applyD1Migrations('DB')
})

afterAll(async () => server.close())

describe('OAuth magic-link E2E', () => {
  it('authorizes without a session cookie and calls every share tool', async () => {
    const clientId = await registerClient()
    const authorization = await beginAuthorization(clientId)
    await requestMagicLink(authorization.pending)
    const consent = await verifyMagicLink()
    const code = await approveWithoutSession(consent)
    const token = await exchangeCode(clientId, code, authorization.verifier)
    const created = await callTool(token, 'share_create', { document: safeDocument() })
    const id = resultData(created).id as string

    expect(resultData(await callTool(token, 'share_list', {})).shares).toEqual([
      expect.objectContaining({ id, revokedAt: null }),
    ])
    expect(resultData(await callTool(token, 'share_revoke', { id }))).toEqual({
      id,
      revoked: true,
    })
  })
})

async function registerClient(): Promise<string> {
  const response = await postJson('/oauth/register', {
    client_name: 'Footon E2E',
    redirect_uris: ['http://127.0.0.1/callback'],
    token_endpoint_auth_method: 'none',
    grant_types: ['authorization_code'],
    response_types: ['code'],
  })
  expect(response.status).toBe(201)
  return requiredString(await response.json(), 'client_id')
}

async function beginAuthorization(
  clientId: string,
): Promise<{ pending: string; verifier: string }> {
  const verifier = 'footon-e2e-verifier-with-more-than-forty-three-characters'
  const query = authorizationQuery(clientId, verifier)
  const response = await server.fetch(`http://localhost/authorize?${query}`)
  expect(response.status).toBe(200)
  return { pending: requiredField(await response.text(), 'pending'), verifier }
}

async function requestMagicLink(pending: string): Promise<void> {
  server.clearLogs()
  const response = await postForm('/auth/request', { pending, email: 'agent@example.com' })
  expect(response.status).toBe(200)
  expect(await response.text()).toContain('Check your email')
}

async function verifyMagicLink(): Promise<{ pending: string; csrf: string; cookie: string }> {
  const artifact = /Text: ([^\s]+\.txt)/u.exec(logText())?.[1]
  expect(artifact).toBeTruthy()
  const email = await readFile(artifact ?? '', 'utf8')
  const link = /http:\/\/localhost\/auth\/verify\?token=[A-Za-z0-9_-]+/u.exec(email)?.[0]
  expect(link).toBeTruthy()
  const verified = await server.fetch(link ?? '', { redirect: 'manual' })
  expect(verified.status).toBe(302)
  const consent = await server.fetch(requiredHeader(verified, 'location'))
  expect(consent.status).toBe(200)
  const html = await consent.text()
  return {
    pending: requiredField(html, 'pending'),
    csrf: requiredField(html, 'csrf'),
    cookie: requiredHeader(consent, 'set-cookie').split(';', 1)[0] ?? '',
  }
}

async function approveWithoutSession(consent: {
  pending: string
  csrf: string
  cookie: string
}): Promise<string> {
  const response = await postForm(
    '/authorize',
    { pending: consent.pending, csrf: consent.csrf },
    { cookie: consent.cookie },
  )
  expect(response.status).toBe(302)
  const callback = new URL(requiredHeader(response, 'location'))
  expect(callback.searchParams.get('state')).toBe('footon-e2e-state')
  return callback.searchParams.get('code') ?? ''
}

async function exchangeCode(clientId: string, code: string, verifier: string): Promise<string> {
  const response = await postForm('/oauth/token', {
    grant_type: 'authorization_code',
    client_id: clientId,
    redirect_uri: 'http://127.0.0.1/callback',
    code,
    code_verifier: verifier,
    resource: 'https://footon.dev/mcp',
  })
  expect(response.status).toBe(200)
  return requiredString(await response.json(), 'access_token')
}

async function callTool(
  token: string,
  name: string,
  args: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  const response = await server.getWorker().fetch('https://footon.dev/mcp', {
    method: 'POST',
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: name,
      method: 'tools/call',
      params: { name, arguments: args },
    }),
  })
  expect(response.status, await response.clone().text()).toBe(200)
  return (await response.json()) as Record<string, unknown>
}

function authorizationQuery(clientId: string, verifier: string): string {
  return new URLSearchParams({
    response_type: 'code',
    client_id: clientId,
    redirect_uri: 'http://127.0.0.1/callback',
    scope: 'shares:read shares:write',
    state: 'footon-e2e-state',
    code_challenge: createHash('sha256').update(verifier).digest('base64url'),
    code_challenge_method: 'S256',
    resource: 'https://footon.dev/mcp',
  }).toString()
}

function safeDocument(): Record<string, unknown> {
  return {
    schemaVersion: 'footon.share.v1',
    title: 'E2E safe thread',
    approvedAt: new Date().toISOString(),
    messages: [{ role: 'user', text: 'Share this safe message.' }],
    report: { redactions: 0, detectors: ['footon-e2e'] },
  }
}

async function postJson(path: string, body: unknown): Promise<HarnessResponse> {
  return server.fetch(`http://localhost${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
}

async function postForm(
  path: string,
  body: Record<string, string>,
  headers: Record<string, string> = {},
): Promise<HarnessResponse> {
  return server.fetch(`http://localhost${path}`, {
    method: 'POST',
    headers: { ...headers, 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams(body).toString(),
    redirect: 'manual',
  })
}

function requiredField(html: string, name: string): string {
  const value = new RegExp(`name="${name}" value="([^"]+)"`, 'u').exec(html)?.[1]
  expect(value).toBeTruthy()
  return value ?? ''
}

function requiredHeader(response: HarnessResponse, name: string): string {
  const value = response.headers.get(name)
  expect(value).toBeTruthy()
  return value ?? ''
}

function logText(): string {
  return server
    .getLogs()
    .map((entry) => JSON.stringify(entry))
    .join('\n')
}

function resultData(response: Record<string, unknown>): Record<string, unknown> {
  const result = response.result as { structuredContent?: Record<string, unknown> }
  expect(result.structuredContent).toBeTruthy()
  return result.structuredContent ?? {}
}

function requiredString(value: unknown, key: string): string {
  const result =
    typeof value === 'object' && value !== null
      ? (value as Record<string, unknown>)[key]
      : undefined
  expect(result).toBeTypeOf('string')
  return typeof result === 'string' ? result : ''
}
