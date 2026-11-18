import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { isPublishSyncing, usePublishInFlight, usePublishSyncStore } from './publishSync'

beforeEach(() => usePublishSyncStore.setState({ syncing: new Set() }))

describe('publish sync store', () => {
  it('tells a mounted page when its collection starts and stops syncing, leaving other collections alone', () => {
    const own = renderHook(() => usePublishInFlight('col-1'))
    const other = renderHook(() => usePublishInFlight('col-2'))
    expect(own.result.current).toBe(false)

    act(() => usePublishSyncStore.getState().start('col-1'))
    expect(own.result.current).toBe(true)
    expect(other.result.current).toBe(false)
    expect(isPublishSyncing('col-1')).toBe(true)

    act(() => usePublishSyncStore.getState().end('col-1'))
    expect(own.result.current).toBe(false)
    expect(isPublishSyncing('col-1')).toBe(false)
  })

  it('reports nothing in flight without a collection', () => {
    act(() => usePublishSyncStore.getState().start('col-1'))
    expect(renderHook(() => usePublishInFlight(undefined)).result.current).toBe(false)
  })
})
