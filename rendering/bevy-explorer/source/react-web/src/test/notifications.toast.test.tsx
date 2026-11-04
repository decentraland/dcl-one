import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import type { AppNotification } from '../engine/protocol'
import {
  NotificationToastHost,
  TOAST_MS,
  clearNotificationToasts,
  queueNotificationToasts
} from '../features/notifications/NotificationToast'
import { ago } from '../features/notifications/summary'
import type { NotificationsState } from '../features/session/useEngineSession'
import { fakeSession, renderSession, enterAsGuest } from './harness'

const notif = (id: string, over: Partial<AppNotification> = {}): AppNotification => ({
  id,
  type: 'item_sold',
  timestamp: String(Date.now()),
  read: false,
  metadata: { title: `Title ${id}` },
  ...over
})

function host(over: Partial<NotificationsState> = {}): NotificationsState {
  const n = { ...fakeSession().notifications, toggle: vi.fn(), ...over }
  render(<NotificationToastHost notifications={n} />)
  return n
}

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  act(() => clearNotificationToasts())
  vi.useRealTimers()
})

describe('new-notification toasts', () => {
  it('shows one toast at a time, in the order queued, each for TOAST_MS', () => {
    host()
    act(() => queueNotificationToasts([notif('a'), notif('b')]))
    expect(screen.getByText('Title a')).toBeTruthy()
    expect(screen.queryByText('Title b')).toBeNull()
    act(() => vi.advanceTimersByTime(TOAST_MS))
    expect(screen.queryByText('Title a')).toBeNull()
    expect(screen.getByText('Title b')).toBeTruthy()
    act(() => vi.advanceTimersByTime(TOAST_MS))
    expect(screen.queryByText('Title b')).toBeNull()
  })

  it('clicking a toast opens the notifications panel and shows the next one', () => {
    const n = host()
    act(() => queueNotificationToasts([notif('a'), notif('b')]))
    act(() => screen.getByRole('button', { name: /Title a/ }).click())
    expect(vi.mocked(n.toggle)).toHaveBeenCalledTimes(1)
    expect(screen.getByText('Title b')).toBeTruthy()
  })

  it('does not toggle the panel closed when it is already open', () => {
    const n = host({ open: true })
    act(() => queueNotificationToasts([notif('a')]))
    act(() => screen.getByRole('button', { name: /Title a/ }).click())
    expect(vi.mocked(n.toggle)).not.toHaveBeenCalled()
  })

  it('clear drops the toast on screen and the queue', () => {
    host()
    act(() => queueNotificationToasts([notif('a'), notif('b')]))
    act(() => clearNotificationToasts())
    act(() => vi.advanceTimersByTime(TOAST_MS * 2))
    expect(screen.queryByRole('button')).toBeNull()
  })
})

describe('notification age', () => {
  it('reads the service timestamp, a string of epoch ms', () => {
    expect(ago(String(Date.now() - 5 * 60_000))).toBe('5m')
    expect(ago(new Date(Date.now() - 2 * 3_600_000).toISOString())).toBe('2h')
    expect(ago('not a date')).toBe('')
  })
})

describe('notificationsNew in the session', () => {
  it('prepends only unknown ids and toasts them oldest first', async () => {
    vi.useRealTimers()
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'notifications', notifications: [notif('a')] })
    vi.useFakeTimers()
    render(<NotificationToastHost notifications={h.session().notifications} />)
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [notif('c'), notif('b'), notif('a')] }))
    expect(h.session().notifications.list.map((n) => n.id)).toEqual(['c', 'b', 'a'])
    expect(h.session().notifications.unread).toBe(3)
    expect(screen.getByText('Title b')).toBeTruthy()
    act(() => vi.advanceTimersByTime(TOAST_MS))
    expect(screen.getByText('Title c')).toBeTruthy()
  })

  it('a repeated poll result adds and toasts nothing', async () => {
    vi.useRealTimers()
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [notif('a')] }))
    act(() => h.driver.emit({ kind: 'notificationsNew', notifications: [notif('a')] }))
    expect(h.session().notifications.list).toHaveLength(1)
  })
})
