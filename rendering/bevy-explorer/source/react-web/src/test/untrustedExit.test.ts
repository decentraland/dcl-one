// The untrusted-link gate's Exit goes to a fixed address: a link carrying ?baseDomain= cannot send the
// player who picks the safe choice to a page it chose.
import { afterEach, describe, expect, it, vi } from 'vitest'

afterEach(() => {
  history.replaceState(null, '', '/')
  vi.resetModules()
})

describe('the untrusted-link exit', () => {
  it('stays on decentraland.org whatever ?baseDomain= the link carries', async () => {
    history.replaceState(null, '', '/?baseDomain=evil.example')
    vi.resetModules()
    const host = await import('../host')
    expect(host.BASE_DOMAIN).toBe('evil.example')
    expect(host.exitUrl()).toBe('https://decentraland.org')
  })
})
