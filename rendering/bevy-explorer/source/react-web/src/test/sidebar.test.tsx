import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '../features/sidebar/Sidebar'
import { fakeSession } from './harness'
import { PopupHost } from '../design/popups'
import { requestDebugPanel } from '../features/debug/useDebugPanel'
import * as boot from '../lib/bootMode'

// Unity parity for the nav rail (unity-explorer DCL/UI/Sidebar/SidebarView.cs).
describe('sidebar parity', () => {
  it.each(['Gallery', 'Places'])('%s uses the Unity art (mask), not a Material glyph', (name) => {
    render(<Sidebar session={fakeSession()} />)
    const button = screen.getByRole('button', { name })
    expect(button.querySelector('svg')).toBeNull()
    expect(button.querySelector('span[aria-hidden="true"]')).not.toBeNull()
  })

  it.each([
    [true, true],
    [false, false]
  ])('Communities carries the live badge when a member community streams (member %s)', (isMember, shown) => {
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, active: [{ communityId: 'c1', communityName: 'C', isMember, positions: [], worlds: [], participantCount: 3, moderatorCount: 1 }] }
    render(<Sidebar session={s} />)
    const badge = within(screen.getByRole('button', { name: 'Communities' })).queryByRole('img', { name: 'A community stream is live' })
    expect(badge != null).toBe(shown)
  })

  it('Communities has no live badge while nothing streams', () => {
    render(<Sidebar session={fakeSession()} />)
    expect(within(screen.getByRole('button', { name: 'Communities' })).queryByRole('img', { name: 'A community stream is live' })).toBeNull()
  })

  it('Shop opens the shop with the web client utm source, between Backpack and Camera', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    render(<Sidebar session={fakeSession()} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Backpack'), labels.indexOf('Camera') + 1)).toEqual(['Backpack', 'Shop', 'Camera'])
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(open).toHaveBeenCalledWith('https://decentraland.org/shop?utm_source=bevy-web-client', '_blank', 'noopener')
  })

  it('Bug report follows Help and opens public support', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    render(<Sidebar session={fakeSession()} />)
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
  ])('voice button shows the nearby-voice state (hearing %s, mic %s)', (hearing, enabled, state, dot) => {
    if (!hearing) localStorage.setItem('nearbyVoiceDisabled', 'true')
    const s = fakeSession()
    s.mic = { ...s.mic, available: true, enabled }
    render(<Sidebar session={s} />)
    const button = screen.getByRole('button', { name: 'Nearby Voice' })
    expect(button).toHaveAttribute('data-voice', state)
    expect(button.querySelector('[data-indicator]') != null).toBe(dot)
    localStorage.clear()
  })
})

afterEach(() => vi.restoreAllMocks())

it.each([true, false])('routes the Session support action through the host that can open a browser (native=%s)', async (native) => {
  vi.spyOn(boot, 'isNativeHud').mockReturnValue(native)
  const open = vi.spyOn(window, 'open').mockReturnValue(null)
  const session = fakeSession()
  session.runDebugCommand = vi.fn(async (line) => line === '/debug_panel' ? '[]' : 'false')
  render(<><Sidebar session={session} /><PopupHost /></>)
  act(() => requestDebugPanel('Session', vi.fn()))
  const panel = within(await screen.findByRole('dialog', { name: 'Debug panel' }))
  await userEvent.click(await panel.findByRole('button', { name: 'Report a bug' }))
  if (native) {
    expect(session.nav).toHaveBeenCalledWith('support')
    expect(open).not.toHaveBeenCalled()
  } else {
    expect(open).toHaveBeenCalledWith('https://decentraland.org/help/', '_blank', 'noopener')
    expect(session.nav).not.toHaveBeenCalled()
  }
})
