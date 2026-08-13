import { describe, expect, it } from 'vitest'
import { CSS } from '../src/web/style.js'

describe('minimap styling', () => {
  it('uses narrow rails with full role-color fills', () => {
    expect(CSS).toContain('width: 48px;')
    expect(CSS).toContain('width: 10px;')
    expect(CSS).toContain('.map-marker.user { background: var(--green); }')
    expect(CSS).toContain('.map-marker.assistant { background: var(--amber); }')
    expect(CSS).toContain('.map-marker.tool { background: #68cce8; }')
    expect(CSS).toContain('.map-marker.file { background: #c6a4ef; }')
    expect(CSS).not.toContain('.map-marker.user { border-color:')
  })
})
