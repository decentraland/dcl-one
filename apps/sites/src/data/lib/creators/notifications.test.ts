import { beforeEach, describe, expect, it } from 'vitest'
import { TOAST_DURATION_MS, useNotifications } from './notifications'

beforeEach(() => {
  useNotifications.setState({ toasts: [] })
})

describe('notifications store', () => {
  it('enqueues success toasts with the default 5s duration', () => {
    useNotifications.getState().showToast('Saved!')
    const [toast] = useNotifications.getState().toasts
    expect(toast.message).toBe('Saved!')
    expect(toast.type).toBe('success')
    expect(toast.durationMs).toBe(TOAST_DURATION_MS)
  })

  it('keeps the requested type and duration', () => {
    useNotifications.getState().showToast('Nope', { type: 'error', durationMs: 1000 })
    const [toast] = useNotifications.getState().toasts
    expect(toast.type).toBe('error')
    expect(toast.durationMs).toBe(1000)
  })

  it('dismisses a toast by id and keeps the rest', () => {
    const store = useNotifications.getState()
    const first = store.showToast('one')
    store.showToast('two')
    useNotifications.getState().dismissToast(first)
    expect(useNotifications.getState().toasts.map(toast => toast.message)).toEqual(['two'])
  })
})
