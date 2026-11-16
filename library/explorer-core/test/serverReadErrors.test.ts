import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchCrowd, fetchWhereabouts } from '../features/lobby/lobbyApi'
import { fetchWorldAccess } from '../features/places/worldAccess'

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
  it("reads a world's access as undecided and releases the body", async () => {
    const { response, cancel } = refused(503)
    vi.stubGlobal('fetch', vi.fn(async () => response))
    await expect(fetchWorldAccess('party.dcl.eth', null, [])).resolves.toBe('failed')
    expect(cancel).toHaveBeenCalledOnce()
  })

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

  it('releases the body of a refused peers or hot-scenes read before it throws', async () => {
    const peers = refused(500)
    vi.stubGlobal('fetch', vi.fn(async () => peers.response))
    await expect(fetchWhereabouts(['0xA'])).rejects.toThrow('peers: 500')
    expect(peers.cancel).toHaveBeenCalledOnce()

    const hot = refused(502)
    vi.stubGlobal('fetch', vi.fn(async () => hot.response))
    await expect(fetchCrowd()).rejects.toThrow('hot-scenes: 502')
    expect(hot.cancel).toHaveBeenCalledOnce()
  })
})
