import { describe, expect, it } from 'vitest'
import { MINIMAP_JS, renderMinimapScript } from '../src/web/minimap.js'
import { htmlResponse, page } from '../src/web/security.js'

describe('visible message range', () => {
  it('scales each marker to its rendered message geometry', () => {
    expect(MINIMAP_JS).toContain('map.clientHeight / documentHeight')
    expect(MINIMAP_JS).toContain('message.getBoundingClientRect().top + window.scrollY')
    expect(MINIMAP_JS).toContain('message.offsetHeight * scale')
  })

  it('moves one viewport rectangle with the visible document range', () => {
    expect(MINIMAP_JS).toContain('window.innerHeight * scale')
    expect(MINIMAP_JS).toContain("'translateY(' + window.scrollY * scale + 'px)'")
    expect(MINIMAP_JS).toContain('requestAnimationFrame')
    expect(MINIMAP_JS).not.toContain('IntersectionObserver')
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
