// Communities: browse list, joining, per-community detail (members/posts/places), and the chat
// channels: the communities the user is a member of, who is online in one, and presence.
//   from: social-api communities service via BevyApi.kernelFetch (signed GETs / POST join)
//       + the engine's profile cache (profile.ts) for the names and faces those reads no
//         longer carry — the v2 endpoints answer with addresses only
//       + the engine's community connectivity stream (the social service's
//         SubscribeToCommunityMemberConnectivityUpdates) for presence; the local user's own
//         updates add and remove channels.
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import { getJson, serviceBase, signed, signedForm } from '../http'
import { fetchIdentities } from './profile'
import type { ProfileIdentity } from './profile'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { Community, CommunityAction, CommunityChannel, CommunityEvent, CommunityMember, CommunityPhoto, CommunityPlace, CommunityPost } from '../../../src/engine/protocol'

// Community events live on the (public) events-api, filtered by community_id. Community photos
// live on the camera-reel service (mirrors /api/users/{a}/images). Community thumbnails are NOT
// in the upstream list response — Unity builds them from the id against the assets CDN
// (DecentralandUrl.CommunityThumbnail); a dcl-one deployment sends `thumbnailUrl`, which wins.
// Some 404 (no thumbnail set) → the React <img> falls back to the initial.

type CommunityRaw = {
  id: string
  name: string
  description: string
  membersCount: number
  role: string
  ownerAddress?: string
  privacy?: string
  pendingActionType?: string
  pendingInviteOrRequestId?: string
  thumbnailUrl?: string
  voiceChatStatus?: { isActive?: boolean; participantCount?: number; moderatorCount?: number }
}

// The v2 rows are FLAT and address-only: no name, face, or claimed-name flag on any of them.
type MemberRaw = { memberAddress?: string; role?: string; friendshipStatus?: number }
type PostRaw = { id: string; authorAddress?: string; content?: string; createdAt?: string; likesCount?: number }
type PlaceRaw = { id: string; title?: string; name?: string; image?: string; base_position?: string; positions?: string[]; like_rate?: number; likeRate?: number }
type EventRaw = { id: string; name?: string; image?: string; next_start_at?: string; start_at?: string }
type PhotoRaw = { id: string; url?: string; thumbnailUrl?: string }

// FriendshipStatus enum: request_sent=0 … friend=3 … none=7. Hide "Add Friend" once friends.
const FRIEND = 3

// Reads take the v2 endpoints, which answer with addresses instead of embedding a profile the
// service had to resolve per row. Writes (and /places, which carries no profile) have no v2.
async function base(version: 'v1' | 'v2'): Promise<string> {
  return `${await serviceBase('socialApi')}/${version}/communities`
}

async function thumbnail(c: CommunityRaw): Promise<string> {
  return c.thumbnailUrl || `${await serviceBase('assetsCdn')}/social/communities/${encodeURIComponent(c.id)}/raw-thumbnail.png`
}

async function communities(raw: CommunityRaw[]): Promise<Community[]> {
  const identity = await fetchIdentities(raw.map((c) => c.ownerAddress ?? ''))
  const thumbs = await Promise.all(raw.map(thumbnail))
  return raw.map((c, i) => ({
    id: c.id,
    name: c.name,
    description: c.description,
    thumbnail: thumbs[i],
    membersCount: c.membersCount,
    role: c.role,
    ownerName: identity(c.ownerAddress ?? '').name,
    privacy: c.privacy,
    pendingRequestId: c.pendingActionType === 'request_to_join' ? c.pendingInviteOrRequestId : undefined,
    voiceChatStatus: c.voiceChatStatus == null ? undefined : {
      isActive: c.voiceChatStatus.isActive === true,
      participantCount: c.voiceChatStatus.participantCount ?? 0,
      moderatorCount: c.voiceChatStatus.moderatorCount ?? 0
    }
  }))
}

async function list(): Promise<Community[]> {
  const res = await signed<{ results?: CommunityRaw[] }>(`${await base('v2')}?limit=50`)
  return await communities(res?.results ?? [])
}

async function rawOne(id: string): Promise<CommunityRaw | undefined> {
  const res = await signed<{ data?: CommunityRaw } | CommunityRaw>(`${await base('v2')}/${encodeURIComponent(id)}`)
  const raw = res != null && 'data' in res && res.data != null ? res.data : (res as CommunityRaw | undefined)
  return raw?.id != null ? raw : undefined
}

// The names of the communities seen, for the chat bubbles' community line: a message names its
// community by id only.
const names = new Map<string, string>()
export function communityName(id: string): string | undefined {
  return names.get(id.toLowerCase())
}

function remember(c: { id: string; name: string }): void {
  names.set(c.id.toLowerCase(), c.name)
}

async function channel(c: CommunityRaw): Promise<CommunityChannel> {
  remember(c)
  return { id: c.id, name: c.name, thumbnail: await thumbnail(c), role: c.role }
}

// The communities the user is a member of, 100 a call. The page count is capped so a server that
// ignores `offset` cannot keep this going. An answer without a results list is a failed read, not
// an empty page: the caller drops a community missing from the list, so an empty one from a
// malformed answer would delete its history.
const MEMBER_OF_PAGE = 100
const MEMBER_OF_PAGES = 10
async function memberOf(): Promise<CommunityChannel[]> {
  const b = await base('v2')
  const raw: CommunityRaw[] = []
  for (let page = 0; page < MEMBER_OF_PAGES; page++) {
    const offset = page * MEMBER_OF_PAGE
    const res = await signed<{ results?: CommunityRaw[]; total?: number }>(`${b}?onlyMemberOf=true&limit=${MEMBER_OF_PAGE}&offset=${offset}`)
    const rows = res?.results
    if (!Array.isArray(rows)) throw new Error('member-of answer has no results list')
    raw.push(...rows)
    if (rows.length < MEMBER_OF_PAGE || (res?.total != null && raw.length >= res.total)) break
  }
  return await Promise.all(raw.map(channel))
}

async function one(id: string): Promise<CommunityChannel | undefined> {
  const raw = await rawOne(id)
  return raw != null ? await channel(raw) : undefined
}

// Who is online in a community now: the members list filtered server-side, with identities.
async function onlineMembers(id: string): Promise<CommunityMember[]> {
  const b = await base('v2')
  const res = await signed<{ results?: MemberRaw[] }>(`${b}/${encodeURIComponent(id)}/members?offset=0&limit=1000&onlyOnline=true`)
  const rows = res?.results ?? []
  const identity = await fetchIdentities(rows.map((m) => m.memberAddress ?? ''))
  return rows.map((m) => mapMember(m, identity))
}

function mapMember(m: MemberRaw, identity: (address: string) => ProfileIdentity): CommunityMember {
  const address = m.memberAddress ?? ''
  const { name, picture, hasClaimedName } = identity(address)
  return {
    address,
    name,
    role: m.role ?? 'member',
    picture,
    hasClaimedName,
    isFriend: m.friendshipStatus === FRIEND
  }
}

async function detail(id: string): Promise<{ members: CommunityMember[]; posts: CommunityPost[]; places: CommunityPlace[]; events: CommunityEvent[]; photos: CommunityPhoto[] }> {
  const b = await base('v2')
  const placesBase = await base('v1')
  const eventsBase = `${await serviceBase('events')}/api/events`
  const reelBase = `${await serviceBase('cameraReel')}/api/communities`
  // The id reaches us from the service (a uuid), but it lands in a URL — encode it so it
  // can only ever be one path segment / one query value.
  const cid = encodeURIComponent(id)
  const [membersRes, postsRes, placesRes, eventsRes, photosRes] = await Promise.all([
    signed<{ results?: MemberRaw[] }>(`${b}/${cid}/members?limit=100`).catch(() => undefined),
    signed<{ posts?: PostRaw[] }>(`${b}/${cid}/posts?limit=20`).catch(() => undefined),
    signed<{ results?: PlaceRaw[] }>(`${placesBase}/${cid}/places?limit=20`).catch(() => undefined),
    getJson<{ data?: { events?: EventRaw[] } }>(`${eventsBase}?community_id=${cid}&list=upcoming`).catch(() => undefined),
    signed<{ images?: PhotoRaw[] }>(`${reelBase}/${cid}/images?limit=30`).catch(() => undefined)
  ])
  const memberRows = membersRes?.results ?? []
  const postRows = postsRes?.posts ?? []
  // One batch for every address both tabs reference — the members list alone would otherwise
  // be up to 100 separate profile fetches.
  const identity = await fetchIdentities([
    ...memberRows.map((m) => m.memberAddress ?? ''),
    ...postRows.map((p) => p.authorAddress ?? '')
  ])
  const members = memberRows.map((m) => mapMember(m, identity))
  const posts: CommunityPost[] = postRows.map((p) => {
    const address = p.authorAddress ?? ''
    const { name, picture } = identity(address)
    return {
      id: p.id,
      author: name,
      authorAddress: address,
      authorPicture: picture,
      text: p.content ?? '',
      timestamp: p.createdAt != null ? Date.parse(p.createdAt) : 0,
      likes: p.likesCount ?? 0
    }
  })
  const places: CommunityPlace[] = (placesRes?.results ?? []).map((pl) => ({
    id: pl.id,
    title: pl.title ?? pl.name ?? '',
    thumbnail: pl.image,
    positions: pl.base_position ?? pl.positions?.[0],
    likeRate: pl.like_rate ?? pl.likeRate
  }))
  const events: CommunityEvent[] = (eventsRes?.data?.events ?? []).map((e) => ({
    id: e.id,
    name: e.name ?? '',
    thumbnail: e.image,
    startsAt: Date.parse(e.next_start_at ?? e.start_at ?? '') || 0
  }))
  const photos: CommunityPhoto[] = (photosRes?.images ?? []).filter((ph) => ph.url != null).map((ph) => ({
    id: ph.id,
    url: ph.url ?? '',
    thumbnail: ph.thumbnailUrl ?? ph.url
  }))
  return { members, posts, places, events, photos }
}

export function registerCommunities(ctx: Ctx): void {
  ctx.on('getCommunities', async () => {
    const communities = await list()
    communities.forEach(remember)
    ctx.send({ kind: 'communities', communities })
  })
  ctx.on('getCommunity', async (msg) => {
    const raw = await rawOne(msg.id).catch((e: unknown) => {
      console.error('[communities] community fetch failed', e)
      ctx.send({ kind: 'communityActionFailed', id: msg.id, action: 'fetch', message: e instanceof Error ? e.message : String(e) })
      return undefined
    })
    if (raw == null) return
    remember(raw)
    const [community] = await communities([raw])
    if (community != null) ctx.send({ kind: 'community', community })
  })
  // The member-of list is authoritative when it arrives: an empty one closes every community
  // chat. So a failed read sends nothing, and the page keeps the list it has (the next join, leave
  // or login reads it again), rather than emptying it for a passing error.
  const sendChannels = async (): Promise<void> => {
    const communities = await memberOf().catch((e: unknown) => {
      console.error('[communities] member-of list failed', e)
      return undefined
    })
    if (communities != null) ctx.send({ kind: 'communityChannels', communities })
  }
  ctx.on('getCommunityChannels', sendChannels)
  ctx.on('setCommunityChannels', (msg) => {
    BevyApi.social.setCommunityChannels(msg.ids)
  })
  // The community whose members the page last asked for: the one shown, whose list its presence
  // updates keep current.
  let shown: string | null = null
  ctx.on('getCommunityOnlineMembers', async (msg) => {
    shown = msg.id
    const members = await onlineMembers(msg.id).catch((e: unknown) => {
      console.error('[communities] online members failed', e)
      return []
    })
    ctx.send({ kind: 'communityOnlineMembers', id: msg.id, members })
  })
  // Presence: the stream carries every community the user is in. The local user's own ONLINE for
  // a community means they joined (or were added), OFFLINE that they left or were removed: the
  // page adds or drops the tab. Another member's update only matters for the community shown
  // (the page keeps no other list), so the rest are dropped before their identity is resolved.
  // The stream never carries `away`.
  relay('communityPresence', async () => await BevyApi.social.getCommunityConnectivityStream(), (u) => {
    const me = getPlayer()?.userId?.toLowerCase()
    const address = u.address.toLowerCase()
    const online = u.status === 'online'
    if (me != null && address === me) {
      if (online) {
        void one(u.communityId)
          .then((community) => {
            if (community != null) ctx.send({ kind: 'communityChannelAdded', community })
          })
          .catch((e: unknown) => console.error('[communities] community fetch failed', e))
      } else {
        ctx.send({ kind: 'communityChannelRemoved', id: u.communityId })
      }
      return
    }
    if (u.communityId !== shown) return
    // the update names the member only; the page keeps the role it listed for them, if any
    void fetchIdentities([address])
      .then((identity) => {
        ctx.send({ kind: 'communityPresence', communityId: u.communityId, member: mapMember({ memberAddress: address }, identity), online })
      })
      .catch((e: unknown) => console.error('[communities] presence identity failed', e))
  })
  ctx.on('createCommunity', async (msg) => {
    // Text-only multipart (no thumbnail — see signedForm). Matches Unity's create payload.
    await signedForm(await base('v1'), 'POST', {
      name: msg.name,
      description: msg.description,
      privacy: msg.privacy,
      visibility: msg.discoverable ? 'all' : 'unlisted'
    }).catch((e: unknown) => {
      console.error('[communities] create failed', e)
    })
    ctx.send({ kind: 'communities', communities: await list() })
    await sendChannels()
  })
  // Every write reports a rejection to the page (the modal shows it) and then re-reads the list,
  // so the buttons always reflect the server.
  const write = async (id: string, action: CommunityAction, run: () => Promise<unknown>): Promise<void> => {
    await run().catch((e: unknown) => {
      console.error(`[communities] ${action} failed`, e)
      ctx.send({ kind: 'communityActionFailed', id, action, message: e instanceof Error ? e.message : String(e) })
    })
    const listed = await list()
    ctx.send({ kind: 'communities', communities: listed })
    // a card for a community past the list's first page shows the community fetched by id, so
    // that copy is refreshed too (its role and buttons follow the write)
    if (!listed.some((c) => c.id === id)) {
      const raw = await rawOne(id).catch((e: unknown) => {
        console.error('[communities] community fetch failed', e)
        return undefined
      })
      const [community] = raw != null ? await communities([raw]) : []
      if (community != null) ctx.send({ kind: 'community', community })
    }
    // joining or leaving changes the chat channels too
    await sendChannels()
  }
  ctx.on('joinCommunity', async (msg) => {
    await write(msg.id, 'join', async () => await signed(`${await base('v1')}/${encodeURIComponent(msg.id)}/members`, 'POST'))
  })
  ctx.on('requestToJoinCommunity', async (msg) => {
    await write(msg.id, 'requestToJoin', async () => {
      const url = `${await base('v1')}/${encodeURIComponent(msg.id)}/requests`
      return await signed(url, 'POST', { targetedAddress: getPlayer()?.userId ?? '', type: 'request_to_join' })
    })
  })
  ctx.on('cancelJoinRequest', async (msg) => {
    await write(msg.id, 'cancelJoinRequest', async () => {
      const url = `${await base('v1')}/${encodeURIComponent(msg.id)}/requests/${encodeURIComponent(msg.requestId)}`
      return await signed(url, 'PATCH', { intention: 'cancelled' })
    })
  })
  ctx.on('leaveCommunity', async (msg) => {
    await write(msg.id, 'leave', async () => {
      const me = getPlayer()?.userId
      if (me == null || me === '') throw new Error('Not signed in')
      return await signed(`${await base('v1')}/${encodeURIComponent(msg.id)}/members/${encodeURIComponent(me)}`, 'DELETE')
    })
  })
  ctx.on('getCommunityDetail', async (msg) => {
    const { members, posts, places, events, photos } = await detail(msg.id).catch(() => ({ members: [], posts: [], places: [], events: [], photos: [] }))
    ctx.send({ kind: 'communityDetail', id: msg.id, members, posts, places, events, photos })
  })
}
