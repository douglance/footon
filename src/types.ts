import type { OAuthHelpers } from '@cloudflare/workers-oauth-provider'

export interface Env {
  DB: D1Database
  OAUTH_KV: KVNamespace
  OAUTH_PROVIDER: OAuthHelpers
  EMAIL: SendEmail
  TURNSTILE_SECRET: string
  TURNSTILE_SITE_KEY: string
}

export interface AuthProps {
  userId: string
  email: string
}

export interface ShareMessage {
  role: 'user' | 'assistant' | 'tool' | 'file'
  text: string
}

export interface ShareDocument {
  schemaVersion: 'footon.share.v1' | 'footon.share.v2'
  title: string
  approvedAt: string
  messages: ShareMessage[]
  report: { redactions: number; detectors: string[] }
}

export interface ShareRecord {
  id: string
  ownerId: string
  title: string
  document: ShareDocument
  createdAt: string
  revokedAt: string | null
}
