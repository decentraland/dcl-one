import { describe, it, expect, vi, afterEach } from 'vitest'
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '../../features/sidebar/Sidebar'
import { renderSettled, fakeSession } from '../harness'

// The nav rail: its buttons, their order and art, and the voice state.
describe('sidebar parity', () => {
  it.each([
    [true, true],
    [false, false]
  ])('Communities carries the live badge when a member community streams (member %s)', async (isMember, shown) => {
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, active: [{ communityId: 'c1', communityName: 'C', isMember, positions: [], worlds: [], participantCount: 3, moderatorCount: 1 }] }
    await renderSettled(<Sidebar session={s} />)
    const badge = within(screen.getByRole('button', { name: 'Communities' })).queryByRole('img', { name: 'A community stream is live' })
    expect(badge != null).toBe(shown)
  })

  it('Communities has no live badge while nothing streams', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(within(screen.getByRole('button', { name: 'Communities' })).queryByRole('img', { name: 'A community stream is live' })).toBeNull()
  })

  it.each(['Gallery', 'Places'])('%s uses the rail\'s mask art, not a Material glyph', async (name) => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    const button = screen.getByRole('button', { name })
    expect(button.querySelector('svg')).toBeNull()
    expect(button.querySelector('span[aria-hidden="true"]')).not.toBeNull()
  })

  it('Bug report follows Help and opens public support', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    await renderSettled(<Sidebar session={fakeSession()} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels[labels.indexOf('Help & Support') + 1]).toBe('Report a bug')
    await userEvent.click(screen.getByRole('button', { name: 'Report a bug' }))
    const url = new URL(String(open.mock.calls[0][0]))
    expect(url.href).toBe('https://decentraland.org/help/')
  })

  it.each([
    [true, false, 'hearing', true],
    [true, true, 'speaking', true],
    [false, false, 'off', false]
  ])('voice button shows the nearby-voice state (hearing %s, mic %s)', async (hearing, enabled, state, dot) => {
    if (!hearing) localStorage.setItem('nearbyVoiceDisabled', 'true')
    const s = fakeSession()
    s.mic = { ...s.mic, available: true, enabled }
    await renderSettled(<Sidebar session={s} />)
    const button = screen.getByRole('button', { name: 'Nearby Voice' })
    expect(button).toHaveAttribute('data-voice', state)
    expect(button.querySelector('[data-indicator]') != null).toBe(dot)
    localStorage.clear()
  })
})

afterEach(() => vi.restoreAllMocks())

