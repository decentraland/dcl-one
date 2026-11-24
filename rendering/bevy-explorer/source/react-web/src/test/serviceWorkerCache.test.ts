// The service worker and the asset cache: the engine reads and fills `ipfs-path-cache-v2` itself,
// so the worker leaves content requests to the browser.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it, vi } from 'vitest'

// the worker's script, run against a stand-in global
function serviceWorker() {
  const listeners: Record<string, (event: unknown) => void> = {}
  const network = vi.fn(async (req: Request) => new Response(new URL(req.url).origin))
  const self = {
    addEventListener: (type: string, fn: (event: unknown) => void) => (listeners[type] = fn),
    registration: { scope: 'https://play.example/' }
  }
  const source = readFileSync(resolve(__dirname, '../../../deploy/web/service_worker.js'), 'utf8')
  const load = new Function('self', 'caches', 'fetch', 'console', source) as (
    self: object,
    caches: object,
    fetch: object,
    console: object
  ) => void
  const open = vi.fn()
  load(self, { open }, network, { log: vi.fn(), warn: vi.fn() })

  // whether the worker replied to a request
  const replies = (request: Request): boolean => {
    let replied = false
    listeners.fetch({ request, respondWith: () => (replied = true) })
    return replied
  }
  return { replies, open, network }
}

describe('service worker asset cache', () => {
  it('leaves a content request to the browser, without looking in the cache', () => {
    const { replies, open, network } = serviceWorker()
    const url = 'https://peer.example/contents/bafkreiexample'
    expect(replies(new Request(url))).toBe(false)
    // with or without an `X-IPFS` header
    expect(replies(new Request(url, { headers: { 'X-IPFS': '1' } }))).toBe(false)
    expect(open).not.toHaveBeenCalled()
    expect(network).not.toHaveBeenCalled()
  })
})
