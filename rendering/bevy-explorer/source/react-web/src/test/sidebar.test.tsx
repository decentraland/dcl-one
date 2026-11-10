// react-web's own Sidebar behaviour: its Shop is the web Shop, and native opens support through the
// engine. The shared Sidebar tests are library/ui/apps/hud/test/hosted/sidebar.test.tsx.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '@hud/features/sidebar/Sidebar'
import { renderSettled, fakeSession } from '@hud/test/harness'
import { PopupHost } from '@hud/design/popups'
import { requestDebugPanel } from '@hud/features/debug/useDebugPanel'
import * as boot from '../lib/bootMode'

// Unity parity for the nav rail (unity-explorer DCL/UI/Sidebar/SidebarView.cs).
afterEach(() => vi.restoreAllMocks())

describe('Sidebar on react-web', () => {
  it('Shop opens the shop with the web client utm source, between Backpack and Camera', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    await renderSettled(<Sidebar session={fakeSession()} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Backpack'), labels.indexOf('Camera') + 1)).toEqual(['Backpack', 'Shop', 'Camera'])
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(open).toHaveBeenCalledWith('https://decentraland.org/shop?utm_source=bevy-web-client', '_blank', 'noopener')
  })

})

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
