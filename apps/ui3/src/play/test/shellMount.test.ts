// The shell page's answers to the app's requests, through mount.ts's own message listener: what a request
// came to, or how it failed, with the shell's code when it threw one, so the app can tell a busy shell by
// its code. The purchases module is faked; jsdom has no service worker, so the shell mounts the app at once.

import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

const shop = vi.hoisted(() => ({ request: vi.fn() }))
vi.mock('../shell/shop/purchase', () => ({ createShellShop: () => ({ request: shop.request }) }))
vi.mock('../shell/shop/overlay', () => ({ createShopOverlay: () => ({}) }))

import { SHOP_BUSY, shopBusy } from '../lib/shopProtocol'
import { mountPlayShell } from '../shell/mount'

let app: Window
let nextId = 1

/** Posts a request from the app's frame, as the app does, and returns the response the shell posts back to it. */
async function ask(method: string): Promise<unknown> {
  const posted = vi.spyOn(app, 'postMessage').mockImplementation(() => {})
  const id = nextId++
  window.dispatchEvent(new MessageEvent('message', { data: { type: 'bevy-shell:request', id, method, params: {} }, origin: location.origin, source: app }))
  await vi.waitFor(() => expect(posted).toHaveBeenCalled())
  const [response, origin] = posted.mock.calls[0]
  expect(origin).toBe(location.origin)
  expect(response).toMatchObject({ type: 'bevy-shell:response', id })
  return response
}

beforeAll(async () => {
  vi.stubGlobal('crossOriginIsolated', false)
  mountPlayShell()
  app = await vi.waitFor(() => {
    const frame = document.querySelector('iframe')
    if (!frame?.contentWindow) throw new Error('the app is not mounted yet')
    return frame.contentWindow
  })
})

afterEach(() => {
  shop.request.mockReset()
  vi.restoreAllMocks()
})

describe('the shell’s answers to the app', () => {
  it('posts back what a shop request came to', async () => {
    shop.request.mockResolvedValue({ v: 1, credits: 700 })
    expect(await ask('shop.balance')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), result: { v: 1, credits: 700 } })
  })

  it('posts back a busy shop’s code with its message', async () => {
    shop.request.mockRejectedValue(shopBusy())
    expect(await ask('shop.buy')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), error: 'a purchase is already open', code: SHOP_BUSY })
  })

  it('posts back any other failure as its message alone', async () => {
    shop.request.mockRejectedValue(new Error('this item is not sold here'))
    expect(await ask('shop.buy')).toEqual({ type: 'bevy-shell:response', id: expect.any(Number), error: 'this item is not sold here' })
  })
})
