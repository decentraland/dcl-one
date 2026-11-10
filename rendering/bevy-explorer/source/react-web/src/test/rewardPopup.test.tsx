import { describe, it, expect, afterEach } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import type { AppNotification } from '@hud/engine/protocol'
import { PopupHost, resetPopups } from '@hud/design'
import { clearRewardPopups } from '@hud/features/rewards/RewardPopup'
import { clearNotificationToasts } from '@hud/features/notifications/NotificationToast'
import { renderSession, enterAsGuest } from './harness'

// The popup itself is tested with the shared HUD (library/ui/apps/hud/test/rewardPopup.test.tsx); this
// host's session is what opens it.
const reward = (id: string, metadata: Record<string, unknown> = {}): AppNotification => ({
  id,
  type: 'reward_in_progress',
  timestamp: String(Date.now()),
  read: false,
  metadata: { title: 'New item', tokenName: `Hat ${id}`, tokenImage: `https://example.com/${id}.png`, tokenRarity: 'Epic', tokenCategory: 'hat', ...metadata }
})

const collect = (): void => act(() => screen.getByRole('button', { name: 'COLLECT' }).click())

afterEach(() => {
  act(() => {
    clearRewardPopups()
    clearNotificationToasts()
    resetPopups()
  })
})

describe('reward popups in the session', () => {
  it('a polled reward opens the popup; the first load does not', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    act(() => h.driver.emit({ kind: 'notifications', notifications: [reward('old')] }))
    expect(screen.queryByRole('dialog')).toBeNull()
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('new')] }))
    expect(screen.getByText('Hat new')).toBeTruthy()
  })

  it('rewards from two separate polls open one after another', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    act(() => h.driver.emit({ kind: 'notifications', notifications: [] }))
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('a')] }))
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('b')] }))
    expect(screen.getByText('Hat a')).toBeTruthy()
    collect()
    expect(screen.getByText('Hat b')).toBeTruthy()
  })

  it('logout closes the reward on screen and drops the ones waiting', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('a'), reward('b')] }))
    expect(screen.getByRole('dialog')).toBeTruthy()
    await act(async () => h.session().logout())
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })

  it('opens nothing with the HUD hidden', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    const search = location.search
    history.replaceState(null, '', '?hud=0')
    try {
      act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('a')] }))
      expect(screen.queryByRole('dialog')).toBeNull()
    } finally {
      history.replaceState(null, '', search || location.pathname)
    }
  })

  it('a reward already in the list opens nothing when polled again', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    act(() => h.driver.emit({ kind: 'notifications', notifications: [reward('a')] }))
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [reward('a')] }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})
