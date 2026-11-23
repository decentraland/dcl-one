// The shell page's answers to the app's requests, through mount.ts's own message listener: what a request
// came to, or how it failed, with the shell's code when it threw one, so the app can tell a busy shell by
// its code. The purchases module is faked; jsdom has no service worker, so the shell mounts the app at once.

import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

const shop = vi.hoisted(() => ({ request: vi.fn() }))
vi.mock('../shell/shop/purchase', () => ({ createShellShop: () => ({ request: shop.request }) }))
vi.mock('../shell/shop/overlay', () => ({ createShopOverlay: () => ({}) }))
// jsdom has no navigation
const page = vi.hoisted(() => ({ reload: vi.fn() }))
vi.mock('@explorer-core/lib/reloadPage', () => ({ reloadPage: page.reload }))

import { SHOP_BUSY, shopBusy } from '@dcl/one-ui/apps/shop/shopProtocol'
import { mountPlayShell } from '../shell/mount'

let app: Window
let nextId = 1
// the address the visitor entered: a link to a world
let entryUrl = ''

/** The app says a session runs or not: only the question before the page goes reads it. */
function sessionLive(live: boolean): void {
  window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:session', live }, origin: location.origin, source: app }))
}

/** Posts a request from the app's frame, as the app does, and returns the response the shell posts back to it. */
async function ask(method: string, params: unknown = {}): Promise<unknown> {
  const posted = vi.spyOn(app, 'postMessage').mockImplementation(() => {})
  const id = nextId++
  window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:request', id, method, params }, origin: location.origin, source: app }))
  await vi.waitFor(() => expect(posted).toHaveBeenCalled())
  const [response, origin] = posted.mock.calls[0]
  expect(origin).toBe(location.origin)
  expect(response).toMatchObject({ type: 'bevy-shell:response', id })
  return response
}

beforeAll(async () => {
  vi.stubGlobal('crossOriginIsolated', false)
  history.replaceState(null, '', '?realm=entry.dcl.eth')
  entryUrl = location.href
  mountPlayShell()
  app = await vi.waitFor(() => {
    const frame = document.querySelector('iframe')
    if (!frame?.contentWindow) throw new Error('the app is not mounted yet')
    return frame.contentWindow
  })
})

afterEach(() => {
  shop.request.mockReset()
  page.reload.mockReset()
  sessionLive(false)
  // one shell for the file: an exit of one test must not hold the question of the next
  window.dispatchEvent(new Event('pageshow'))
  localStorage.clear()
  vi.restoreAllMocks()
})

describe('the shell’s answers to the app', () => {
  it('posts back what a shop request came to', async () => {
    shop.request.mockResolvedValue({ v: 1, credits: 700 })
    expect(await ask('shop.balance')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), result: { v: 1, credits: 700 } })
  })

  it('hands a shop request to the shop as the app sent it: its method and its params', async () => {
    const params = { v: 1, items: [{ contract: '0x' + '11'.repeat(20), itemId: '1', quantity: 2 }], method: 'credits', cart: true }
    shop.request.mockResolvedValue({ v: 1, operation: 'op', state: 'reviewing' })
    await ask('shop.buy', params)
    expect(shop.request).toHaveBeenCalledTimes(1)
    expect(shop.request).toHaveBeenCalledWith('shop.buy', params)
  })

  it('posts back a busy shop’s code with its message', async () => {
    shop.request.mockRejectedValue(shopBusy())
    expect(await ask('shop.buy')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), error: 'a purchase is already open', code: SHOP_BUSY })
  })

  it('refuses page isolation where it cannot help: here, no service worker', async () => {
    expect(await ask('isolation.page')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), error: 'page isolation is not available' })
    expect(localStorage.getItem('dcl-shell-isolation')).toBeNull()
  })

  it('loads the page for page isolation as the visitor entered it, not as the app wrote the address since', async () => {
    const registration = { update: async () => {}, installing: null, waiting: null, active: {} }
    Object.defineProperty(navigator, 'serviceWorker', { configurable: true, value: { register: async () => registration } })
    try {
      // the engine's URL sync: the lobby has no realm yet
      window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:url', search: '?position=0%2C0' }, origin: location.origin, source: app }))
      expect(location.href).not.toBe(entryUrl)
      expect(await ask('isolation.page')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), result: true })
      await vi.waitFor(() => expect(page.reload).toHaveBeenCalledWith(entryUrl))
    } finally {
      delete (navigator as { serviceWorker?: unknown }).serviceWorker
      sessionStorage.clear()
    }
  })

  it('posts back any other failure as its message alone', async () => {
    shop.request.mockRejectedValue(new Error('this item is not sold here'))
    expect(await ask('shop.buy')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), error: 'this item is not sold here' })
  })
})

describe('the question before the page goes', () => {
  /** True when the browser would ask before it leaves the page. */
  const asks = (): boolean => !window.dispatchEvent(new Event('beforeunload', { cancelable: true }))
  const fromApp = (type: string): void => {
    window.dispatchEvent(new MessageEvent('message', { data: { type }, origin: location.origin, source: app }))
  }

  it('comes only while the app says a session runs: the start screen has none to lose', () => {
    expect(asks()).toBe(false)
    sessionLive(true)
    expect(asks()).toBe(true)
    sessionLive(false)
    expect(asks()).toBe(false)
  })

  it('takes the word of the app’s frame only', () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:session', live: true }, origin: location.origin, source: window }))
    expect(asks()).toBe(false)
  })

  it('does not come on the page’s own exit, and comes again when Back shows the page as it was left', () => {
    sessionLive(true)
    vi.spyOn(window, 'close').mockImplementation(() => {})
    // jsdom has no navigation: the exit's replace() only logs
    vi.spyOn(console, 'error').mockImplementation(() => {})
    fromApp('bevy-shell:exit')
    expect(asks()).toBe(false)
    window.dispatchEvent(new Event('pageshow'))
    expect(asks()).toBe(true)
  })
})

describe('the app’s sign-out in world', () => {
  const ACCOUNT_KEY = 'single-sign-on-0x00000000000000000000000000000000000000aa'
  const signOut = (source: Window): void => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:sign-out' }, origin: location.origin, source }))
  }

  const urlSync = (search: string): void => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:url', search }, origin: location.origin, source: app }))
  }

  // in this order: after a sign-out from the app this shell takes no more of that app's URL sync
  it('takes it only from the app’s frame', async () => {
    localStorage.setItem(ACCOUNT_KEY, '{}')
    signOut(window)
    await Promise.resolve()
    expect(page.reload).not.toHaveBeenCalled()
    expect(localStorage.getItem(ACCOUNT_KEY)).toBe('{}')
  })

  it('keeps the session and the URL sync when the stored sign-in cannot be removed', async () => {
    localStorage.setItem(ACCOUNT_KEY, '{}')
    const failed = vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(() => {
      throw new Error('storage is blocked')
    })
    const posted = vi.spyOn(app, 'postMessage').mockImplementation(() => {})
    signOut(app)
    await vi.waitFor(() => expect(failed).toHaveBeenCalledTimes(1))
    expect(page.reload).not.toHaveBeenCalled()
    // the app tells the player: its menu is closed and the player is still in world
    expect(posted).toHaveBeenCalledWith({ type: 'bevy-shell:sign-out-failed' }, location.origin)
    urlSync('?realm=main&position=9%2C20')
    expect(location.search).toBe('?realm=main&position=9%2C20')
  })

  it('removes the stored account sign-in and the place in the address, then loads the page again once', async () => {
    localStorage.setItem(ACCOUNT_KEY, '{}')
    urlSync('?realm=main&position=10%2C20&preview=1')
    expect(location.search).toBe('?realm=main&position=10%2C20&preview=1')
    page.reload.mockImplementation(() => expect(location.search).toBe('?preview=1'))
    signOut(app)
    // the engine still runs until the page loads again: its place must not come back
    urlSync('?realm=main&position=11%2C20&preview=1')
    await vi.waitFor(() => expect(page.reload).toHaveBeenCalledTimes(1))
    expect(localStorage.getItem(ACCOUNT_KEY)).toBeNull()
    // the page goes at once: the session ended on the player's own word
    sessionLive(true)
    expect(!window.dispatchEvent(new Event('beforeunload', { cancelable: true }))).toBe(false)
    urlSync('?realm=main&position=12%2C20&preview=1')
    expect(location.search).toBe('?preview=1')
  })
})
