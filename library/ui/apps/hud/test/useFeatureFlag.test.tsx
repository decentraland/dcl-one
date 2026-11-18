import { afterEach, describe, expect, it } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { parseFlagOverrides, primeFeatureFlags, setFlagOverrides, type Flag } from '@explorer-core/lib/featureFlags'
import { useFeatureFlag } from '../lib/useFeatureFlag'

// Made-up names, not FLAG's.
const flag = (name: string) => name as Flag
const KNOWN = new Set(['a', 'b'])

afterEach(() => {
  act(() => {
    setFlagOverrides(parseFlagOverrides(''))
    primeFeatureFlags(null)
  })
})

describe('useFeatureFlag', () => {
  it('is undefined until the read settles, then as answered', () => {
    const a = renderHook(() => useFeatureFlag(flag('a')))
    const b = renderHook(() => useFeatureFlag(flag('b')))
    expect(a.result.current).toBeUndefined()
    expect(b.result.current).toBeUndefined()
    act(() => primeFeatureFlags({ flags: { a: true }, variants: {} }))
    expect(a.result.current).toBe(true)
    expect(b.result.current).toBe(false)
  })

  it('takes a ?ff= override at once, and over the answer', () => {
    act(() => setFlagOverrides(parseFlagOverrides('?ff=a,-b', KNOWN)))
    const a = renderHook(() => useFeatureFlag(flag('a')))
    const b = renderHook(() => useFeatureFlag(flag('b')))
    expect(a.result.current).toBe(true)
    expect(b.result.current).toBe(false)
    act(() => primeFeatureFlags({ flags: { a: false, b: true }, variants: {} }))
    expect(a.result.current).toBe(true)
    expect(b.result.current).toBe(false)
  })
})
