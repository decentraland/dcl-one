// The untrusted-link gate's Exit goes to a fixed address: a link carrying ?baseDomain= cannot send the
// player who picks the safe choice to a page it chose.
import { afterEach, describe, expect, it, vi } from 'vitest'

afterEach(() => {
  history.replaceState(null, '', '/')
  vi.resetModules()
})

describe('the untrusted-link exit', () => {
  it('stays on the Sites page it is served from whatever ?baseDomain= the link carries', async () => {
    history.replaceState(null, '', '/play/app.html?baseDomain=evil.example')
    vi.resetModules()
    const host = await import('../host')
    expect(host.BASE_DOMAIN).toBe('evil.example')
    expect(host.exitUrl()).toBe(`${window.location.origin}/`)
    expect(host.exitUrl()).not.toContain('evil.example')
  })
})
