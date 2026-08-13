import { WorkerEntrypoint } from 'cloudflare:workers'
import type { AuthProps, Env } from '../types.js'
import { handleMcp } from './protocol.js'

export class McpHandler extends WorkerEntrypoint<Env, AuthProps> {
  override fetch(request: Request): Promise<Response> {
    return handleMcp(request, this.ctx.props, this.env)
  }
}
