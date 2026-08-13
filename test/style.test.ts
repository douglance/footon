import { describe, expect, it } from 'vitest'
import { CSS } from '../src/web/style.js'

describe('minimap styling', () => {
  it('uses narrow rails with full role-color fills', () => {
    expect(CSS).toContain('width: 48px;')
    expect(CSS).toContain('width: 10px;')
    expect(CSS).toContain('.minimap li { position: absolute; inset-inline: 1px;')
    expect(CSS).toContain('.map-viewport { position: absolute; z-index: 2; inset-inline: 0;')
    expect(CSS).toContain('.map-marker.user { background: rgba(114, 227, 159, .55); }')
    expect(CSS).toContain('.map-marker.assistant { background: rgba(230, 181, 102, .55); }')
    expect(CSS).toContain('.map-marker.tool { background: rgba(104, 204, 232, .55); }')
    expect(CSS).toContain('.map-marker.file { background: rgba(198, 164, 239, .55); }')
    expect(CSS).toContain('.minimap canvas { position: absolute; z-index: 3; inset: 0;')
    expect(CSS).toContain(
      '.minimap.enhanced ol, .minimap.enhanced .map-viewport { visibility: hidden;',
    )
    expect(CSS).not.toContain('.map-marker.user { border-color:')
  })
})

describe('document controls', () => {
  it('pins one compact global rendered-text switch', () => {
    expect(CSS).toContain('position: sticky;')
    expect(CSS).toContain('grid-template-columns: minmax(0, 1fr) auto;')
    expect(CSS).toContain('.thread-view-toggle:checked ~ .thread .rendered { display: none; }')
    expect(CSS).toContain('.thread-view-toggle:checked ~ .thread .message-text { display: block; }')
    expect(CSS).toContain(
      '.message-body .message-text { overflow-wrap: anywhere; white-space: pre-wrap; }',
    )
    expect(CSS).not.toContain('.render-toggle')
  })
})
