// The bridge between the app and the shell, both ends real: the shell's answer to a request (shellResponse,
// which mount.ts posts back) crossing postMessage's structured clone to the app's shellRequest. A failure
// keeps the shell's code, so the app tells a busy shell by its code, never by its message.

import { afterEach, describe, expect, it, vi } from 'vitest'
import { shellRequest } from '@explorer-core/lib/shell'
import { isShellError, ShellError, shellResponse } from '@explorer-core/lib/shellReply'
import { SHOP_BUSY, shopBusy } from '../lib/shopProtocol'

/** The shell's side: each request the app posts runs `run`, and its response comes back as postMessage carries it. */
function shellAnswering(run: () => Promise<unknown>): void {
  // jsdom's window is its own parent, the shell's place.
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    const m = message as { type?: unknown; id?: unknown }
    if (m.type !== 'bevy-shell:request') return
    void shellResponse(m.id, run).then((response) => {
      window.dispatchEvent(new MessageEvent('message', { data: structuredClone(response), origin: location.origin, source: window }))
    })
  })
}

const failure = (method: string): Promise<unknown> => shellRequest(method).then(
  () => {
    throw new Error(`${method} resolved`)
  },
  (e: unknown) => e
)

afterEach(() => {
  vi.restoreAllMocks()
})

describe('the shell’s answers, across the bridge', () => {
  it('resolves with what the request came to', async () => {
    shellAnswering(async () => ({ v: 1, credits: 700 }))
    await expect(shellRequest('shop.balance')).resolves.toEqual({ v: 1, credits: 700 })
  })

  it('keeps a busy shell’s code, so the app tells it by its code', async () => {
    shellAnswering(() => Promise.reject(shopBusy()))
    const e = await failure('shop.buy')
    expect(e).toBeInstanceOf(ShellError)
    expect(e).toMatchObject({ code: SHOP_BUSY, message: 'a purchase is already open' })
    expect(isShellError(e, SHOP_BUSY)).toBe(true)
  })

  it('carries any other failure as its message alone, which no code matches: not even the busy one’s words', async () => {
    shellAnswering(() => Promise.reject(new Error(shopBusy().message)))
    const e = await failure('shop.buy')
    expect(e).toBeInstanceOf(Error)
    expect(e).not.toBeInstanceOf(ShellError)
    expect((e as Error).message).toBe('a purchase is already open')
    expect(isShellError(e, SHOP_BUSY)).toBe(false)
  })

  it('carries something thrown that isn’t an Error as its text', async () => {
    shellAnswering(() => Promise.reject('no wallet here'))
    expect(((await failure('shop.buy')) as Error).message).toBe('no wallet here')
  })
})
