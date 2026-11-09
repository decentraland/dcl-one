// The bridge's community places: the form's picker (the user's own places, the community's linked
// ones and the ids the places service did not return), the card's places with the user's own marked,
// and the card's place delete.
import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { CommunityDetailMessage, CommunityPlaceOptionsMessage, PageToScene, SceneToPage } from '../engine/protocol'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))
const { signed, getCompactProfiles } = vi.hoisted(() => ({ signed: vi.fn(), getCompactProfiles: vi.fn() }))
vi.mock('../../bridge-scene/src/http', () => ({
  serviceBase: async (name: string) => `https://${name}.example`,
  signed,
  getJson: vi.fn(async () => undefined),
  signedForm: vi.fn(),
  dataUrlBytes: vi.fn(),
  HttpError: class extends Error {}
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getCompactProfiles, social: {} } }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/domains/profile', () => ({
  fetchIdentities: async () => (address: string) => ({ name: address, hasClaimedName: false })
}))

const { registerCommunities } = await import('../../bridge-scene/src/domains/communities')

const DESTINATIONS = 'https://places.example/api/destinations'
const LINKED = 'https://socialApi.example/v1/communities/c1/places?limit=100&offset=0'
const land = { id: 'land-1', title: 'Plaza', base_position: '1,2', owner: '0xME' }
const world = { id: 'world-1', title: 'Mansion', world_name: 'me.dcl.eth', base_position: '0,0', owner: '0xother' }

function fakeCtx(): { sent: SceneToPage[]; req: (msg: PageToScene) => Promise<void> } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: () => undefined
  }
  registerCommunities(ctx)
  return {
    sent,
    req: async (msg) => {
      await Promise.all((handlers.get(msg.kind) ?? []).map((h) => h(msg)))
    }
  }
}

/** Answer `signed` by url and method; anything else is undefined (an empty answer). */
function serve(routes: Record<string, unknown>): void {
  signed.mockImplementation(async (url: string, method = 'GET') => {
    const answer = routes[`${method} ${url}`]
    if (answer instanceof Error) throw answer
    return answer
  })
}

const options = (sent: SceneToPage[]): CommunityPlaceOptionsMessage => {
  const m = sent.find((s) => s.kind === 'communityPlaceOptions')
  if (m?.kind !== 'communityPlaceOptions') throw new Error('no place options sent')
  return m
}
const detail = (sent: SceneToPage[]): CommunityDetailMessage | undefined => {
  const m = sent.filter((s) => s.kind === 'communityDetail').at(-1)
  return m?.kind === 'communityDetail' ? m : undefined
}

beforeEach(() => {
  signed.mockReset()
  getCompactProfiles.mockReset()
  getCompactProfiles.mockResolvedValue([{ pointer: '0xme', name: 'Me' }, { pointer: '0xother', name: 'Other' }])
  player = { userId: '0xme', isGuest: false }
})

describe('bridge community place options', () => {
  it("lists the user's places by owner, labelled as unity-explorer's", async () => {
    serve({ [`GET ${DESTINATIONS}?owner=0xme`]: [land, world] })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityPlaceOptions' })
    expect(options(h.sent)).toEqual({
      kind: 'communityPlaceOptions',
      communityId: undefined,
      owned: [
        { id: 'land-1', isWorld: false, label: 'Plaza (1,2)', ownerName: 'Me' },
        { id: 'world-1', isWorld: true, label: 'Mansion (me.dcl.eth)', ownerName: 'Other' }
      ],
      linked: [],
      hiddenLinkedIds: [],
      linkedFailed: false,
      errors: []
    })
  })

  // An empty owner is no filter at all to the places service: every place would be offered.
  it('never reads places without a signed-in user', async () => {
    player = null
    serve({})
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityPlaceOptions' })
    expect(signed).not.toHaveBeenCalled()
    expect(options(h.sent).errors).toEqual(['There was an error getting places. Please try again.'])
  })

  it("says so when no owner's name could be read, and lists the places without names", async () => {
    getCompactProfiles.mockResolvedValue([])
    serve({ [`GET ${DESTINATIONS}?owner=0xme`]: [land] })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityPlaceOptions' })
    expect(options(h.sent).owned[0].ownerName).toBe('')
    expect(options(h.sent).errors).toEqual(['There was an error getting owners names. Please try again.'])
  })

  it('keeps the linked places the places service did not return, to be sent on every save', async () => {
    const h = fakeCtx()
    serve({
      [`GET ${DESTINATIONS}?owner=0xme`]: [land],
      [`GET ${LINKED}`]: { results: [{ id: 'land-1' }, { id: 'disabled-1' }], total: 2 },
      [`POST ${DESTINATIONS}`]: [land]
    })
    await h.req({ kind: 'getCommunityPlaceOptions', communityId: 'c1' })
    const m = options(h.sent)
    expect(m.linked.map((p) => p.id)).toEqual(['land-1'])
    expect(m.hiddenLinkedIds).toEqual(['disabled-1'])
    expect(m.linkedFailed).toBe(false)
    expect(signed).toHaveBeenCalledWith(DESTINATIONS, 'POST', ['land-1', 'disabled-1'])
  })

  it('reads every page of linked places', async () => {
    const page = (from: number, n: number) => ({ results: Array.from({ length: n }, (_, i) => ({ id: `p${from + i}` })), total: 150 })
    serve({
      [`GET ${DESTINATIONS}?owner=0xme`]: [],
      [`GET ${LINKED}`]: page(0, 100),
      ['GET https://socialApi.example/v1/communities/c1/places?limit=100&offset=100']: page(100, 50),
      [`POST ${DESTINATIONS}`]: []
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityPlaceOptions', communityId: 'c1' })
    expect(options(h.sent).hiddenLinkedIds).toHaveLength(150)
  })

  it('marks the linked places unreadable when the read fails, so the form keeps them as they are', async () => {
    serve({ [`GET ${DESTINATIONS}?owner=0xme`]: [], [`GET ${LINKED}`]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityPlaceOptions', communityId: 'c1' })
    expect(options(h.sent).linkedFailed).toBe(true)
    expect(options(h.sent).errors).toEqual(['There was an error getting the community places. Please try again.'])
  })
})

describe('bridge community card places', () => {
  const cardPlaces = 'https://socialApi.example/v1/communities/c1/places?limit=20'

  it("reads each place from the places service, marks the user's own and names a World", async () => {
    serve({ [`GET ${cardPlaces}`]: { results: [{ id: 'land-1' }, { id: 'world-1' }] }, [`POST ${DESTINATIONS}`]: [world, land] })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityDetail', id: 'c1' })
    expect(detail(h.sent)?.places).toEqual([
      { id: 'land-1', title: 'Plaza', thumbnail: undefined, positions: '1,2', likeRate: undefined, isMine: true },
      { id: 'world-1', title: 'Mansion', thumbnail: undefined, positions: 'me.dcl.eth', likeRate: undefined, isMine: false }
    ])
  })

  it("falls back to the social service's rows when the places service fails", async () => {
    serve({ [`GET ${cardPlaces}`]: { results: [{ id: 'land-1', title: 'Plaza (social)' }] }, [`POST ${DESTINATIONS}`]: new Error('HTTP 503') })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityDetail', id: 'c1' })
    expect(detail(h.sent)?.places.map((p) => p.title)).toEqual(['Plaza (social)'])
  })

  it('deletes a place, then sends the card its places again', async () => {
    serve({ ['DELETE https://socialApi.example/v1/communities/c1/places/land-1']: undefined, [`GET ${cardPlaces}`]: { results: [] } })
    const h = fakeCtx()
    await h.req({ kind: 'removeCommunityPlace', id: 'c1', placeId: 'land-1' })
    expect(signed).toHaveBeenCalledWith('https://socialApi.example/v1/communities/c1/places/land-1', 'DELETE')
    expect(detail(h.sent)?.id).toBe('c1')
    expect(h.sent.some((s) => s.kind === 'communityActionFailed')).toBe(false)
  })

  it('says on the card when the delete fails', async () => {
    serve({ ['DELETE https://socialApi.example/v1/communities/c1/places/land-1']: new Error('HTTP 403') })
    const h = fakeCtx()
    await h.req({ kind: 'removeCommunityPlace', id: 'c1', placeId: 'land-1' })
    expect(h.sent).toContainEqual({ kind: 'communityActionFailed', id: 'c1', action: 'removePlace', message: 'HTTP 403' })
    expect(detail(h.sent)).toBeUndefined()
  })
})
