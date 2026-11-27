import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import { renderSession, enterAsGuest } from '../harness'
import { ToastHost, dismissToast } from '../../design'
import { SCENE_BAN_NOTICE, SCENE_BAN_NOTICE_MS, SCENE_BAN_TOAST } from '../../features/minimap/sceneBan'
import type { SceneBanScope } from '@explorer-core/engine/generated'

type Harness = ReturnType<typeof renderSession>

const NOTICE = SCENE_BAN_NOTICE.scene

function ban(h: Harness, scope: SceneBanScope | null): void {
  act(() => h.driver.emit({ kind: 'sceneBan', ban: { scope } }))
}
function loading(h: Harness, visible: boolean): void {
  act(() => h.driver.emit({ kind: 'sceneLoading', state: { visible, realmConnected: true, title: '', pendingAssets: null } }))
}
/** In the world with the notices on screen. Timers are faked only from here, so the entry's waits run. */
async function inWorld(): Promise<Harness> {
  const h = renderSession()
  render(<ToastHost />)
  await enterAsGuest(h)
  vi.useFakeTimers()
  return h
}

afterEach(() => {
  vi.useRealTimers()
  act(() => dismissToast(SCENE_BAN_TOAST))
})

// DOMAIN: the scene ban. The engine says which ban refuses the scene the player stands in; the
// minimap marks it, and the player is told why once per ban.
describe('scene ban domain', () => {
  it('keeps the scope for the minimap and drops it when admitted', async () => {
    const h = await inWorld()
    expect(h.session().minimap.sceneBan).toBeNull()
    ban(h, 'world')
    expect(h.session().minimap.sceneBan).toBe('world')
    ban(h, null)
    expect(h.session().minimap.sceneBan).toBeNull()
  })

  it('says why for ten seconds', async () => {
    const h = await inWorld()
    ban(h, 'scene')
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
    act(() => { vi.advanceTimersByTime(SCENE_BAN_NOTICE_MS - 1) })
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
    act(() => { vi.advanceTimersByTime(1) })
    expect(screen.queryByText(NOTICE)).toBeNull()
  })

  it('waits for loading to end before saying why', async () => {
    const h = await inWorld()
    loading(h, true)
    ban(h, 'scene')
    expect(screen.queryByText(NOTICE)).toBeNull()
    loading(h, false)
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
  })

  it('does not say it again while the ban stands, and does for the next ban', async () => {
    const h = await inWorld()
    ban(h, 'scene')
    act(() => { vi.advanceTimersByTime(SCENE_BAN_NOTICE_MS) })
    loading(h, true)
    loading(h, false)
    expect(screen.queryByText(NOTICE)).toBeNull()
    ban(h, null)
    ban(h, 'scene')
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
  })

  it('an admission takes the notice down', async () => {
    const h = await inWorld()
    ban(h, 'scene')
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
    ban(h, null)
    expect(screen.queryByText(NOTICE)).toBeNull()
  })

  it('sign-out clears the ban and its notice', async () => {
    const h = await inWorld()
    vi.useRealTimers()
    ban(h, 'scene')
    expect(screen.getByText(NOTICE)).toBeInTheDocument()
    act(() => h.session().logout())
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().minimap.sceneBan).toBeNull()
    expect(screen.queryByText(NOTICE)).toBeNull()
  })
})
