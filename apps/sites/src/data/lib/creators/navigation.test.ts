import { afterEach, describe, expect, it, vi } from 'vitest'
import { isWebUrl, openExternal, redirectExternal } from './navigation'

describe('openExternal', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('opens the url in a new, isolated tab', () => {
    const open = vi.fn()
    vi.stubGlobal('open', open)
    openExternal('https://example.com')
    expect(open).toHaveBeenCalledWith('https://example.com', '_blank', 'noopener,noreferrer')
  })
})

describe('redirectExternal', () => {
  it('leaves the app in the current tab', () => {
    const assign = vi.fn()
    vi.stubGlobal('location', { ...window.location, assign })
    redirectExternal('https://checkout.example/cs_1')
    expect(assign).toHaveBeenCalledWith('https://checkout.example/cs_1')
    vi.unstubAllGlobals()
  })

  it('refuses anything but a web URL', () => {
    const assign = vi.fn()
    vi.stubGlobal('location', { ...window.location, assign })
    expect(() => redirectExternal('javascript:alert(1)')).toThrow()
    expect(() => redirectExternal('not a url')).toThrow()
    expect(assign).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  })
})

describe('isWebUrl', () => {
  it('accepts only http(s) addresses', () => {
    expect(isWebUrl('https://docs.decentraland.org/creator/wearables/')).toBe(true)
    expect(isWebUrl('http://localhost:3000/x')).toBe(true)
    expect(isWebUrl('javascript:alert(1)')).toBe(false)
    expect(isWebUrl('not a url')).toBe(false)
    expect(isWebUrl(undefined)).toBe(false)
  })
})
