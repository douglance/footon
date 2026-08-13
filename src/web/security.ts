export function htmlResponse(body: string, status = 200, extra: HeadersInit = {}): Response {
  const headers = new Headers(extra)
  headers.set('content-type', 'text/html; charset=utf-8')
  if (!headers.has('content-security-policy')) {
    headers.set(
      'content-security-policy',
      "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
    )
  }
  headers.set('referrer-policy', 'no-referrer')
  headers.set('x-content-type-options', 'nosniff')
  headers.set('x-frame-options', 'DENY')
  return new Response(body, { status, headers })
}

export function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

export function page(title: string, content: string): string {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHtml(title)} · footon</title><link rel="stylesheet" href="/style.css?v=5"></head><body><header><a class="brand" href="/">footon</a><span>safe agent threads</span><span class="status">safe</span></header><main>${content}</main><footer><span>Raw thread stays local</span><span>Not uploaded</span></footer></body></html>`
}
