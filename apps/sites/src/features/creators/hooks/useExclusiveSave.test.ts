import { describe, expect, it } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { SaveInProgressError, useExclusiveSave } from './useExclusiveSave'

function deferred() {
  let resolve!: () => void
  let reject!: (error: Error) => void
  const promise = new Promise<void>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('useExclusiveSave', () => {
  it('stays saving until the save settles and refuses another one meanwhile', async () => {
    const { result } = renderHook(() => useExclusiveSave())
    const first = deferred()
    let running!: Promise<void>
    act(() => {
      running = result.current.run(() => first.promise)
    })
    expect(result.current.isSaving).toBe(true)
    await expect(result.current.run(async () => undefined)).rejects.toBeInstanceOf(SaveInProgressError)

    await act(async () => {
      first.resolve()
      await running
    })
    expect(result.current.isSaving).toBe(false)
    await expect(result.current.run(async () => 'next')).resolves.toBe('next')
  })

  it('frees the next save when one fails', async () => {
    const { result } = renderHook(() => useExclusiveSave())
    await act(async () => {
      await expect(result.current.run(() => Promise.reject(new Error('offline')))).rejects.toThrow('offline')
    })
    expect(result.current.isSaving).toBe(false)
    await expect(result.current.run(async () => 'next')).resolves.toBe('next')
  })
})
