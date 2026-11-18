import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ValidationSeverity, type ValidationResult } from './types'

// An in-memory stand-in for IndexedDB behind idb-keyval's API.
const stores = new Map<string, Map<string, unknown>>()
vi.mock('idb-keyval', () => ({
  createStore: (db: string, name: string) => {
    const key = `${db}/${name}`
    if (!stores.has(key)) stores.set(key, new Map())
    return key
  },
  get: async (key: string, store: string) => stores.get(store)!.get(key),
  set: async (key: string, value: unknown, store: string) => void stores.get(store)!.set(key, value),
  del: async (key: string, store: string) => void stores.get(store)!.delete(key),
  clear: async (store: string) => stores.get(store)!.clear()
}))

const { CACHE_REVISION, cacheKey, cachedRun, deleteCached, resetCacheConnection } = await import('./cache')

const result: ValidationResult = {
  issues: [{ code: 'triangle-count', severity: ValidationSeverity.ERROR, message: 'Too many triangles' }]
}
const key = ['model-validation', ['item', [['bafyM', [], []]]], 'wearable', 'hat', [], null] as const
const results = () => stores.get('item-validation/results')!

beforeEach(() => {
  stores.clear()
  resetCacheConnection()
  vi.stubGlobal('indexedDB', {})
})

describe('validation cache', () => {
  it('serves a stored result without running the check again', async () => {
    const run = vi.fn().mockResolvedValue(result)
    await expect(cachedRun(key, run)).resolves.toEqual(result)
    await expect(cachedRun(key, run)).resolves.toEqual(result)
    expect(run).toHaveBeenCalledTimes(1)
  })

  it('never stores a check that failed', async () => {
    await expect(cachedRun(key, () => Promise.reject(new Error('404')))).rejects.toThrow('404')
    const run = vi.fn().mockResolvedValue(result)
    await cachedRun(key, run)
    expect(run).toHaveBeenCalledTimes(1)
  })

  it('drops every entry when the validator version or our revision changes', async () => {
    await cachedRun(key, async () => result)
    results().set('stamp', JSON.stringify([CACHE_REVISION - 1, 'old']))
    resetCacheConnection()
    const run = vi.fn().mockResolvedValue(result)
    await cachedRun(key, run)
    expect(run).toHaveBeenCalledTimes(1)
  })

  it('checks again once an entry is deleted', async () => {
    await cachedRun(key, async () => result)
    await deleteCached([cacheKey(key)])
    const run = vi.fn().mockResolvedValue({ issues: [] })
    await expect(cachedRun(key, run)).resolves.toEqual({ issues: [] })
  })

  it('never reads results written by a tab running another rule book version', async () => {
    const run = vi.fn().mockResolvedValue(result)
    await cachedRun(key, run)
    // What a stale tab would have written under the same query key.
    results().set(JSON.stringify([JSON.stringify([CACHE_REVISION, 'older']), key]), { issues: [] })
    await expect(cachedRun(key, run)).resolves.toEqual(result)
    expect(run).toHaveBeenCalledTimes(1)
  })

  it('works without IndexedDB, just unpersisted', async () => {
    vi.stubGlobal('indexedDB', undefined)
    const run = vi.fn().mockResolvedValue(result)
    await cachedRun(key, run)
    await cachedRun(key, run)
    expect(run).toHaveBeenCalledTimes(2)
  })

  it('does not persist in-memory sources', async () => {
    const run = vi.fn().mockResolvedValue(result)
    await cachedRun(null, run)
    await cachedRun(null, run)
    expect(run).toHaveBeenCalledTimes(2)
  })
})
