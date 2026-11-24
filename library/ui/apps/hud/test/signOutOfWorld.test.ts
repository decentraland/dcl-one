import { beforeEach, describe, expect, it, vi } from 'vitest'

const { shell, postToShell, reloadPage } = vi.hoisted(() => ({ shell: { inside: false }, postToShell: vi.fn(), reloadPage: vi.fn() }))
vi.mock('@explorer-core/lib/shell', async (original) => ({
  ...(await original<object>()),
  get inShell() {
    return shell.inside
  },
  postToShell
}))
vi.mock('@explorer-core/lib/reloadPage', () => ({ reloadPage }))

const showToast = vi.hoisted(() => vi.fn())
vi.mock('../design', async (original) => ({ ...(await original<object>()), showToast }))

const { signOutOfWorld } = await import('../features/gate/signOutOfWorld')

const ACCOUNT_KEY = 'single-sign-on-0x00000000000000000000000000000000000000aa'
const GUEST_KEY = 'dcl-guest-identity'

beforeEach(() => {
  shell.inside = false
  postToShell.mockReset()
  reloadPage.mockReset()
  showToast.mockReset()
  localStorage.clear()
  history.replaceState(null, '', '/play/')
})

// DOMAIN: signing out in world — the page loads again without the stored sign-in.
describe('signing out in world', () => {
  it('inside the shell it asks the shell, and touches neither the storage nor the page', () => {
    shell.inside = true
    localStorage.setItem(ACCOUNT_KEY, '{}')
    signOutOfWorld()
    expect(postToShell).toHaveBeenCalledTimes(1)
    expect(postToShell).toHaveBeenCalledWith({ type: 'bevy-shell:sign-out' })
    expect(reloadPage).not.toHaveBeenCalled()
    expect(localStorage.getItem(ACCOUNT_KEY)).toBe('{}')
  })

  it('inside the shell it tells the player when the shell could not sign out, once for each failure', () => {
    shell.inside = true
    const failed = (source: Window): void => {
      window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:sign-out-failed' }, origin: location.origin, source }))
    }
    signOutOfWorld()
    signOutOfWorld()
    expect(showToast).not.toHaveBeenCalled()
    failed(window.parent)
    expect(showToast).toHaveBeenCalledTimes(1)
    expect(showToast).toHaveBeenCalledWith('Could not sign out. Try again in a moment.', { tone: 'error' })
  })

  it('with no shell it removes the stored account sign-in, keeps a stored guest, and loads the page again', () => {
    localStorage.setItem(ACCOUNT_KEY, '{}')
    localStorage.setItem(GUEST_KEY, '{}')
    signOutOfWorld()
    expect(localStorage.getItem(ACCOUNT_KEY)).toBeNull()
    expect(localStorage.getItem(GUEST_KEY)).toBe('{}')
    expect(reloadPage).toHaveBeenCalledTimes(1)
    expect(postToShell).not.toHaveBeenCalled()
  })

  it('with no shell the page loads again without the place the engine wrote in the address', () => {
    history.replaceState(null, '', '/play/?realm=main&position=10%2C20&preview=1#x')
    reloadPage.mockImplementation(() => expect(location.search).toBe('?preview=1'))
    signOutOfWorld()
    expect(reloadPage).toHaveBeenCalledTimes(1)
    expect(location.pathname + location.search + location.hash).toBe('/play/?preview=1#x')
  })
})
