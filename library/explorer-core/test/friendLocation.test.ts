// Where a friend is, for Jump to Location (features/friends/friendLocation): Genesis City first,
// then the World the friend is connected to, as unity-explorer's online-users provider reads them.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { locateFriend } from '../features/friends/friendLocation'
import { fixMapPublicReads } from '../features/map/mapApi'

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

function stubFetch(answer: (url: string) => Response | Promise<Response>): ReturnType<typeof vi.fn> {
  const fn = vi.fn(async (url: string) => answer(url))
  vi.stubGlobal('fetch', fn)
  return fn
}

afterEach(() => {
  vi.unstubAllGlobals()
  fixMapPublicReads(null)
})

describe('locateFriend', () => {
  it("reads the friend's position from the Archipelago peers, asking by the lowercase address, and floors it to its parcel", async () => {
    const fetch = stubFetch(() => json({ ok: true, peers: [{ address: '0xabc', position: [-1, 0, 33], parcel: [-1, 2] }] }))
    expect(await locateFriend('0xABC')).toEqual({ kind: 'parcel', x: -1, y: 2 })
    expect(fetch).toHaveBeenCalledTimes(1)
    expect(String(fetch.mock.calls[0][0])).toMatch(/\/comms\/peers\?id=0xabc$/)
  })

  it('asks for the connected World when the friend is not in Genesis City', async () => {
    const fetch = stubFetch((url) => (url.includes('/comms/peers') ? json({ ok: true, peers: [] }) : json({ wallet: '0xabc', world: 'boedo.dcl.eth' })))
    expect(await locateFriend('0xabc')).toEqual({ kind: 'world', world: 'boedo.dcl.eth' })
    expect(String(fetch.mock.calls[1][0])).toMatch(/\/wallet\/0xabc\/connected-world$/)
  })

  it('skips a peer that is someone else', async () => {
    stubFetch((url) => (url.includes('/comms/peers') ? json({ peers: [{ address: '0xother', position: [0, 0, 0] }] }) : json({ message: 'not connected' }, 404)))
    expect(await locateFriend('0xabc')).toBeNull()
  })

  it('answers from the fixed reads in mock mode, without the network', async () => {
    const fetch = stubFetch(() => json({}, 500))
    fixMapPublicReads({ pois: [], peers: [{ address: '0xabc', x: 40, z: -20 }], worlds: { '0xdef': 'boedo.dcl.eth' } })
    expect(await locateFriend('0xABC')).toEqual({ kind: 'parcel', x: 2, y: -2 })
    expect(await locateFriend('0xDEF')).toEqual({ kind: 'world', world: 'boedo.dcl.eth' })
    expect(await locateFriend('0x123')).toBeNull()
    expect(fetch).not.toHaveBeenCalled()
  })

  it('is null when the friend is nowhere, or a read fails', async () => {
    stubFetch((url) => (url.includes('/comms/peers') ? json({ peers: [] }) : json({ message: 'not connected' }, 404)))
    expect(await locateFriend('0xabc')).toBeNull()
    stubFetch(() => json({}, 503))
    expect(await locateFriend('0xabc')).toBeNull()
    stubFetch(() => Promise.reject(new Error('offline')))
    expect(await locateFriend('0xabc')).toBeNull()
  })
})
