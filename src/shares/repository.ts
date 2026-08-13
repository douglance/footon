import type { ShareDocument, ShareRecord } from '../types.js'
import { randomToken } from '../auth/crypto.js'

interface ShareRow {
  id: string
  owner_id: string
  title: string
  document_json: string
  created_at: string
  revoked_at: string | null
}

export async function createShare(
  db: D1Database,
  ownerId: string,
  document: ShareDocument,
): Promise<ShareRecord> {
  const id = randomToken(18)
  const createdAt = new Date().toISOString()
  await db
    .prepare(
      'INSERT INTO shares (id, owner_id, title, document_json, created_at) VALUES (?, ?, ?, ?, ?)',
    )
    .bind(id, ownerId, document.title, JSON.stringify(document), createdAt)
    .run()
  return { id, ownerId, title: document.title, document, createdAt, revokedAt: null }
}

export async function sharesCreatedToday(db: D1Database, ownerId: string): Promise<number> {
  const row = await db
    .prepare(
      "SELECT COUNT(*) AS count FROM shares WHERE owner_id = ? AND created_at >= datetime('now', '-1 day')",
    )
    .bind(ownerId)
    .first<{ count: number }>()
  return row?.count ?? 0
}

export async function getShare(db: D1Database, id: string): Promise<ShareRecord | null> {
  const row = await db
    .prepare('SELECT * FROM shares WHERE id = ? AND revoked_at IS NULL')
    .bind(id)
    .first<ShareRow>()
  return row ? fromRow(row) : null
}

export async function listShares(db: D1Database, ownerId: string): Promise<ShareRecord[]> {
  const result = await db
    .prepare('SELECT * FROM shares WHERE owner_id = ? ORDER BY created_at DESC LIMIT 100')
    .bind(ownerId)
    .all<ShareRow>()
  return result.results.map(fromRow)
}

export async function revokeShare(db: D1Database, ownerId: string, id: string): Promise<boolean> {
  const result = await db
    .prepare(
      'UPDATE shares SET revoked_at = ? WHERE id = ? AND owner_id = ? AND revoked_at IS NULL',
    )
    .bind(new Date().toISOString(), id, ownerId)
    .run()
  return result.meta.changes === 1
}

function fromRow(row: ShareRow): ShareRecord {
  return {
    id: row.id,
    ownerId: row.owner_id,
    title: row.title,
    document: JSON.parse(row.document_json) as ShareDocument,
    createdAt: row.created_at,
    revokedAt: row.revoked_at,
  }
}
