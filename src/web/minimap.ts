export const MINIMAP_JS = `
const markers = [...document.querySelectorAll('.map-marker')]
const targets = new Map(markers.map((marker) => [marker.hash.slice(1), marker]))
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    const marker = targets.get(entry.target.id)
    if (marker) marker.classList.toggle('active', entry.isIntersecting)
  }
})
for (const id of targets.keys()) {
  const message = document.getElementById(id)
  if (message) observer.observe(message)
}
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
