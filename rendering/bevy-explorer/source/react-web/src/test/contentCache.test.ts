// The page side of the content cache's upkeep: relays the engine's trim request to the controlling
// service worker and asks for persistent storage.
import { afterEach, describe, expect, it, vi } from 'vitest'
// plain JS served as is by the engine page (deploy/web/engine), so it has no declarations
// @ts-expect-error TS7016
import { contentCacheTouch, contentCacheTrim, installContentCachePageFunctions, requestPersistentStorage } from '../../../deploy/web/engine/content_cache.js'

afterEach(() => vi.useRealTimers())

describe('content cache page functions', () => {
  it("resolves with the service worker's reply", async () => {
    const container = {
      controller: {
        postMessage(msg: unknown, [port]: MessagePort[]) {
          port.postMessage({ evictedEntries: 2, msg })
        }
      }
    }
    const reply = await contentCacheTrim(100, false, container as never, 1000)
    expect(reply).toMatchObject({
      evictedEntries: 2,
      msg: { type: 'content-cache-trim', capBytes: 100, quotaError: false }
    })
  })

  it('resolves null without a controlling worker', async () => {
    expect(await contentCacheTrim(100, false, { controller: null } as never, 1000)).toBeNull()
    expect(await contentCacheTrim(100, false, undefined, 1000)).toBeNull()
  })

  it('resolves null when the worker never answers', async () => {
    vi.useFakeTimers()
    const container = { controller: { postMessage: vi.fn() } }
    const pending = contentCacheTrim(100, false, container as never)
    await vi.advanceTimersByTimeAsync(30_000)
    expect(await pending).toBeNull()
  })

  it('gives up on a quota-error request after 5 s, while a regular one waits 30 s', async () => {
    vi.useFakeTimers()
    const container = { controller: { postMessage: vi.fn() } }
    let quotaDone = false
    let regularDone = false
    const quota = contentCacheTrim(100, true, container as never).then(() => (quotaDone = true))
    const regular = contentCacheTrim(100, false, container as never).then(() => (regularDone = true))
    await vi.advanceTimersByTimeAsync(4_999)
    expect(quotaDone).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    await quota
    expect(quotaDone).toBe(true)
    expect(regularDone).toBe(false)
    await vi.advanceTimersByTimeAsync(25_000)
    await regular
    expect(regularDone).toBe(true)
  })

  it('asks for persistence only when not persisted', async () => {
    const persisted = { persisted: vi.fn(async () => true), persist: vi.fn(async () => false) }
    expect(await requestPersistentStorage(persisted as never)).toBe(true)
    expect(persisted.persist).not.toHaveBeenCalled()

    const denied = { persisted: vi.fn(async () => false), persist: vi.fn(async () => false) }
    expect(await requestPersistentStorage(denied as never)).toBe(false)
    expect(denied.persist).toHaveBeenCalledTimes(1)
  })

  it('posts used keys to the service worker', async () => {
    const container = {
      controller: {
        postMessage(msg: unknown, [port]: MessagePort[]) {
          port.postMessage({ msg })
        }
      }
    }
    const reply = await contentCacheTouch(['k1', 'k2'], container as never, 1000)
    expect(reply).toEqual({ msg: { type: 'content-cache-touch', keys: ['k1', 'k2'] } })
  })

  it('does not hold a quota-error trim behind a touch the worker never answers', async () => {
    vi.useFakeTimers()
    const posted: string[] = []
    const container = {
      controller: {
        postMessage(msg: { type: string }) {
          posted.push(msg.type)
        }
      }
    }
    const touch = contentCacheTouch(['k1'], container as never)
    let trimDone = false
    const trim = contentCacheTrim(100, true, container as never).then(() => (trimDone = true))
    // both are posted at once, in call order: the worker's queue runs the touch first
    expect(posted).toEqual(['content-cache-touch', 'content-cache-trim'])
    await vi.advanceTimersByTimeAsync(5_000)
    await trim
    expect(trimDone).toBe(true)
    await vi.advanceTimersByTimeAsync(25_000)
    expect(await touch).toBeNull()
  })

  it('installs the page functions', () => {
    const target: Record<string, unknown> = {}
    installContentCachePageFunctions(target as never)
    expect(typeof target.__contentCacheTrim).toBe('function')
    expect(typeof target.__contentCacheTouch).toBe('function')
  })

  it('warns once without a controlling worker and asks again when one takes control', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const listeners: Record<string, () => void> = {}
    const posted: unknown[] = []
    const container: { controller: unknown; addEventListener: (type: string, fn: () => void) => void } = {
      controller: null,
      addEventListener: (type, fn) => (listeners[type] = fn)
    }
    try {
      const target: Record<string, (capBytes: number, quotaError: boolean) => Promise<unknown>> = {}
      installContentCachePageFunctions(target as never, container as never)
      expect(await target.__contentCacheTrim(4096, false)).toBeNull()
      expect(await target.__contentCacheTrim(8192, false)).toBeNull()
      expect(warn).toHaveBeenCalledTimes(1)

      container.controller = {
        postMessage(msg: unknown, [port]: MessagePort[]) {
          posted.push(msg)
          port.postMessage({ ok: true })
        }
      }
      listeners.controllerchange()
      await vi.waitFor(() => expect(posted).toEqual([{ type: 'content-cache-trim', capBytes: 8192, quotaError: false }]))
      // only once per unanswered request
      listeners.controllerchange()
      expect(posted).toHaveLength(1)
    } finally {
      warn.mockRestore()
    }
  })

  it('forwards the cap and the quota-error flag from the installed page function', async () => {
    const posted: unknown[] = []
    vi.stubGlobal('navigator', {
      serviceWorker: {
        controller: {
          postMessage(msg: unknown, [port]: MessagePort[]) {
            posted.push(msg)
            port.postMessage({ ok: true })
          }
        }
      }
    })
    try {
      const target: Record<string, (capBytes: number, quotaError: boolean) => Promise<unknown>> = {}
      installContentCachePageFunctions(target as never)
      expect(await target.__contentCacheTrim(4096, true)).toEqual({ ok: true })
      expect(posted).toEqual([{ type: 'content-cache-trim', capBytes: 4096, quotaError: true }])
      // and the uses a quota-error trim carries, when there are any
      expect(await (target.__contentCacheTrim as (...args: unknown[]) => Promise<unknown>)(4096, true, ['https://a.example/x'])).toEqual({ ok: true })
      expect(posted[1]).toEqual({ type: 'content-cache-trim', capBytes: 4096, quotaError: true, usedNow: ['https://a.example/x'] })
    } finally {
      vi.unstubAllGlobals()
    }
  })
})
