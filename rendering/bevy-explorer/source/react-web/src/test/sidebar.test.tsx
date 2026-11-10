import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '@hud/features/sidebar/Sidebar'
import { renderSettled, fakeSession } from './harness'
import { PopupHost } from '@hud/design/popups'
import { requestDebugPanel } from '@hud/features/debug/useDebugPanel'
import * as boot from '../lib/bootMode'

// Unity parity for the nav rail (unity-explorer DCL/UI/Sidebar/SidebarView.cs).
describe('sidebar parity', () => {
  it.each(['Gallery', 'Places'])('%s uses the Unity art (mask), not a Material glyph', async (name) => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    const button = screen.getByRole('button', { name })
    expect(button.querySelector('svg')).toBeNull()
    expect(button.querySelector('span[aria-hidden="true"]')).not.toBeNull()
  })

  it('Shop opens the shop with the web client utm source, between Backpack and Camera', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    await renderSettled(<Sidebar session={fakeSession()} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Backpack'), labels.indexOf('Camera') + 1)).toEqual(['Backpack', 'Shop', 'Camera'])
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(open).toHaveBeenCalledWith('https://decentraland.org/shop?utm_source=bevy-web-client', '_blank', 'noopener')
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

it.each([true, false])('routes the Session support action through the host that can open a browser (native=%s)', async (native) => {
  vi.spyOn(boot, 'isNativeHud').mockReturnValue(native)
  const open = vi.spyOn(window, 'open').mockReturnValue(null)
  const session = fakeSession()
  session.runDebugCommand = vi.fn(async (line) => line === '/debug_panel' ? '[]' : 'false')
  await renderSettled(<><Sidebar session={session} /><PopupHost /></>)
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
