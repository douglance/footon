import { OAuthProvider } from '@cloudflare/workers-oauth-provider'
import { ApiHandler } from './api/handler.js'
import { handleDefault } from './auth/handler.js'
import { deleteExpiredAuth } from './auth/store.js'
import { McpHandler } from './mcp/handler.js'
import type { Env } from './types.js'
import { renderConnect, renderCss, renderHome, renderInstall } from './web/home.js'
import { renderMinimapScript } from './web/minimap.js'
import { renderShare } from './web/viewer.js'

const browserHandler: ExportedHandler<Env> = {
  async fetch(request, env) {
    const url = new URL(request.url)
    const staticPage = request.method === 'GET' ? renderStatic(url.pathname) : null
    if (staticPage) return staticPage
    const share = /^\/s\/([A-Za-z0-9_-]{20,40})$/u.exec(url.pathname)
    if (request.method === 'GET' && share?.[1]) return renderShare(env, share[1])
    return handleDefault(request, env)
  },
}

function renderStatic(path: string): Response | null {
  if (path === '/') return renderHome()
  if (path === '/install') return renderInstall()
  if (path === '/connect') return renderConnect()
  if (path === '/style.css') return renderCss()
  if (path === '/viewer.js') return renderMinimapScript()
  return null
}

const provider = new OAuthProvider<Env>({
  apiHandlers: { '/mcp': McpHandler, '/api': ApiHandler },
  defaultHandler: browserHandler,
  authorizeEndpoint: '/authorize',
  tokenEndpoint: '/oauth/token',
  clientRegistrationEndpoint: '/oauth/register',
  clientIdMetadataDocumentEnabled: true,
  allowPlainPKCE: false,
  disallowPublicClientRegistration: false,
  scopesSupported: ['shares:read', 'shares:write'],
  resourceMetadata: {
    resource: 'https://footon.dev/mcp',
    authorization_servers: ['https://footon.dev'],
    scopes_supported: ['shares:read', 'shares:write'],
    bearer_methods_supported: ['header'],
    resource_name: 'footon safe thread sharing',
  },
})

export default {
  fetch: provider.fetch.bind(provider),
  scheduled: async (_event: ScheduledEvent, env: Env): Promise<void> => {
    await Promise.all([
      provider.purgeExpiredData(env, { batchSize: 100 }),
      deleteExpiredAuth(env.DB),
    ])
  },
}

export { ApiHandler, McpHandler }
