import { describe, expect, it } from 'vitest'
import { renderDocumentHeader, renderProseMarkdown, renderTranscript } from '../src/web/viewer.js'

describe('thread viewer basics', () => {
  it('renders every message in the minimap and emphasizes user jumps', () => {
    const transcript = renderTranscript([
      { role: 'user', text: 'Plan the change.' },
      { role: 'assistant', text: 'Here is the plan.' },
      { role: 'user', text: 'Ship it.' },
    ])

    expect(transcript.map.match(/class="map-marker/g)).toHaveLength(3)
    expect(transcript.map).toContain('class="map-viewport"')
    expect(transcript.map.match(/class="map-marker user"/g)).toHaveLength(2)
    expect(transcript.map.match(/<a class="map-marker user"/g)).toHaveLength(2)
    expect(transcript.map.match(/<span class="map-marker assistant"/g)).toHaveLength(1)
    expect(transcript.map).toContain('data-message-id="message-2"')
    expect(transcript.map).not.toContain('map-head')
    expect(transcript.map).not.toContain('<span>3</span>')
    expect(transcript.map).toContain('href="#message-3"')
    expect(transcript.messages).toContain('id="message-3"')
  })

  it('escapes message content in both transcript roles', () => {
    const transcript = renderTranscript([{ role: 'assistant', text: '<script>nope</script>' }])

    expect(transcript.messages).toContain('<span>001</span>AGENT</div>')
    expect(transcript.messages).toContain('aria-label="agent 1"')
    expect(transcript.messages).toContain('&lt;script&gt;nope&lt;/script&gt;')
    expect(transcript.messages).not.toContain('<script>')
  })

  it('combines consecutive assistant messages and removes repeats in the run', () => {
    const transcript = renderTranscript([
      { role: 'user', text: 'Start.' },
      { role: 'assistant', text: 'First chunk.' },
      { role: 'assistant', text: 'First chunk.' },
      { role: 'assistant', text: 'Second chunk.' },
      { role: 'user', text: 'Continue.' },
    ])

    expect(transcript.map.match(/class="map-marker/g)).toHaveLength(3)
    expect(transcript.messages.match(/First chunk\./g)).toHaveLength(2)
    expect(transcript.messages).toContain('First chunk.\n\nSecond chunk.')
    expect(transcript.messages).not.toContain('First chunk.\n\nFirst chunk.')
  })
})

describe('thread viewer Markdown', () => {
  it('renders CommonMark for user and agent prose with escaped text fallback', () => {
    const transcript = renderTranscript([
      {
        role: 'assistant',
        text: '# Title\n\nVisit [safe](https://example.com) and [bad](javascript:alert(1)).\n\n<script>nope</script>',
      },
    ])

    expect(transcript.messages).not.toContain('class="view-control"')
    expect(transcript.messages).not.toContain('class="thread-view-toggle"')
    expect(transcript.messages).toContain('<h2>Title</h2>')
    expect(transcript.messages).toContain('<a href="https://example.com">safe</a>')
    expect(transcript.messages).not.toContain('href="javascript:alert(1)"')
    expect(transcript.messages).toContain('&lt;script&gt;nope&lt;/script&gt;')
    expect(transcript.messages).toContain(
      '# Title\n\nVisit [safe](https://example.com) and [bad](javascript:alert(1)).',
    )
  })

  it('keeps activity escaped without Markdown rendering', () => {
    const transcript = renderTranscript([
      { role: 'tool', text: '**exec** <b>raw</b>' },
      { role: 'file', text: 'update viewer.ts' },
    ])

    expect(transcript.messages).not.toContain('class="view-control"')
    expect(transcript.messages).toContain('**exec** &lt;b&gt;raw&lt;/b&gt;')
    expect(transcript.messages).not.toContain('<strong>exec</strong>')
  })
})

describe('thread viewer source preservation', () => {
  it('preserves untouched user and agent source text exactly', () => {
    const source = '  leading\n\n\ntrailing  \n'
    const transcript = renderTranscript([
      { role: 'user', text: source },
      { role: 'assistant', text: source },
    ])

    expect(transcript.messages.split(source)).toHaveLength(3)
  })

  it('falls back without throwing when the Markdown renderer fails', () => {
    const failed = renderProseMarkdown('safe source', {
      parse: () => {
        throw new Error('render failure')
      },
    } as never)

    expect(failed).toEqual({ ok: false })
  })
})

describe('thread viewer attribution', () => {
  it('filters injected blocks in stored shares before rendering', () => {
    const transcript = renderTranscript([
      {
        role: 'user',
        text: 'Visible\n\n<environment_context>\n<cwd>/private</cwd>\n</environment_context>\n\nStill visible',
      },
    ])

    expect(transcript.messages).toContain('Visible')
    expect(transcript.messages).toContain('Still visible')
    expect(transcript.messages).not.toContain('/private')
  })

  it('drops fully injected user records before numbering and minimap rendering', () => {
    const transcript = renderTranscript([
      {
        role: 'user',
        text: '# [DOMAIN_NAME] instructions\n\n<INSTRUCTIONS>not authored by the user</INSTRUCTIONS>',
      },
      {
        role: 'user',
        text: '<codex_internal_context source="goal">not authored by the user</codex_internal_context>',
      },
      { role: 'user', text: '# Real prompt\n\nPlease inspect this.' },
      { role: 'assistant', text: 'Done.' },
    ])

    expect(transcript.map.match(/class="map-marker/g)).toHaveLength(2)
    expect(transcript.messages).toContain('aria-label="user 1"')
    expect(transcript.messages).toContain('aria-label="agent 2"')
    expect(transcript.messages).not.toContain('not authored by the user')
  })
})

describe('thread viewer scale', () => {
  it('renders the full 2,000-message contract with only user minimap links', () => {
    const messages = Array.from({ length: 2_000 }, (_, index) => ({
      role: index % 2 === 0 ? ('user' as const) : ('assistant' as const),
      text: `message ${String(index + 1)}`,
    }))

    const transcript = renderTranscript(messages)
    expect(transcript.map.match(/class="map-marker/g)).toHaveLength(2_000)
    expect(transcript.map.match(/<a class="map-marker user"/g)).toHaveLength(1_000)
    expect(transcript.map.match(/<span class="map-marker assistant"/g)).toHaveLength(1_000)
    expect(transcript.messages).not.toContain('class="thread-view-toggle"')
  })
})

describe('thread document header', () => {
  it('renders one global view control with compact metadata', () => {
    const header = renderDocumentHeader('Build history', '2026-08-13T00:00:00.000Z', 61)

    expect(header.match(/class="thread-view-toggle"/g)).toHaveLength(1)
    expect(header).toContain('aria-label="Show source text for all messages"')
    expect(header).toContain('<span>Rendered</span><span>Text</span>')
    expect(header).toContain('Shared August 13, 2026. 61 redactions.')
    expect(header).not.toContain('Sanitized locally')
  })
})

describe('execution history', () => {
  it('groups tool activity beneath model responses without call labels', () => {
    const transcript = renderTranscript([
      { role: 'assistant', text: 'I will inspect it.' },
      { role: 'tool', text: 'functions.exec custom-tool 2 arguments' },
      { role: 'file', text: 'update viewer.ts' },
      { role: 'assistant', text: 'The change is complete.' },
    ])

    expect(transcript.messages).not.toContain('llm_call')
    expect(transcript.messages).not.toContain('events')
    expect(transcript.messages).not.toContain('aria-hidden')
    expect(transcript.messages).not.toContain('response</div>')
    expect(transcript.messages).toContain('class="activity-run"')
    expect(transcript.messages).toContain('class="message tool"')
    expect(transcript.messages.indexOf('custom-tool 2 arguments')).toBeLessThan(
      transcript.messages.indexOf('The change is complete.'),
    )
  })
})
