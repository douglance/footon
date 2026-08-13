import { describe, expect, it } from 'vitest'
import { MINIMAP_JS, renderMinimapScript } from '../src/web/minimap.js'
import { htmlResponse, page } from '../src/web/security.js'

describe('visible message range', () => {
  it('builds one proportional offscreen texture from rendered messages', () => {
    expect(MINIMAP_JS).toContain('map.clientHeight / documentHeight')
    expect(MINIMAP_JS).toContain('message.getBoundingClientRect().top + window.scrollY')
    expect(MINIMAP_JS).toContain('message.offsetHeight * scale')
    expect(MINIMAP_JS).toContain("document.createElement('canvas')")
    expect(MINIMAP_JS).toContain('textureContext.fillRect(1, top * scale')
    expect(MINIMAP_JS).toContain('marker.dataset.messageId')
  })

  it('magnifies the visible range with frame-synchronized exponential easing', () => {
    expect(MINIMAP_JS).toContain('sourceHeight * 4.4')
    expect(MINIMAP_JS).toContain('1 - Math.exp(-elapsed / 72)')
    expect(MINIMAP_JS).toContain('drawSlice(area.sourceTop, area.sourceHeight')
    expect(MINIMAP_JS).toContain('requestAnimationFrame')
    expect(MINIMAP_JS).not.toContain('IntersectionObserver')
    expect(MINIMAP_JS).not.toContain('marker.parentElement.style')
  })

  it('supports pointer and keyboard navigation with reduced motion', () => {
    expect(MINIMAP_JS).toContain("canvas.addEventListener('pointerdown'")
    expect(MINIMAP_JS).toContain("canvas.addEventListener('keydown'")
    expect(MINIMAP_JS).toContain("matchMedia('(prefers-reduced-motion: reduce)')")
  })

  it('serves the observer as a self-hosted script', async () => {
    const response = renderMinimapScript()

    expect(response.headers.get('content-type')).toBe('text/javascript; charset=utf-8')
    expect(await response.text()).toBe(MINIMAP_JS)
  })
})

describe('page shell', () => {
  it('allows self-hosted scripts without rendering the old footer', async () => {
    const response = htmlResponse(page('Thread', '<article>history</article>'))
    const contentPolicy = response.headers.get('content-security-policy')
    const body = await response.text()

    expect(contentPolicy).toContain("script-src 'self'")
    expect(body).not.toContain('Raw thread stays local')
    expect(body).not.toContain('Not uploaded')
    expect(body).not.toContain('<footer>')
  })
})
