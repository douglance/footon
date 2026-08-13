export const MINIMAP_JS = `
const rail = document.querySelector('.minimap')
const map = document.querySelector('.minimap ol')
const markers = [...document.querySelectorAll('.map-marker')]
const targets = markers.map((marker) => ({
  marker,
  message: document.getElementById(marker.hash.slice(1)),
}))
const canvas = document.createElement('canvas')
const context = canvas.getContext('2d', { alpha: true })
const texture = document.createElement('canvas')
const textureContext = texture.getContext('2d', { alpha: true })
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches
let scale = 1
let frame = 0
let currentScroll = window.scrollY
let targetScroll = currentScroll
let lastTime = performance.now()

function lens(scroll) {
  const height = map.clientHeight
  const sourceHeight = Math.min(height, window.innerHeight * scale)
  const targetHeight = Math.min(height * .18, sourceHeight * 4.4)
  const sourceTop = Math.min(height - sourceHeight, Math.max(0, scroll * scale))
  const outside = height - sourceHeight
  const targetOutside = height - targetHeight
  const targetTop = outside > 0 ? sourceTop / outside * targetOutside : 0
  return { height, sourceHeight, sourceTop, targetHeight, targetTop }
}

function drawSlice(sourceTop, sourceHeight, targetTop, targetHeight) {
  if (sourceHeight <= 0 || targetHeight <= 0) return
  const ratio = window.devicePixelRatio || 1
  context.drawImage(
    texture,
    0,
    sourceTop * ratio,
    texture.width,
    sourceHeight * ratio,
    0,
    targetTop,
    map.clientWidth,
    targetHeight,
  )
}

function render(scroll) {
  const area = lens(scroll)
  context.clearRect(0, 0, map.clientWidth, area.height)
  drawSlice(0, area.sourceTop, 0, area.targetTop)
  drawSlice(area.sourceTop, area.sourceHeight, area.targetTop, area.targetHeight)
  const sourceAfter = area.sourceTop + area.sourceHeight
  const targetAfter = area.targetTop + area.targetHeight
  drawSlice(sourceAfter, area.height - sourceAfter, targetAfter, area.height - targetAfter)
  context.fillStyle = 'rgba(255, 255, 255, .2)'
  context.fillRect(0, area.targetTop, map.clientWidth, area.targetHeight)
}

function animate(time) {
  const elapsed = Math.max(0, Math.min(48, time - lastTime))
  lastTime = time
  const easing = reducedMotion ? 1 : 1 - Math.exp(-elapsed / 72)
  currentScroll += (targetScroll - currentScroll) * easing
  if (Math.abs(targetScroll - currentScroll) < .1) currentScroll = targetScroll
  render(currentScroll)
  frame = currentScroll !== targetScroll ? requestAnimationFrame(animate) : 0
}

function schedule() {
  targetScroll = window.scrollY
  if (!frame) {
    lastTime = performance.now()
    frame = requestAnimationFrame(animate)
  }
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
  currentScroll = window.scrollY
  targetScroll = currentScroll
  render(currentScroll)
  rail.classList.add('enhanced')
}

function sourceAt(targetY) {
  const area = lens(currentScroll)
  if (targetY < area.targetTop) return targetY / area.targetTop * area.sourceTop
  const targetAfter = area.targetTop + area.targetHeight
  if (targetY <= targetAfter) {
    return area.sourceTop + (targetY - area.targetTop) / area.targetHeight * area.sourceHeight
  }
  const sourceAfter = area.sourceTop + area.sourceHeight
  return sourceAfter + (targetY - targetAfter) / (area.height - targetAfter) * (area.height - sourceAfter)
}

canvas.tabIndex = 0
canvas.setAttribute('role', 'navigation')
canvas.setAttribute('aria-label', 'Thread minimap. Use arrow keys to move through the thread.')
canvas.addEventListener('pointerdown', (event) => {
  const target = sourceAt(event.clientY) / scale - window.innerHeight / 2
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
