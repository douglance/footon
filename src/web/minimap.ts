export const MINIMAP_JS = `
const map = document.querySelector('.minimap ol')
const viewport = document.querySelector('.map-viewport')
const markers = [...document.querySelectorAll('.map-marker')]
const targets = markers.map((marker) => ({
  marker,
  message: document.getElementById(marker.hash.slice(1)),
}))
let scale = 1
let frame = 0

function positionViewport() {
  if (!viewport) return
  viewport.style.height = Math.max(1, window.innerHeight * scale) + 'px'
  viewport.style.transform = 'translateY(' + window.scrollY * scale + 'px)'
}

function layout() {
  if (!map) return
  const documentHeight = document.documentElement.scrollHeight
  scale = map.clientHeight / documentHeight
  for (const { marker, message } of targets) {
    if (!message || !marker.parentElement) continue
    const top = message.getBoundingClientRect().top + window.scrollY
    marker.parentElement.style.top = top * scale + 'px'
    marker.parentElement.style.height = Math.max(1, message.offsetHeight * scale) + 'px'
  }
  positionViewport()
}

function scheduleViewport() {
  if (frame) return
  frame = requestAnimationFrame(() => {
    frame = 0
    positionViewport()
  })
}

window.addEventListener('scroll', scheduleViewport, { passive: true })
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
