import type { Env } from '../types.js'

export async function sendMagicLink(env: Env, email: string, link: string): Promise<void> {
  const escaped = escapeHtml(link)
  await env.EMAIL.send({
    to: email,
    from: { email: 'sign-in@footon.dev', name: 'footon' },
    subject: 'Sign in to footon',
    text: `Open this one-time link within 10 minutes:\n\n${link}\n\nIf you did not request it, ignore this email.`,
    html: `<p>Open this one-time link within 10 minutes:</p><p><a href="${escaped}">Sign in to footon</a></p><p>If you did not request it, ignore this email.</p>`,
  })
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;')
}
