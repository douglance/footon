import { describe, expect, it } from 'vitest'
import { handleMcp } from '../src/mcp/protocol.js'

describe('MCP protocol', () => {
  it('initializes against the current protocol', async () => {
    const response = await handleMcp(
      new Request('https://footon.dev/mcp', {
        method: 'POST',
        headers: { 'content-type': 'application/json', accept: 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          method: 'initialize',
          params: {
            protocolVersion: '2026-07-28',
            capabilities: {},
            clientInfo: { name: 'test', version: '1' },
          },
        }),
      }),
      { userId: 'user-1', email: 'owner@example.com' },
      {} as never,
    )
    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      result: { protocolVersion: '2026-07-28', serverInfo: { name: 'footon' } },
    })
  })

  it('advertises only the safe share lifecycle tools', async () => {
    const response = await handleMcp(
      new Request('https://footon.dev/mcp', {
        method: 'POST',
        headers: { 'content-type': 'application/json', accept: 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }),
      }),
      { userId: 'user-1', email: 'owner@example.com' },
      {} as never,
    )
    const body: { result: { tools: { name: string }[] } } = await response.json()
    expect(body.result.tools.map((tool) => tool.name)).toEqual([
      'share_create',
      'share_list',
      'share_revoke',
    ])
  })
})
