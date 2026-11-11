// The bridge's communities: the form's place picker (the user's own places, the community's linked
// ones and the ids the places service did not return), the card's places with the user's own marked,
// the card's place delete, the member menu's actions, and the browse grid's pages.
import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { CommunityDetailMessage, CommunityPlaceOptionsMessage, PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))
const { signed, getCompactProfiles, serviceBase } = vi.hoisted(() => ({
  signed: vi.fn(),
  getCompactProfiles: vi.fn(),
  serviceBase: vi.fn(async (name: string) => `https://${name}.example`)
}))
vi.mock('../../bridge-scene/src/http', () => ({
  serviceBase,
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

// The card's member menu, as unity-explorer's MembersListController: the role PATCH (moderator,
// member, owner), the member DELETE and the ban POST, then the card's members read again.
describe('bridge community member actions', () => {
  const member = 'https://socialApi.example/v1/communities/c1/members/0xm'
  const members = 'https://socialApi.example/v2/communities/c1/members?limit=100'

  it.each([
    ['promote', 'PATCH', member, { role: 'moderator' }],
    ['demote', 'PATCH', member, { role: 'member' }],
    ['transfer', 'PATCH', member, { role: 'owner' }],
    ['kick', 'DELETE', member, undefined],
    ['ban', 'POST', `${member}/bans`, undefined]
  ] as const)('%s sends %s and reads the members again', async (action, method, url, body) => {
    serve({ [`GET ${members}`]: { results: [{ memberAddress: '0xm', role: 'moderator' }] } })
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action })
    expect(signed).toHaveBeenCalledWith(url, method, ...(body != null ? [body] : []))
    expect(detail(h.sent)?.members).toEqual([{ address: '0xm', name: '0xm', role: 'moderator', picture: undefined, hasClaimedName: false }])
    expect(h.sent.some((s) => s.kind === 'communityActionFailed')).toBe(false)
  })

  it('reads the community again after a transfer, since the user is no longer its owner', async () => {
    serve({ [`GET https://socialApi.example/v2/communities/c1`]: { id: 'c1', name: 'Builders', description: '', membersCount: 2, role: 'moderator' } })
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action: 'transfer' })
    const card = h.sent.find((s) => s.kind === 'community')
    expect(card?.kind === 'community' ? card.community.role : undefined).toBe('moderator')
  })

  it.each(['kick', 'ban'] as const)('reads the community again after a %s, for its member count', async (action) => {
    serve({
      [`GET https://socialApi.example/v2/communities/c1`]: { id: 'c1', name: 'Builders', description: '', membersCount: 1, role: 'owner' },
      [`GET https://socialApi.example/v2/communities?onlyMemberOf=true&limit=100&offset=0`]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action })
    const card = h.sent.find((s) => s.kind === 'community')
    expect(card?.kind === 'community' ? card.community.membersCount : undefined).toBe(1)
    expect(h.sent.some((s) => s.kind === 'communities')).toBe(true)
  })

  it('does not read the community again after a promote', async () => {
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action: 'promote' })
    expect(h.sent.some((s) => s.kind === 'community')).toBe(false)
  })

  it('says on the card when the service cannot be located', async () => {
    serviceBase.mockRejectedValueOnce(new Error('no socialApi service'))
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action: 'kick' })
    expect(h.sent).toContainEqual({ kind: 'communityActionFailed', id: 'c1', action: 'kick', message: 'no socialApi service' })
  })

  it('says on the card when the action fails, and reads nothing', async () => {
    serve({ [`POST ${member}/bans`]: new Error('HTTP 401') })
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c1', address: '0xm', action: 'ban' })
    expect(h.sent).toContainEqual({ kind: 'communityActionFailed', id: 'c1', action: 'ban', message: 'HTTP 401' })
    expect(detail(h.sent)).toBeUndefined()
  })
})

describe('bridge community browse', () => {
  const v2 = 'https://socialApi.example/v2/communities'
  const memberOf = `GET ${v2}?onlyMemberOf=true&limit=100&offset=0`
  const raw = (id: string, over: Record<string, unknown> = {}): Record<string, unknown> => ({ id, name: id, description: '', membersCount: 1, role: 'none', ...over })
  const browsed = (sent: SceneToPage[]): Extract<SceneToPage, { kind: 'communityBrowse' }> => {
    const m = sent.find((s) => s.kind === 'communityBrowse')
    if (m?.kind !== 'communityBrowse') throw new Error('no browse page sent')
    return m
  }

  it('reads a page of 20, the search on the server', async () => {
    serve({ [`GET ${v2}?search=pix%20el&onlyMemberOf=false&offset=20&limit=20`]: { results: [raw('b1')], total: 21 } })
    const h = fakeCtx()
    await h.req({ kind: 'browseCommunities', search: 'pix el', page: 2 })
    expect(browsed(h.sent)).toMatchObject({ search: 'pix el', page: 2, total: 21, communities: [{ id: 'b1' }] })
    expect(browsed(h.sent).failed).toBeUndefined()
  })

  it("names the first three of the user's friends in each community", async () => {
    serve({ [`GET ${v2}?search=&onlyMemberOf=false&offset=0&limit=20`]: { results: [raw('b1', { friends: ['0xa', '0xb', '0xc', '0xd'] }), raw('b2')], total: 2 } })
    const h = fakeCtx()
    await h.req({ kind: 'browseCommunities', search: '', page: 1 })
    const [b1, b2] = browsed(h.sent).communities
    expect(b1.friends).toEqual(['0xa', '0xb', '0xc'].map((a) => ({ address: a, name: a, hasClaimedName: false })))
    expect(b2.friends).toBeUndefined()
  })

  it('answers a failed read as failed, so the grid stops loading', async () => {
    serve({ [`GET ${v2}?search=&onlyMemberOf=false&offset=0&limit=20`]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'browseCommunities', search: '', page: 1 })
    expect(browsed(h.sent)).toEqual({ kind: 'communityBrowse', search: '', page: 1, communities: [], total: 0, failed: true })
  })

  it('takes an answer without a results list for a failed read', async () => {
    serve({ [`GET ${v2}?search=&onlyMemberOf=false&offset=0&limit=20`]: { total: 3 } })
    const h = fakeCtx()
    await h.req({ kind: 'browseCommunities', search: '', page: 1 })
    expect(browsed(h.sent).failed).toBe(true)
  })

  it('My Communities is every membership, not a browse page', async () => {
    serve({ [memberOf]: { results: [raw('m1', { role: 'member' })], total: 1 } })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunities' })
    expect(h.sent).toEqual([expect.objectContaining({ kind: 'communities', communities: [expect.objectContaining({ id: 'm1' })] })])
    expect(signed.mock.calls.map((c) => c[0] as string).some((u) => u.includes('onlyMemberOf=false'))).toBe(false)
  })

  it('a join sends the community it changed, then My Communities and the channels from one read', async () => {
    serve({
      [`GET ${v2}/c1`]: raw('c1', { role: 'member', membersCount: 2 }),
      [memberOf]: { results: [raw('c1', { role: 'member' })], total: 1 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'joinCommunity', id: 'c1' })
    expect(signed).toHaveBeenCalledWith('https://socialApi.example/v1/communities/c1/members', 'POST')
    expect(h.sent.map((s) => s.kind)).toEqual(['community', 'communities', 'communityChannels'])
    expect(signed.mock.calls.filter((c) => c[0] === memberOf.slice(4))).toHaveLength(1)
  })

  const requests = 'GET https://socialApi.example/v2/members/0xme/requests?type=request_to_join'
  const sentCommunity = (sent: SceneToPage[]): Extract<SceneToPage, { kind: 'community' }>['community'] | undefined => {
    const m = sent.filter((s) => s.kind === 'community').at(-1)
    return m?.kind === 'community' ? m.community : undefined
  }

  it("marks the communities the user asked to join, from the user's own join requests", async () => {
    serve({
      [`GET ${v2}?search=&onlyMemberOf=false&offset=0&limit=20`]: { results: [raw('b1', { privacy: 'private' }), raw('b2', { privacy: 'private' })], total: 2 },
      [requests]: { results: [{ id: 'r1', communityId: 'B1', type: 'request_to_join', status: 'pending' }], total: 1 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'browseCommunities', search: '', page: 1 })
    const [b1, b2] = browsed(h.sent).communities
    expect(b1.pendingRequestId).toBe('r1')
    expect(b2.pendingRequestId).toBeUndefined()
  })

  it("keeps the request POST's id so the card can cancel it, and forgets it on cancel", async () => {
    serve({
      [`POST https://socialApi.example/v1/communities/p1/requests`]: { id: 'r9' },
      [`PATCH https://socialApi.example/v1/communities/p1/requests/r9`]: {},
      [`GET ${v2}/p1`]: raw('p1', { privacy: 'private' }),
      [memberOf]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'requestToJoinCommunity', id: 'p1' })
    // told first, before the community is read again
    expect(h.sent[0]).toEqual({ kind: 'communityJoinRequest', id: 'p1', requestId: 'r9' })
    expect(sentCommunity(h.sent)?.pendingRequestId).toBe('r9')
    await h.req({ kind: 'cancelJoinRequest', id: 'p1', requestId: 'r9' })
    expect(signed).toHaveBeenCalledWith('https://socialApi.example/v1/communities/p1/requests/r9', 'PATCH', { intention: 'cancelled' })
    expect(h.sent).toContainEqual({ kind: 'communityJoinRequest', id: 'p1' })
    expect(sentCommunity(h.sent)?.pendingRequestId).toBeUndefined()
  })

  it("reads the user's join requests when a closed community's card opens", async () => {
    serve({
      [`GET ${v2}/p2`]: raw('p2', { privacy: 'private' }),
      [requests]: { results: [{ id: 'r2', communityId: 'p2' }], total: 1 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunity', id: 'p2' })
    expect(sentCommunity(h.sent)?.pendingRequestId).toBe('r2')
  })

  it('a re-read without the friends list leaves the card its friends', async () => {
    serve({ [`GET ${v2}/c1`]: raw('c1', { role: 'member' }), [memberOf]: { results: [], total: 0 } })
    const h = fakeCtx()
    await h.req({ kind: 'joinCommunity', id: 'c1' })
    expect(sentCommunity(h.sent)).not.toHaveProperty('friends', [])
    expect(sentCommunity(h.sent)?.friends).toBeUndefined()
  })

  it('a failed member-of read after a write sends neither list', async () => {
    serve({ [`GET ${v2}/c1`]: raw('c1', { role: 'member' }), [memberOf]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'joinCommunity', id: 'c1' })
    expect(h.sent.map((s) => s.kind)).toEqual(['community'])
  })
})

describe('bridge community invites and requests', () => {
  const s = 'https://socialApi.example'
  const v2 = `${s}/v2/communities`
  const memberOf = `GET ${v2}?onlyMemberOf=true&limit=100&offset=0`
  const invitesRead = `GET ${s}/v2/members/0xme/requests?type=invite`
  const requestsRead = `GET ${s}/v2/members/0xme/requests?type=request_to_join`
  const raw = (id: string, over: Record<string, unknown> = {}): Record<string, unknown> => ({ id, name: id, description: '', membersCount: 1, role: 'none', ...over })
  const last = <K extends SceneToPage['kind']>(sent: SceneToPage[], kind: K): Extract<SceneToPage, { kind: K }> | undefined =>
    sent.filter((m): m is Extract<SceneToPage, { kind: K }> => m.kind === kind).at(-1)

  it("reads the section: the user's invites, the requests their communities received and their own requests", async () => {
    serve({
      [invitesRead]: { results: [{ id: 'inv1', communityId: 'c3', name: 'VTATV', description: '', membersCount: 4, role: 'none', privacy: 'public' }] },
      [requestsRead]: { results: [{ id: 'rq1', communityId: 'c2', name: 'Chiri', description: '', membersCount: 9, role: 'none', privacy: 'private' }] },
      [memberOf]: { results: [raw('c8', { role: 'owner', privacy: 'private' }), raw('c9', { role: 'moderator', privacy: 'private' }), raw('c1', { role: 'member' })], total: 3 },
      [`GET ${v2}/c8/requests?type=request_to_join&limit=100&offset=0`]: { results: [{ id: 'r-a', memberAddress: '0xa' }, { id: 'r-b', memberAddress: '0xb' }], total: 2 },
      [`GET ${v2}/c9/requests?type=request_to_join&limit=100&offset=0`]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityInvites' })
    const m = last(h.sent, 'communityInvites')
    expect(m?.failed).toEqual([])
    expect(m?.invites).toMatchObject([{ id: 'c3', name: 'VTATV', pendingInviteId: 'inv1' }])
    expect(m?.requestsSent).toMatchObject([{ id: 'c2', pendingRequestId: 'rq1' }])
    // only the communities with requests, and a plain member's community is never read
    expect(m?.requestsReceived).toHaveLength(1)
    expect(m?.requestsReceived[0]).toMatchObject({ community: { id: 'c8' }, total: 2, requests: [{ address: '0xa', requestId: 'r-a' }, { address: '0xb', requestId: 'r-b' }] })
    expect(signed.mock.calls.some((c) => String(c[0]).includes('/c1/requests'))).toBe(false)
  })

  it("counts the sidebar's invites and requests received from each read's total", async () => {
    serve({
      [`${invitesRead}&limit=1`]: { results: [{ id: 'i' }], total: 2 },
      [memberOf]: { results: [raw('c8', { role: 'owner', privacy: 'private' }), raw('c9', { role: 'owner', privacy: 'public' }), raw('c1', { role: 'member', privacy: 'private' })], total: 3 },
      [`GET ${v2}/c8/requests?type=request_to_join&limit=1&offset=0`]: { results: [{ id: 'r' }], total: 5 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityInvitesCount' })
    expect(h.sent).toEqual([{ kind: 'communityInvitesCount', count: 7 }])
    // neither a plain member's community nor a public one (which takes no join requests) is read
    expect(signed.mock.calls.some((c) => /\/(c1|c9)\/requests/.test(String(c[0])))).toBe(false)
  })

  it("keeps the communities whose requests were read when another's read fails", async () => {
    serve({
      [invitesRead]: { results: [] },
      [requestsRead]: { results: [] },
      [memberOf]: { results: [raw('c8', { role: 'owner', privacy: 'private' }), raw('c7', { role: 'moderator', privacy: 'private' })], total: 2 },
      [`GET ${v2}/c8/requests?type=request_to_join&limit=100&offset=0`]: { results: [{ id: 'r-a', memberAddress: '0xa' }], total: 1 },
      [`GET ${v2}/c7/requests?type=request_to_join&limit=100&offset=0`]: new Error('HTTP 403')
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityInvites' })
    expect(last(h.sent, 'communityInvites')).toMatchObject({ failed: [], requestsReceived: [{ community: { id: 'c8' }, total: 1 }] })
  })

  it('answers even when the whole read throws, so the section stops loading', async () => {
    serve({ [invitesRead]: { results: [{ id: 'i1', communityId: 'c3' }] }, [requestsRead]: { results: [] }, [memberOf]: { results: [], total: 0 } })
    // the invited community's picture cannot be located, so building its card throws
    serviceBase.mockImplementation(async (name: string) => {
      if (name === 'assetsCdn') throw new Error('no assets service')
      return `https://${name}.example`
    })
    const h = fakeCtx()
    try {
      await h.req({ kind: 'getCommunityInvites' })
    } finally {
      serviceBase.mockImplementation(async (name: string) => `https://${name}.example`)
    }
    expect(last(h.sent, 'communityInvites')).toMatchObject({ invites: [], failed: ['invites', 'requestsReceived', 'requestsSent'] })
  })

  it('names the lists it could not read', async () => {
    serve({ [invitesRead]: new Error('HTTP 500'), [requestsRead]: { results: [] }, [memberOf]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityInvites' })
    expect(last(h.sent, 'communityInvites')).toMatchObject({ invites: [], requestsReceived: [], requestsSent: [], failed: expect.arrayContaining(['invites', 'requestsReceived']) })
  })

  it("marks a card the user is invited to, read when it opens", async () => {
    serve({ [`GET ${v2}/c3`]: raw('c3'), [invitesRead]: { results: [{ id: 'inv1', communityId: 'c3' }] } })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunity', id: 'c3' })
    expect(last(h.sent, 'community')?.community.pendingInviteId).toBe('inv1')
  })

  it("accepts the user's own invite, then sends the community, the lists and the section again", async () => {
    serve({
      [`PATCH ${s}/v1/communities/c3/requests/inv1`]: {},
      [`GET ${v2}/c3`]: raw('c3', { role: 'member' }),
      [memberOf]: { results: [raw('c3', { role: 'member' })], total: 1 },
      [invitesRead]: { results: [] },
      [requestsRead]: { results: [] }
    })
    const h = fakeCtx()
    await h.req({ kind: 'manageCommunityRequest', id: 'c3', requestId: 'inv1', intention: 'accepted' })
    expect(signed).toHaveBeenCalledWith(`${s}/v1/communities/c3/requests/inv1`, 'PATCH', { intention: 'accepted' })
    expect(h.sent.map((m) => m.kind)).toEqual(['community', 'communities', 'communityChannels', 'communityInvites'])
  })

  it("answers a member's request, then reads the card's requests and invites again", async () => {
    serve({
      [`PATCH ${s}/v1/communities/c8/requests/r-a`]: {},
      [`GET ${v2}/c8/requests?type=request_to_join&limit=100&offset=0`]: { results: [{ id: 'r-b', memberAddress: '0xb' }], total: 1 },
      [`GET ${v2}/c8/requests?type=invite&limit=100&offset=0`]: { results: [], total: 0 },
      [invitesRead]: { results: [] },
      [requestsRead]: { results: [] },
      [memberOf]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'manageCommunityRequest', id: 'c8', requestId: 'r-a', intention: 'rejected', address: '0xa' })
    const sections = h.sent.filter((m) => m.kind === 'communityMemberSection')
    expect(sections).toMatchObject([
      { section: 'requests', total: 1, members: [{ address: '0xb', requestId: 'r-b' }] },
      { section: 'invites', total: 0, members: [] }
    ])
    // a reject changes no member: the card's members are not read
    expect(h.sent.some((m) => m.kind === 'communityDetail')).toBe(false)
  })

  it("accepting a member's request reads the card's members and the community again", async () => {
    serve({
      [`PATCH ${s}/v1/communities/c8/requests/r-a`]: {},
      [`GET ${v2}/c8/requests?type=request_to_join&limit=100&offset=0`]: { results: [], total: 0 },
      [`GET ${v2}/c8/requests?type=invite&limit=100&offset=0`]: { results: [], total: 0 },
      [`GET ${v2}/c8/members?limit=100`]: { results: [{ memberAddress: '0xa', role: 'member' }] },
      [`GET ${v2}/c8`]: raw('c8', { role: 'owner', membersCount: 2 }),
      [invitesRead]: { results: [] },
      [requestsRead]: { results: [] },
      [memberOf]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'manageCommunityRequest', id: 'c8', requestId: 'r-a', intention: 'accepted', address: '0xa' })
    expect(h.sent.map((m) => m.kind)).toEqual(['communityMemberSection', 'communityMemberSection', 'communityDetail', 'community', 'communityInvites'])
    expect(last(h.sent, 'communityDetail')?.members).toMatchObject([{ address: '0xa' }])
    expect(last(h.sent, 'community')?.community.membersCount).toBe(2)
  })

  it('a ban sends the Banned section again', async () => {
    serve({
      [`POST ${s}/v1/communities/c8/members/0xbad/bans`]: {},
      [`GET ${v2}/c8/bans?limit=100&offset=0`]: { results: [{ memberAddress: '0xbad' }], total: 1 },
      [`GET ${v2}/c8`]: raw('c8', { role: 'owner' }),
      [memberOf]: { results: [], total: 0 }
    })
    const h = fakeCtx()
    await h.req({ kind: 'communityMemberAction', id: 'c8', address: '0xbad', action: 'ban' })
    expect(last(h.sent, 'communityMemberSection')).toMatchObject({ section: 'banned', members: [{ address: '0xbad' }] })
  })

  it('says which answer failed, and reads nothing after it', async () => {
    serve({ [`PATCH ${s}/v1/communities/c3/requests/inv1`]: new Error('HTTP 500'), [`PATCH ${s}/v1/communities/c8/requests/r`]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'manageCommunityRequest', id: 'c3', requestId: 'inv1', intention: 'rejected' })
    expect(h.sent).toEqual([expect.objectContaining({ kind: 'communityActionFailed', id: 'c3', action: 'rejectInvite' })])
    await h.req({ kind: 'manageCommunityRequest', id: 'c8', requestId: 'r', intention: 'cancelled', address: '0xa' })
    // a member's row is read again, which releases it
    expect(h.sent.slice(1).map((m) => m.kind)).toEqual(['communityActionFailed', 'communityMemberSection', 'communityMemberSection', 'communityInvites'])
    expect(h.sent[1]).toMatchObject({ kind: 'communityActionFailed', action: 'manageRequest' })
  })

  it("reads the card's banned members, and unbans one", async () => {
    serve({
      [`GET ${v2}/c8/bans?limit=100&offset=0`]: { results: [{ communityId: 'c8', memberAddress: '0xbad' }], total: 1 },
      [`DELETE ${s}/v1/communities/c8/members/0xbad/bans`]: {}
    })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityMemberSection', id: 'c8', section: 'banned' })
    expect(last(h.sent, 'communityMemberSection')).toMatchObject({ section: 'banned', total: 1, members: [{ address: '0xbad', requestId: '' }] })
    await h.req({ kind: 'unbanCommunityMember', id: 'c8', address: '0xbad' })
    expect(signed).toHaveBeenCalledWith(`${s}/v1/communities/c8/members/0xbad/bans`, 'DELETE')
    expect(h.sent.at(-1)).toMatchObject({ kind: 'communityMemberSection', section: 'banned' })
  })

  it('a section that cannot be read says so', async () => {
    serve({ [`GET ${v2}/c8/requests?type=invite&limit=100&offset=0`]: new Error('HTTP 403') })
    const h = fakeCtx()
    await h.req({ kind: 'getCommunityMemberSection', id: 'c8', section: 'invites' })
    expect(last(h.sent, 'communityMemberSection')).toEqual({ kind: 'communityMemberSection', id: 'c8', section: 'invites', members: [], total: 0, failed: true })
  })

  it('lists the communities the user may invite someone to, and invites them', async () => {
    serve({
      [`GET ${s}/v1/members/0xfriend/invites`]: [{ id: 'c8', name: 'Builders' }],
      [`POST ${s}/v1/communities/c8/requests`]: { id: 'new' }
    })
    const h = fakeCtx()
    await h.req({ kind: 'getInvitableCommunities', address: '0xfriend' })
    expect(last(h.sent, 'invitableCommunities')).toEqual({ kind: 'invitableCommunities', address: '0xfriend', communities: [{ id: 'c8', name: 'Builders' }] })
    await h.req({ kind: 'inviteToCommunity', id: 'c8', address: '0xfriend' })
    expect(signed).toHaveBeenCalledWith(`${s}/v1/communities/c8/requests`, 'POST', { targetedAddress: '0xfriend', type: 'invite' })
    expect(last(h.sent, 'communityInviteSent')).toEqual({ kind: 'communityInviteSent', id: 'c8', address: '0xfriend', ok: true })
    // the list is read again after the invite, so the community just used is no longer offered
    expect(h.sent.at(-1)).toMatchObject({ kind: 'invitableCommunities', address: '0xfriend' })
  })

  it('a cancelled join request leaves the Invites & Requests section', async () => {
    serve({
      [`PATCH ${s}/v1/communities/c2/requests/rq1`]: {},
      [`GET ${v2}/c2`]: raw('c2', { privacy: 'private' }),
      [memberOf]: { results: [], total: 0 },
      [invitesRead]: { results: [] },
      [requestsRead]: { results: [] }
    })
    const h = fakeCtx()
    await h.req({ kind: 'cancelJoinRequest', id: 'c2', requestId: 'rq1' })
    expect(h.sent.map((m) => m.kind)).toEqual(['communityJoinRequest', 'community', 'communities', 'communityChannels', 'communityInvites'])
    expect(last(h.sent, 'communityInvites')?.requestsSent).toEqual([])
  })

  it('reports a failed invitable read and a failed invite', async () => {
    serve({ [`GET ${s}/v1/members/0xfriend/invites`]: new Error('HTTP 500'), [`POST ${s}/v1/communities/c8/requests`]: new Error('HTTP 500') })
    const h = fakeCtx()
    await h.req({ kind: 'getInvitableCommunities', address: '0xfriend' })
    expect(last(h.sent, 'invitableCommunities')).toMatchObject({ communities: [], failed: true })
    await h.req({ kind: 'inviteToCommunity', id: 'c8', address: '0xfriend' })
    expect(last(h.sent, 'communityInviteSent')?.ok).toBe(false)
  })
})
