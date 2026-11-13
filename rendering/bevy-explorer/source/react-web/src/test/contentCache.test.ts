// The page side of the content cache's upkeep: relays the engine's trim request to the controlling
// service worker and asks for persistent storage.
import { afterEach, describe, expect, it, vi } from 'vitest'
// plain JS served as is by the engine page (deploy/web/engine), so it has no declarations
// @ts-expect-error TS7016
import { contentCacheTrim, installContentCachePageFunctions, requestPersistentStorage } from '../../../deploy/web/engine/content_cache.js'

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

  it('asks for persistence only when not persisted', async () => {
    const persisted = { persisted: vi.fn(async () => true), persist: vi.fn(async () => false) }
    expect(await requestPersistentStorage(persisted as never)).toBe(true)
    expect(persisted.persist).not.toHaveBeenCalled()

    const denied = { persisted: vi.fn(async () => false), persist: vi.fn(async () => false) }
    expect(await requestPersistentStorage(denied as never)).toBe(false)
    expect(denied.persist).toHaveBeenCalledTimes(1)
  })

  it('installs the trim page function', () => {
    const target: Record<string, unknown> = {}
    installContentCachePageFunctions(target as never)
    expect(typeof target.__contentCacheTrim).toBe('function')
  })
})
