// The service worker's upkeep of the content cache: on a `content-cache-trim` message it deletes
// the least recently used entries of `ipfs-path-cache-v2` (oldest written when no use is recorded)
// while the cache is over its cap, and touches no other cache. A `content-cache-touch` message
// records uses.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { IDBFactory } from 'fake-indexeddb'
import { describe, expect, it, vi } from 'vitest'

const CONTENT_CACHE = 'ipfs-path-cache-v2'
const OTHER_CACHES = ['dcl-editor-preview-v1', 'dcl-editor-scene-v1', 'dcl-editor-storage-v1']

type Message = { type: string; capBytes?: unknown; quotaError?: boolean }
type Pure = {
  contentCacheTarget: (input: { capBytes: number; quota: number; cachesUsage: number; quotaError: boolean }) => {
    cap: number
    bytesToFree: number
  }
  pickEvictions: (sized: { key: string; size: number }[], bytesToFree: number) => { picked: string[]; freed: number }
  contentCacheEvictionOrder: (requests: string[], lastUsed: Map<string, number>) => string[]
}

const entryUrl = (n: number | string) => `https://shared.invalid/bafkrei${n}`

function entry(size: number, withLength = true) {
  const headers: Record<string, string> = withLength ? { 'Content-Length': String(size) } : {}
  return new Response('x'.repeat(size), { headers })
}

const keyOf = (key: string | Request) => (typeof key === 'string' ? key : key.url)

// the spec's cache: insertion ordered, a put removes the old item and appends the new one
function fakeCache(seed: [string, Response][] = []) {
  const map = new Map<string, Response>(seed)
  return {
    map,
    keys: async () => [...map.keys()].map((url) => new Request(url)),
    match: async (key: string | Request) => map.get(keyOf(key))?.clone(),
    delete: async (key: string | Request) => map.delete(keyOf(key)),
    put: async (key: string | Request, res: Response) => {
      map.delete(keyOf(key))
      map.set(keyOf(key), res)
    }
  }
}
type FakeCache = ReturnType<typeof fakeCache>

function serviceWorker(options: { quota?: number; usage?: number; usageDetails?: object | null; others?: boolean; otherSize?: number; persisted?: 'absent' | 'rejects'; indexedDB?: unknown } = {}) {
  const listeners: Record<string, (event: unknown) => void> = {}
  const content = fakeCache()
  const fakes = new Map<string, FakeCache>([[CONTENT_CACHE, content]])
  if (options.others) {
    for (const name of OTHER_CACHES) fakes.set(name, fakeCache([[`https://editor.invalid/${name}`, entry(options.otherSize ?? 1000)]]))
  }
  const opened: string[] = []
  const caches = {
    open: vi.fn(async (name: string) => {
      opened.push(name)
      let cache = fakes.get(name)
      if (!cache) fakes.set(name, (cache = fakeCache()))
      return cache
    })
  }
  const cachesUsage = () => {
    let total = 0
    for (const cache of fakes.values()) {
      for (const response of cache.map.values()) total += Number(response.headers.get('Content-Length') ?? 0)
    }
    return total
  }
  const estimate = vi.fn(async () => ({
    usage: options.usage ?? cachesUsage(),
    quota: options.quota ?? 1e9,
    ...(options.usageDetails === null ? {} : { usageDetails: options.usageDetails ?? { caches: cachesUsage() } })
  }))
  const storage: Record<string, unknown> = { estimate }
  if (options.persisted === 'rejects') storage.persisted = async () => Promise.reject(new Error('not allowed'))
  else if (options.persisted !== 'absent') storage.persisted = async () => false
  const navigator = { storage }
  const self = {
    addEventListener: (type: string, fn: (event: unknown) => void) => (listeners[type] = fn),
    registration: { scope: 'https://play.example/' }
  }
  const source = readFileSync(resolve(__dirname, '../../../deploy/web/service_worker.js'), 'utf8')
  const load = new Function(
    'self',
    'caches',
    'fetch',
    'console',
    'navigator',
    'indexedDB',
    source + '\nreturn { contentCacheTarget, pickEvictions, contentCacheEvictionOrder };'
  ) as (...args: unknown[]) => Pure
  const indexedDB = options.indexedDB ?? new IDBFactory()
  const pure = load(self, caches, vi.fn(), { log: vi.fn(), warn: vi.fn() }, navigator, indexedDB)

  const send = async (data: unknown) => {
    const port = { postMessage: vi.fn() }
    const waitUntil = vi.fn()
    listeners.message({ data, ports: [port], waitUntil })
    if (waitUntil.mock.calls.length) await waitUntil.mock.calls[0][0]
    return { port, waitUntil }
  }
  // like `send`, but does not wait: the replies come in `done`, in the order they were posted
  const sendAsync = (data: unknown) => {
    const port = { postMessage: vi.fn((message: unknown) => replies.push({ data, message })) }
    const waitUntil = vi.fn()
    listeners.message({ data, ports: [port], waitUntil })
    return { port, done: waitUntil.mock.calls.length ? (waitUntil.mock.calls[0][0] as Promise<unknown>) : Promise.resolve() }
  }
  const replies: { data: unknown; message: any }[] = []
  const seed = (n: number, size = 100, withLength = true) => content.map.set(entryUrl(n), entry(size, withLength))
  const trim = (message: Partial<Message> & { capBytes: unknown }) => send({ type: 'content-cache-trim', ...message })
  const touch = (keys: unknown) => send({ type: 'content-cache-touch', keys })
  const remaining = () => [...content.map.keys()].map((url) => url.replace('https://shared.invalid/', ''))
  return { pure, send, sendAsync, replies, trim, touch, indexedDB, seed, content, fakes, caches, opened, estimate, remaining }
}

describe('content cache target', () => {
  it('has nothing to free under the cap', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheTarget({ capBytes: 1000, quota: 1e9, cachesUsage: 1000, quotaError: false })).toEqual({
      cap: 1000,
      bytesToFree: 0
    })
  })

  it('frees down to 90% of the cap when over it', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheTarget({ capBytes: 1000, quota: 1e9, cachesUsage: 1200, quotaError: false }).bytesToFree).toBe(300)
  })

  it('caps at half the quota', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheTarget({ capBytes: 1000, quota: 1000, cachesUsage: 600, quotaError: false })).toEqual({
      cap: 500,
      bytesToFree: 150
    })
  })

  it('frees a quarter after a quota error even under the cap', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheTarget({ capBytes: 1000, quota: 1e9, cachesUsage: 400, quotaError: true }).bytesToFree).toBe(100)
  })
})

describe('eviction choice', () => {
  it('takes the leading entries until enough is freed', () => {
    const { pure } = serviceWorker()
    const sized = [
      { key: 'a', size: 50 },
      { key: 'b', size: 50 },
      { key: 'c', size: 50 }
    ]
    expect(pure.pickEvictions(sized, 60)).toEqual({ picked: ['a', 'b'], freed: 100 })
    expect(pure.pickEvictions(sized, 0)).toEqual({ picked: [], freed: 0 })
    expect(pure.pickEvictions(sized, 1000).picked).toEqual(['a', 'b', 'c'])
  })
})

describe('content cache trim message', () => {
  it('evicts the oldest-written entries until the cache is under the cap', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 500 })
    expect(sw.remaining()).toEqual(['bafkrei6', 'bafkrei7', 'bafkrei8', 'bafkrei9'])
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ evictedEntries: 6, evictedBytes: 600, capBytes: 500, targetUnreachable: false, preempted: false })
    )
  })

  it('moves an entry written again to the newest end', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 3; n++) sw.seed(n)
    await sw.content.put(entryUrl(0), entry(100))
    await sw.trim({ capBytes: 250 })
    expect(sw.remaining()).toEqual(['bafkrei2', 'bafkrei0'])
  })

  it('does not open the cache when under the cap', async () => {
    const sw = serviceWorker()
    sw.seed(0)
    const { port } = await sw.trim({ capBytes: 1000 })
    expect(sw.caches.open).not.toHaveBeenCalled()
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ evictedEntries: 0 }))
  })

  it('never opens or changes another cache', async () => {
    const sw = serviceWorker({ others: true })
    for (let n = 0; n < 5; n++) sw.seed(n)
    await sw.trim({ capBytes: 10 })
    expect(sw.remaining()).toEqual([])
    expect(new Set(sw.opened)).toEqual(new Set([CONTENT_CACHE]))
    for (const name of OTHER_CACHES) expect(sw.fakes.get(name)!.map.size).toBe(1)
  })

  it('sizes an entry without Content-Length by its body', async () => {
    const sw = serviceWorker({ usage: 1000, usageDetails: { caches: 1000 } })
    sw.seed(0, 300, false)
    sw.seed(1)
    sw.seed(2)
    // the worker is told the cache holds 1000 bytes against a cap of 100
    const { port } = await sw.trim({ capBytes: 100 })
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ evictedEntries: 3, evictedBytes: 500 }))
  })

  it('answers a bad cap with an error and keeps serving later trims', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 4; n++) sw.seed(n)
    const bad = await sw.trim({ capBytes: 'big' })
    expect(bad.port.postMessage).toHaveBeenCalledWith({ error: expect.any(String) })
    const good = await sw.trim({ capBytes: 200 })
    expect(good.port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ evictedEntries: 3, evictedBytes: 300 }))
  })

  it('ignores other messages', async () => {
    const sw = serviceWorker()
    const { waitUntil } = await sw.send({ type: 'something-else' })
    expect(sw.estimate).not.toHaveBeenCalled()
    expect(waitUntil).not.toHaveBeenCalled()
  })

  it('trims a cache that is under its cap after a quota error', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 4; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 1000, quotaError: true })
    expect(sw.remaining()).toEqual(['bafkrei1', 'bafkrei2', 'bafkrei3'])
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ evictedEntries: 1, evictedBytes: 100 }))
  })

  it('reads Cache Storage usage from usageDetails, and 0 when it is left out', async () => {
    const sw = serviceWorker({ usage: 5000, usageDetails: {} })
    for (let n = 0; n < 3; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 1000 })
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ cachesUsage: 0, evictedEntries: 0 }))
  })
  it('falls back to usage when usageDetails is missing altogether', async () => {
    const sw = serviceWorker({ usage: 1000, usageDetails: null })
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 500 })
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ cachesUsage: 1000, evictedEntries: 6 }))
  })

  it('still trims when navigator.storage.persisted is missing', async () => {
    const sw = serviceWorker({ persisted: 'absent' })
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 500 })
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ evictedEntries: 6, persisted: false })
    )
  })

  it('still trims when navigator.storage.persisted rejects', async () => {
    const sw = serviceWorker({ persisted: 'rejects' })
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 500 })
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ evictedEntries: 6, persisted: false })
    )
  })

  it('does not evict content when other storage alone keeps usage over the cap', async () => {
    const sw = serviceWorker({ others: true })
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 2000 })
    expect(sw.remaining()).toHaveLength(10)
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ cachesUsage: 4000, evictedEntries: 0, evictedBytes: 0, targetUnreachable: true })
    )
  })

  it('frees a quarter of the content after a quota error when other storage fills the cap', async () => {
    const sw = serviceWorker({ others: true })
    for (let n = 0; n < 10; n++) sw.seed(n)
    const { port } = await sw.trim({ capBytes: 2000, quotaError: true })
    expect(sw.remaining()).toHaveLength(7)
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ evictedEntries: 3, evictedBytes: 300, targetUnreachable: true })
    )
  })

  it('cancels the body of an entry sized by its Content-Length', async () => {
    const sw = serviceWorker()
    const cancel = vi.fn()
    for (let n = 0; n < 5; n++) {
      const body = new ReadableStream({ cancel })
      sw.content.map.set(entryUrl(n), new Response(body, { headers: { 'Content-Length': '100' } }))
    }
    // hand out the stored response itself, so the spy sees the cancel
    sw.content.match = async (key: string | Request) => sw.content.map.get(keyOf(key))
    await sw.trim({ capBytes: 100 })
    expect(cancel).toHaveBeenCalledTimes(5)
  })

  it('lets a quota-error request run before pending regular trims and stops a running regular one', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 300; n++) sw.seed(n)
    const first = sw.sendAsync({ type: 'content-cache-trim', capBytes: 1000 })
    const second = sw.sendAsync({ type: 'content-cache-trim', capBytes: 1000 })
    const forced = sw.sendAsync({ type: 'content-cache-trim', capBytes: 1000, quotaError: true })
    await Promise.all([first.done, second.done, forced.done])
    expect(first.port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ preempted: true, evictedEntries: 0 }))
    expect(sw.replies.map((r) => (r.data as Message).quotaError === true)).toEqual([false, true, false])
    expect(forced.port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ preempted: false }))
    expect(sw.remaining().length * 100).toBeLessThanOrEqual(1000)
  })
})

describe('eviction order', () => {
  it('puts entries without a recorded use first, oldest written first', () => {
    const { pure } = serviceWorker()
    const used = new Map([['a', 5], ['c', 1]])
    expect(pure.contentCacheEvictionOrder(['a', 'b', 'c', 'd'], used)).toEqual(['b', 'd', 'c', 'a'])
  })

  it('keeps write order between equal last uses', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheEvictionOrder(['b', 'a'], new Map([['a', 1], ['b', 1]]))).toEqual(['b', 'a'])
  })

  it('is the write order with no record', () => {
    const { pure } = serviceWorker()
    expect(pure.contentCacheEvictionOrder(['a', 'b', 'c'], new Map())).toEqual(['a', 'b', 'c'])
  })
})

const recordedKeys = (factory: IDBFactory) =>
  new Promise<unknown[]>((resolve, reject) => {
    const open = factory.open('dcl-content-cache-index-v1')
    open.onerror = () => reject(open.error)
    open.onsuccess = () => {
      const request = open.result.transaction('lastUsed').objectStore('lastUsed').getAllKeys()
      request.onsuccess = () => resolve(request.result)
      request.onerror = () => reject(request.error)
    }
  })

describe('content cache last use', () => {
  it('keeps a read entry longer than a later-written unread one', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 4; n++) sw.seed(n)
    await sw.touch([entryUrl(0)])
    const { port } = await sw.trim({ capBytes: 300 })
    expect(sw.remaining()).toEqual(['bafkrei0', 'bafkrei3'])
    expect(port.postMessage).toHaveBeenCalledWith(expect.objectContaining({ evictedEntries: 2, indexedEntries: 1 }))
  })

  it('ranks uses recorded later later', async () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    try {
      const sw = serviceWorker()
      sw.seed(0)
      sw.seed(1)
      await sw.touch([entryUrl(1)])
      vi.setSystemTime(Date.now() + 60_000)
      await sw.touch([entryUrl(0)])
      await sw.trim({ capBytes: 150 })
      expect(sw.remaining()).toEqual(['bafkrei0'])
    } finally {
      vi.useRealTimers()
    }
  })

  it('falls back to write order without its index', async () => {
    const sw = serviceWorker({
      indexedDB: {
        open() {
          throw new Error('gone')
        }
      }
    })
    for (let n = 0; n < 4; n++) sw.seed(n)
    const touched = await sw.touch([entryUrl(0)])
    expect(touched.port.postMessage).toHaveBeenCalledWith({ error: expect.any(String) })
    const { port } = await sw.trim({ capBytes: 300 })
    expect(sw.remaining()).toEqual(['bafkrei2', 'bafkrei3'])
    const report = port.postMessage.mock.calls[0][0]
    expect(report).toMatchObject({ evictedEntries: 2, indexedEntries: 0 })
    expect(report.error).toBeUndefined()
  })

  it('prunes the records of entries that are gone', async () => {
    const sw = serviceWorker()
    sw.seed(0)
    sw.seed(1)
    await sw.touch(['https://shared.invalid/x', entryUrl(0)])
    await sw.trim({ capBytes: 100 })
    expect(sw.remaining()).toEqual([])
    expect(await recordedKeys(sw.indexedDB as IDBFactory)).toEqual([])
  })

  it('records only short https urls, and at most 4096', async () => {
    const sw = serviceWorker()
    const some = await sw.touch([entryUrl('a'), 7, null, 'b', entryUrl('b'), `https://shared.invalid/${'x'.repeat(3000)}`])
    expect(some.port.postMessage).toHaveBeenCalledWith({ recorded: 2 })
    const bad = await sw.touch('a')
    expect(bad.port.postMessage).toHaveBeenCalledWith({ error: expect.any(String) })
    const many = await sw.touch(Array.from({ length: 5000 }, (_, i) => entryUrl(`k${i}`)))
    expect(many.port.postMessage).toHaveBeenCalledWith({ recorded: 4096 })
    expect(await recordedKeys(sw.indexedDB as IDBFactory)).toHaveLength(4096 + 2)
  })

  it('records the own key of a plain http content server', async () => {
    const sw = serviceWorker()
    const { port } = await sw.touch(['http://192.168.0.5:8080/contents/bafkreihash'])
    expect(port.postMessage).toHaveBeenCalledWith({ recorded: 1 })
  })

  it('looks only at the first 4096 keys of a batch', async () => {
    const sw = serviceWorker()
    const keys = [...Array.from({ length: 4096 }, () => 7), entryUrl('late')]
    const { port } = await sw.touch(keys)
    expect(port.postMessage).toHaveBeenCalledWith({ recorded: 0 })
  })

  it('reads no index when a trim finds the cache under its cap', async () => {
    const open = vi.fn()
    const sw = serviceWorker({ indexedDB: { open } })
    sw.seed(0)
    await sw.trim({ capBytes: 1000 })
    expect(open).not.toHaveBeenCalled()
  })

  it('runs touches and trims in arrival order', async () => {
    const sw = serviceWorker()
    for (let n = 0; n < 4; n++) sw.seed(n)
    const touched = sw.touch([entryUrl(0)])
    const trimmed = sw.trim({ capBytes: 300 })
    await Promise.all([touched, trimmed])
    expect(sw.remaining()).toEqual(['bafkrei0', 'bafkrei3'])
  })
})

function neverOpens() {
  const requests: Record<string, unknown>[] = []
  return {
    requests,
    open() {
      const request: Record<string, unknown> = {}
      requests.push(request)
      return request
    }
  }
}

describe('content cache index that does not answer', () => {
  it('does not stop a trim: it gives up after 5 s and evicts in write order', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    try {
      const factory = neverOpens()
      const sw = serviceWorker({ indexedDB: factory })
      for (let n = 0; n < 4; n++) sw.seed(n)
      const trimmed = sw.trim({ capBytes: 300 })
      await vi.advanceTimersByTimeAsync(0)
      let done = false
      void trimmed.then(() => (done = true))
      await vi.advanceTimersByTimeAsync(4_999)
      expect(done).toBe(false)
      await vi.advanceTimersByTimeAsync(1)
      // the prune opens the index again and gives up the same way
      await vi.advanceTimersByTimeAsync(5_000)
      const { port } = await trimmed
      expect(sw.remaining()).toEqual(['bafkrei2', 'bafkrei3'])
      const report = port.postMessage.mock.calls[0][0]
      expect(report).toMatchObject({ evictedEntries: 2, indexedEntries: 0 })
      expect(report.error).toBeUndefined()
    } finally {
      vi.useRealTimers()
    }
  })

  it('holds a quota-error trim at most 1 s, and its reply does not wait for the prune', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    try {
      const factory = neverOpens()
      const sw = serviceWorker({ indexedDB: factory })
      for (let n = 0; n < 4; n++) sw.seed(n)
      const trimmed = sw.trim({ capBytes: 1000, quotaError: true })
      await vi.advanceTimersByTimeAsync(0)
      let done = false
      void trimmed.then(() => (done = true))
      await vi.advanceTimersByTimeAsync(999)
      expect(done).toBe(false)
      await vi.advanceTimersByTimeAsync(1)
      expect(done).toBe(true)
      const { port } = await trimmed
      // a quarter of 400 bytes, oldest written first
      expect(sw.remaining()).toEqual(['bafkrei1', 'bafkrei2', 'bafkrei3'])
      expect(port.postMessage.mock.calls[0][0]).toMatchObject({ evictedEntries: 1, indexedEntries: 0 })
      // the read given up on and the prune settle later, without an error
      await vi.advanceTimersByTimeAsync(10_000)
    } finally {
      vi.useRealTimers()
    }
  })

  it('closes a connection that opens after the wait was given up', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    try {
      const factory = neverOpens()
      const sw = serviceWorker({ indexedDB: factory })
      const touched = sw.touch([entryUrl(0)])
      await vi.advanceTimersByTimeAsync(5_000)
      expect((await touched).port.postMessage).toHaveBeenCalledWith({ error: expect.any(String) })
      const close = vi.fn()
      const request = factory.requests[0] as { onsuccess: () => void; result: unknown }
      request.result = { close }
      request.onsuccess()
      expect(close).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('falls back at once when the open is blocked, and closes it if it opens later', async () => {
    const factory = neverOpens()
    const sw = serviceWorker({ indexedDB: factory })
    for (let n = 0; n < 4; n++) sw.seed(n)
    const trimmed = sw.trim({ capBytes: 300 })
    await vi.waitFor(() => expect(factory.requests.length).toBe(1))
    const request = factory.requests[0] as { onblocked: () => void; onsuccess: () => void; result: unknown }
    request.onblocked()
    // the prune's open is blocked too
    await vi.waitFor(() => expect(factory.requests.length).toBe(2))
    ;(factory.requests[1] as { onblocked: () => void }).onblocked()
    await trimmed
    expect(sw.remaining()).toEqual(['bafkrei2', 'bafkrei3'])
    const close = vi.fn()
    request.result = { close }
    request.onsuccess()
    expect(close).toHaveBeenCalledTimes(1)
  })

  it('closes its connection when another page asks for an upgrade', async () => {
    const factory = neverOpens()
    const sw = serviceWorker({ indexedDB: factory })
    const touched = sw.touch([entryUrl(0)])
    await vi.waitFor(() => expect(factory.requests.length).toBe(1))
    const request = factory.requests[0] as { onsuccess: () => void; result: Record<string, unknown> }
    const close = vi.fn()
    request.result = { close, onversionchange: null, transaction: () => {
        throw new Error('stop')
      }
    }
    request.onsuccess()
    expect(typeof request.result.onversionchange).toBe('function')
    ;(request.result.onversionchange as () => void)()
    expect(close).toHaveBeenCalled()
    await touched
  })
})
