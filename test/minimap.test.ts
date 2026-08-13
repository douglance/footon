import { describe, expect, it } from 'vitest'
import { MINIMAP_JS, renderMinimapScript } from '../src/web/minimap.js'
import { htmlResponse, page } from '../src/web/security.js'

describe('visible message range', () => {
  it('observes message elements and toggles matching active markers', () => {
    expect(MINIMAP_JS).toContain('new IntersectionObserver')
    expect(MINIMAP_JS).toContain("marker.classList.toggle('active', entry.isIntersecting)")
    expect(MINIMAP_JS).toContain('observer.observe(message)')
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
