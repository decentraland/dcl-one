import { afterEach, describe, expect, it, vi } from 'vitest'
import { serviceUrl } from '@hud-host'
import { fetchCrowd, fetchDestinationsById, fetchWhereabouts } from '../features/lobby/lobbyApi'
import { fixMapPublicReads } from '../features/map/mapApi'
import { checkRealm } from '../lib/realmCheck'

// A refused read releases its body: an unread stream holds the connection until it is collected.
function refused(status: number): { response: Response; cancel: ReturnType<typeof vi.fn> } {
  const cancel = vi.fn(async () => undefined)
  const response = { ok: false, status, body: { cancel }, json: async () => ({}) } as unknown as Response
  return { response, cancel }
}

const ok = (body: unknown): Response => ({ ok: true, status: 200, body: null, json: async () => body }) as unknown as Response

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('refused server reads', () => {

  it('releases the body of each friend in no world (404) and lists them as unknown', async () => {
    const refusals = [refused(404), refused(404)]
    let next = 0
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => (url.includes('/comms/peers') ? ok({ peers: [] }) : refusals[next++].response))
    )
    const where = await fetchWhereabouts(['0xA', '0xB'])
    expect(refusals.map((r) => r.cancel.mock.calls.length)).toEqual([1, 1])
    expect(where.get('0xa')).toEqual({ kind: 'unknown' })
    expect(where.get('0xb')).toEqual({ kind: 'unknown' })
  })

  it('releases the body of a refused peers read and still asks the worlds', async () => {
    const peers = refused(500)
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => (url.includes('/comms/peers') ? peers.response : ok({ world: 'mana.dcl.eth' })))
    )
    const where = await fetchWhereabouts(['0xA'])
    expect(peers.cancel).toHaveBeenCalledOnce()
    expect(where.get('0xa')).toEqual({ kind: 'world', world: 'mana.dcl.eth', parcel: null })
  })

  it('asks the worlds when the peers read fails outright', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        if (url.includes('/comms/peers')) throw new TypeError('network')
        return ok({ world: 'mana.dcl.eth' })
      })
    )
    expect((await fetchWhereabouts(['0xA'])).get('0xa')).toEqual({ kind: 'world', world: 'mana.dcl.eth', parcel: null })
  })

  it('asks the worlds a few at a time', async () => {
    let running = 0
    let most = 0
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        if (url.includes('/comms/peers')) return ok({ peers: [] })
        most = Math.max(most, ++running)
        await new Promise((resolve) => setTimeout(resolve, 1))
        running--
        return ok({})
      })
    )
    const where = await fetchWhereabouts(Array.from({ length: 10 }, (_, i) => `0x${i}`))
    expect(most).toBe(4)
    expect(where.size).toBe(10)
  })

  it('reads the comms peers in batches of 100 wallets', async () => {
    const asked: string[][] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const ids = new URL(url).searchParams.getAll('id')
        asked.push(ids)
        return ok({ peers: ids.map((address) => ({ address, position: [16, 0, 32] })) })
      })
    )
    const wallets = Array.from({ length: 250 }, (_, i) => `0x${i}`)
    const where = await fetchWhereabouts(wallets)
    expect(asked.map((ids) => ids.length)).toEqual([100, 100, 50])
    expect(where.get('0x249')).toEqual({ kind: 'genesis', parcel: { x: 1, y: 2 } })
  })

  it('reads the most crowded scene from the archipelago stats, and releases a refused read', async () => {
    const read = vi.fn(async () => ok([{ baseCoords: [12, -3] }]))
    vi.stubGlobal('fetch', read)
    await expect(fetchCrowd()).resolves.toEqual({ x: 12, y: -3 })
    expect(read).toHaveBeenCalledWith(`${serviceUrl('archipelagoStats')}/hot-scenes`, { signal: expect.any(AbortSignal) })

    const hot = refused(502)
    vi.stubGlobal('fetch', vi.fn(async () => hot.response))
    await expect(fetchCrowd()).rejects.toThrow('hot-scenes: 502')
    expect(hot.cancel).toHaveBeenCalledOnce()
  })

  it('gives up on a stalled places or hot-scenes read', async () => {
    const timeout = new AbortController()
    const asked = vi.spyOn(AbortSignal, 'timeout').mockReturnValue(timeout.signal)
    const stalled = vi.fn((_url: string, init?: RequestInit) => new Promise<Response>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('timed out', 'TimeoutError')))))
    vi.stubGlobal('fetch', stalled)
    const reads = [fetchDestinationsById(['p']).catch(() => 'failed'), fetchCrowd().catch(() => 'failed')]
    expect(stalled).toHaveBeenCalledTimes(2)
    expect(asked).toHaveBeenCalledTimes(2)
    timeout.abort()
    await expect(Promise.all(reads)).resolves.toEqual(['failed', 'failed'])
    asked.mockRestore()
  })
})

describe('lobby reads shared with the map', () => {
  it("answers friends' whereabouts from the map's fixed reads in mock mode, without the network", async () => {
    const fetch = vi.fn(async () => ok({}))
    vi.stubGlobal('fetch', fetch)
    fixMapPublicReads({ pois: [], peers: [{ address: '0xabc', x: 40, z: -20 }], worlds: { '0xdef': 'boedo.dcl.eth' } })
    try {
      const where = await fetchWhereabouts(['0xABC', '0xDEF', '0x123'])
      expect(where.get('0xabc')).toEqual({ kind: 'genesis', parcel: { x: 2, y: -2 } })
      expect(where.get('0xdef')).toEqual({ kind: 'world', world: 'boedo.dcl.eth', parcel: null })
      expect(where.get('0x123')).toEqual({ kind: 'unknown' })
      expect(fetch).not.toHaveBeenCalled()
    } finally {
      fixMapPublicReads(null)
    }
  })

  it("tells Go to's world that does not exist from a worlds server that cannot answer", async () => {
    const world = `${serviceUrl('worldsServer')}/world/ghost.eth`
    const missing = refused(404)
    vi.stubGlobal('fetch', vi.fn(async () => missing.response))
    await expect(checkRealm(world)).resolves.toBe('not-found')
    expect(missing.cancel).toHaveBeenCalledOnce()
    const down = refused(503)
    const read = vi.fn(async () => down.response)
    vi.stubGlobal('fetch', read)
    await expect(checkRealm(world)).resolves.toBe('unreachable')
    expect(String((read.mock.calls[0] as unknown[])[0])).toBe(`${world}/about`)
  })
})
