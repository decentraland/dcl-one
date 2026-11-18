import { describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useDiscardGuard, type DiscardGuardState } from './useDiscardGuard'

type Props = { state: DiscardGuardState; dirty: boolean; pending?: boolean }

function renderGuard(initialProps: Props) {
  const onClose = vi.fn()
  const hook = renderHook((props: Props) => useDiscardGuard({ ...props, onClose }), { initialProps })
  return { onClose, ...hook }
}

function fireBeforeUnload() {
  const event = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(event)
  return event.defaultPrevented
}

function pressBack() {
  act(() => {
    window.dispatchEvent(new PopStateEvent('popstate'))
  })
}

describe('useDiscardGuard', () => {
  it('closes a clean draft straight away and asks to discard a dirty one, from a close or browser back', () => {
    const { result, rerender, onClose, unmount } = renderGuard({ state: 'editing', dirty: false })
    act(() => result.current.requestClose())
    expect(onClose).toHaveBeenCalledTimes(1)

    rerender({ state: 'editing', dirty: true })
    pressBack()
    expect(result.current.isDiscardOpen).toBe(true)
    expect(onClose).toHaveBeenCalledTimes(1)
    act(() => result.current.keepEditing())
    expect(result.current.isDiscardOpen).toBe(false)
    act(() => result.current.requestClose())
    expect(result.current.isDiscardOpen).toBe(true)
    unmount()
  })

  it('ignores closes while busy, closes freely once failed or done, and never carries a hidden request back to editing', () => {
    const { result, rerender, onClose, unmount } = renderGuard({ state: 'busy', dirty: true })
    act(() => result.current.requestClose())
    pressBack()
    expect(result.current.isDiscardOpen).toBe(false)
    expect(onClose).not.toHaveBeenCalled()

    rerender({ state: 'editing', dirty: true })
    expect(result.current.isDiscardOpen).toBe(false)

    rerender({ state: 'failed', dirty: true })
    pressBack()
    expect(onClose).toHaveBeenCalledTimes(1)

    rerender({ state: 'done', dirty: true })
    pressBack()
    expect(onClose).toHaveBeenCalledTimes(2)
    act(() => result.current.requestClose())
    expect(onClose).toHaveBeenCalledTimes(3)
    unmount()
  })

  it('warns before unload while a dirty draft is edited, kept for a retry or being submitted, not once done', () => {
    const { rerender, unmount } = renderGuard({ state: 'editing', dirty: false })
    expect(fireBeforeUnload()).toBe(false)
    rerender({ state: 'editing', dirty: true })
    expect(fireBeforeUnload()).toBe(true)
    rerender({ state: 'busy', dirty: true, pending: true })
    expect(fireBeforeUnload()).toBe(true)
    rerender({ state: 'editing', dirty: true, pending: true })
    expect(fireBeforeUnload()).toBe(true)
    rerender({ state: 'failed', dirty: true })
    expect(fireBeforeUnload()).toBe(true)
    rerender({ state: 'failed', dirty: false })
    expect(fireBeforeUnload()).toBe(false)
    rerender({ state: 'done', dirty: true })
    expect(fireBeforeUnload()).toBe(false)
    unmount()
  })
})
