export const MINIMAP_JS = `
const rail = document.querySelector('.minimap')
const map = document.querySelector('.minimap ol')
const markers = [...document.querySelectorAll('.map-marker')]
const targets = markers.map((marker) => ({
  marker,
  message: document.getElementById(marker.getAttribute('href')?.slice(1) || marker.dataset.messageId),
}))
const canvas = document.createElement('canvas')
const context = canvas.getContext('2d', { alpha: true })
const texture = document.createElement('canvas')
const textureContext = texture.getContext('2d', { alpha: true })
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches
let scale = 1
let frame = 0

function render() {
  const height = map.clientHeight
  const viewportHeight = Math.min(height, window.innerHeight * scale)
  const viewportTop = Math.min(height - viewportHeight, Math.max(0, window.scrollY * scale))
  context.clearRect(0, 0, map.clientWidth, height)
  context.drawImage(texture, 0, 0, map.clientWidth, height)
  context.fillStyle = 'rgba(255, 255, 255, .2)'
  context.fillRect(0, viewportTop, map.clientWidth, viewportHeight)
}

function schedule() {
  if (frame) return
  frame = requestAnimationFrame(() => {
    frame = 0
    render()
  })
}

function layout() {
  if (!rail || !map || !context || !textureContext) return
  const documentHeight = document.documentElement.scrollHeight
  scale = map.clientHeight / documentHeight
  const ratio = window.devicePixelRatio || 1
  const width = map.clientWidth
  const height = map.clientHeight
  canvas.width = Math.round(width * ratio)
  canvas.height = Math.round(height * ratio)
  texture.width = canvas.width
  texture.height = canvas.height
  canvas.style.width = width + 'px'
  canvas.style.height = height + 'px'
  context.setTransform(ratio, 0, 0, ratio, 0, 0)
  textureContext.setTransform(ratio, 0, 0, ratio, 0, 0)
  textureContext.clearRect(0, 0, width, height)
  for (const { marker, message } of targets) {
    if (!message) continue
    const top = message.getBoundingClientRect().top + window.scrollY
    textureContext.fillStyle = getComputedStyle(marker).backgroundColor
    textureContext.fillRect(1, top * scale, Math.max(1, width - 2), Math.max(1, message.offsetHeight * scale))
  }
  render()
  rail.classList.add('enhanced')
}

canvas.tabIndex = 0
canvas.setAttribute('role', 'navigation')
canvas.setAttribute('aria-label', 'Thread minimap. Use arrow keys to move through the thread.')
canvas.addEventListener('pointerdown', (event) => {
  const target = event.clientY / scale - window.innerHeight / 2
  window.scrollTo({ top: target, behavior: reducedMotion ? 'auto' : 'smooth' })
})
canvas.addEventListener('keydown', (event) => {
  const direction = event.key === 'ArrowUp' || event.key === 'PageUp' ? -1 :
    event.key === 'ArrowDown' || event.key === 'PageDown' ? 1 : 0
  if (!direction) return
  event.preventDefault()
  window.scrollBy({ top: direction * window.innerHeight * .75, behavior: reducedMotion ? 'auto' : 'smooth' })
})
rail?.prepend(canvas)
window.addEventListener('scroll', schedule, { passive: true })
window.addEventListener('resize', layout)
layout()
`

export function renderMinimapScript(): Response {
  return new Response(MINIMAP_JS, {
    headers: {
      'content-type': 'text/javascript; charset=utf-8',
      'cache-control': 'public, max-age=3600',
      'x-content-type-options': 'nosniff',
    },
  })
}
