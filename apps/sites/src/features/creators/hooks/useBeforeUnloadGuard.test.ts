import { describe, expect, it } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useBeforeUnloadGuard } from './useBeforeUnloadGuard'

function fireBeforeUnload() {
  const event = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(event)
  return event.defaultPrevented
}

describe('useBeforeUnloadGuard', () => {
  it('blocks unload only while active', () => {
    const { rerender, unmount } = renderHook(({ active }) => useBeforeUnloadGuard(active), {
      initialProps: { active: true }
    })
    expect(fireBeforeUnload()).toBe(true)
    rerender({ active: false })
    expect(fireBeforeUnload()).toBe(false)
    rerender({ active: true })
    unmount()
    expect(fireBeforeUnload()).toBe(false)
  })
})
