import { describe, expect, it } from 'vitest'
import { renderTranscript } from '../src/web/viewer.js'

describe('thread viewer', () => {
  it('renders every message in the minimap and emphasizes user jumps', () => {
    const transcript = renderTranscript([
      { role: 'user', text: 'Plan the change.' },
      { role: 'assistant', text: 'Here is the plan.' },
      { role: 'user', text: 'Ship it.' },
    ])

    expect(transcript.map.match(/class="map-marker/g)).toHaveLength(3)
    expect(transcript.map.match(/class="map-marker user"/g)).toHaveLength(2)
    expect(transcript.map).not.toContain('map-head')
    expect(transcript.map).not.toContain('<span>3</span>')
    expect(transcript.map).toContain('href="#message-3"')
    expect(transcript.messages).toContain('id="message-3"')
  })

  it('escapes message content in both transcript roles', () => {
    const transcript = renderTranscript([{ role: 'assistant', text: '<script>nope</script>' }])

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
    expect(transcript.messages.match(/First chunk\./g)).toHaveLength(1)
    expect(transcript.messages).toContain('First chunk.\n\nSecond chunk.')
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
