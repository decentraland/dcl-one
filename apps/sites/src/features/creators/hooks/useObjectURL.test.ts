import { afterEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useObjectURL } from './useObjectURL'

describe('useObjectURL', () => {
  afterEach(() => vi.restoreAllMocks())

  it('creates a URL for the blob and revokes it only when the blob changes or on unmount', () => {
    let counter = 0
    const create = vi.fn(() => `blob:${++counter}`)
    const revoke = vi.fn()
    URL.createObjectURL = create
    URL.revokeObjectURL = revoke

    const initialProps: { blob: Blob | null } = { blob: new Blob(['a']) }
    const { result, rerender, unmount } = renderHook(({ blob }: { blob: Blob | null }) => useObjectURL(blob), {
      initialProps
    })
    expect(result.current).toBe('blob:1')

    rerender(initialProps)
    expect(revoke).not.toHaveBeenCalled()

    rerender({ blob: new Blob(['b']) })
    expect(revoke).toHaveBeenCalledWith('blob:1')
    expect(result.current).toBe('blob:2')

    rerender({ blob: null })
    expect(result.current).toBeNull()
    expect(revoke).toHaveBeenCalledWith('blob:2')

    unmount()
    expect(revoke).toHaveBeenCalledTimes(2)
  })
})
