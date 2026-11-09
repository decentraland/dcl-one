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
import { dataUrlBytes, getJson, HttpError, serviceBase, signed, signedForm } from '../http'
import { fetchIdentities } from './profile'
import type { ProfileIdentity } from './profile'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { Community, CommunityAction, CommunityChannel, CommunityEvent, CommunityMember, CommunityPhoto, CommunityPlace, CommunityPlaceOption, CommunityPlaceOptionsMessage, CommunityPost, CommunitySaveResultMessage } from '../../../src/engine/protocol'

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
  visibility?: string
  pendingActionType?: string
  pendingInviteOrRequestId?: string
  thumbnailUrl?: string
}

// The v2 rows are FLAT and address-only: no name, face, or claimed-name flag on any of them.
type MemberRaw = { memberAddress?: string; role?: string }
type PostRaw = { id: string; authorAddress?: string; content?: string; createdAt?: string; likesCount?: number }
type PlaceRaw = {
  id: string
  title?: string
  name?: string
  image?: string
  base_position?: string
  positions?: string[]
  world_name?: string | null
  like_rate?: number
  likeRate?: number
  owner?: string | null
}
type EventRaw = { id: string; name?: string; image?: string; next_start_at?: string; start_at?: string }
type PhotoRaw = { id: string; url?: string; thumbnailUrl?: string }

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
    visibility: c.visibility,
    pendingRequestId: c.pendingActionType === 'request_to_join' ? c.pendingInviteOrRequestId : undefined
  }))
}

// The page's list: the first 50 of the browse order, then the user's own communities it left out,
// so My Communities holds every one the user is a member of, however far down the browse order it
// is. A failed member-of read leaves the browse list as it is.
async function list(): Promise<Community[]> {
  const [browse, member] = await Promise.all([
    signed<{ results?: CommunityRaw[] }>(`${await base('v2')}?limit=50`),
    memberOfRaw().catch((): CommunityRaw[] => [])
  ])
  const raw = browse?.results ?? []
  const listed = new Set(raw.map((c) => c.id))
  return await communities([...raw, ...member.filter((c) => !listed.has(c.id))])
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
  return await Promise.all((await memberOfRaw()).map(channel))
}

async function memberOfRaw(): Promise<CommunityRaw[]> {
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
  return raw
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
    hasClaimedName
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
  // As unity-explorer's card: the community's place ids, then each place from the places service
  // (title, picture, owner). The social service's own row is the fallback when that read fails.
  const placeRows = placesRes?.results ?? []
  const destinations =
    placeRows.length > 0
      ? await serviceBase('places')
          .then((b) => signed<PlaceRaw[]>(`${b}/api/destinations`, 'POST', placeRows.map((pl) => pl.id)))
          .catch((e: unknown) => {
            console.error('[communities] places details failed', e)
            return undefined
          })
      : undefined
  const byId = new Map((destinations ?? []).map((d) => [d.id, d]))
  const me = (getPlayer()?.userId ?? '').toLowerCase()
  const places: CommunityPlace[] = placeRows.map((row) => {
    const pl = byId.get(row.id) ?? row
    return {
      id: pl.id,
      title: pl.title ?? pl.name ?? '',
      thumbnail: pl.image,
      positions: (pl.world_name ?? '') !== '' ? (pl.world_name ?? undefined) : (pl.base_position ?? pl.positions?.[0]),
      likeRate: pl.like_rate ?? pl.likeRate,
      isMine: me !== '' && (pl.owner ?? '').toLowerCase() === me
    }
  })
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

// The form's places picker, as unity-explorer's CommunityCreationEditionController: the places
// the user owns or operates (one destinations read by owner), and, when editing, the community's
// linked places (their ids from the social service, their details from the places service by id).
// Each is labelled "title (world name)" or "title (x,y)" and carries its owner's name.
type DestinationRaw = { id: string; title?: string; world_name?: string | null; base_position?: string; owner?: string | null }

const GET_PLACES_ERROR = 'There was an error getting places. Please try again.'
const GET_OWNERS_NAMES_ERROR = 'There was an error getting owners names. Please try again.'
const GET_COMMUNITY_PLACES_ERROR = 'There was an error getting the community places. Please try again.'

/** The owners' names by address (lowercased), or null when the profile read found none, which
 *  unity-explorer reports as an error and shows the places without names. */
async function ownerNames(rows: DestinationRaw[]): Promise<Map<string, string> | null> {
  const owners = [...new Set(rows.map((r) => (r.owner ?? '').toLowerCase()).filter((a) => a !== ''))]
  if (owners.length === 0) return new Map()
  const found = await BevyApi.getCompactProfiles(owners).catch(() => [])
  if (found.length === 0) return null
  return new Map(found.map((p) => [p.pointer.toLowerCase(), p.name]))
}

function placeOptions(rows: DestinationRaw[], names: Map<string, string> | null): CommunityPlaceOption[] {
  return rows.map((r) => {
    const isWorld = (r.world_name ?? '') !== ''
    return {
      id: r.id,
      isWorld,
      label: `${r.title ?? ''} (${isWorld ? r.world_name : (r.base_position ?? '')})`,
      ownerName: names?.get((r.owner ?? '').toLowerCase()) ?? ''
    }
  })
}

/** The places the user owns or operates, one read as unity-explorer's. Never without an owner: the
 *  service reads an empty one as no filter at all, and every place would be offered as the user's.
 *  (`signed` answers with the envelope's `data`: here the list itself.) */
async function ownedPlaces(destinations: string, me: string): Promise<DestinationRaw[]> {
  if (me === '') throw new Error('no signed-in user')
  return (await signed<DestinationRaw[]>(`${destinations}?owner=${encodeURIComponent(me)}`)) ?? []
}

// The services' page cap, and how many pages of a community's places are read before giving up.
const PAGE = 100
const LINKED_PAGES = 10

/** Every place id the community links, a page at a time. Throws when the list does not end within
 *  the pages read, so the form never takes a partial list for the whole one. */
async function linkedPlaceIds(communityId: string): Promise<string[]> {
  const url = `${await base('v1')}/${encodeURIComponent(communityId)}/places`
  const ids: string[] = []
  for (let page = 0; page < LINKED_PAGES; page++) {
    const res = await signed<{ results?: { id: string }[]; total?: number }>(`${url}?limit=${PAGE}&offset=${page * PAGE}`)
    const rows = res?.results ?? []
    ids.push(...rows.map((r) => r.id))
    if (rows.length < PAGE || (res?.total != null && ids.length >= res.total)) return ids
  }
  throw new Error(`more than ${PAGE * LINKED_PAGES} linked places`)
}

/** The places' details by id, a page at a time. A place the service does not return (a disabled
 *  one, say) is left out. */
async function destinationsByIds(destinations: string, ids: string[]): Promise<DestinationRaw[]> {
  const rows: DestinationRaw[] = []
  for (let i = 0; i < ids.length; i += PAGE) rows.push(...((await signed<DestinationRaw[]>(destinations, 'POST', ids.slice(i, i + PAGE))) ?? []))
  return rows
}

async function placeOptionsFor(communityId: string | undefined): Promise<CommunityPlaceOptionsMessage> {
  const errors: string[] = []
  const destinations = `${await serviceBase('places')}/api/destinations`
  let owned: CommunityPlaceOption[] = []
  try {
    const rows = await ownedPlaces(destinations, getPlayer()?.userId ?? '')
    const names = await ownerNames(rows)
    if (names == null) errors.push(GET_OWNERS_NAMES_ERROR)
    owned = placeOptions(rows, names)
  } catch (e) {
    console.error('[communities] owned places failed', e)
    errors.push(GET_PLACES_ERROR)
  }
  let linked: CommunityPlaceOption[] = []
  let hiddenLinkedIds: string[] = []
  let linkedFailed = false
  if (communityId != null) {
    try {
      const ids = await linkedPlaceIds(communityId)
      const rows = ids.length > 0 ? await destinationsByIds(destinations, ids) : []
      const shown = new Set(rows.map((r) => r.id))
      // linked, but not returned by the places service: the form keeps them on every save
      hiddenLinkedIds = ids.filter((id) => !shown.has(id))
      if (rows.length > 0) {
        const names = await ownerNames(rows)
        if (names == null) errors.push(GET_OWNERS_NAMES_ERROR)
        linked = placeOptions(rows, names)
      }
    } catch (e) {
      console.error('[communities] community places failed', e)
      linkedFailed = true
      errors.push(GET_COMMUNITY_PLACES_ERROR)
    }
  }
  return { kind: 'communityPlaceOptions', communityId, owned, linked, hiddenLinkedIds, linkedFailed, errors }
}

/** A refused create or update as the form shows it. As in unity-explorer, every HTTP 400 is the
 *  content check's answer: its per-field issues (possibly none), or `unavailable` when the check
 *  could not run. Anything else is a plain error. */
function saveFailure(e: unknown): Pick<CommunitySaveResultMessage, 'moderation' | 'error'> {
  if (e instanceof HttpError && e.status === 400) {
    let body: { data?: { issues?: Partial<Record<'name' | 'description' | 'image', unknown>> }; communityContentValidationUnavailable?: unknown } = {}
    try {
      body = JSON.parse(e.body) as typeof body
    } catch {
      // not JSON: a refusal with no findings
    }
    const tags = (v: unknown): string[] => (Array.isArray(v) ? v.filter((t): t is string => typeof t === 'string') : [])
    const issues = body.data?.issues
    return {
      moderation: {
        issues: { name: tags(issues?.name), description: tags(issues?.description), image: tags(issues?.image) },
        unavailable: body.communityContentValidationUnavailable === true
      }
    }
  }
  return { error: e instanceof Error ? e.message : String(e) }
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
  // After a create, update or delete: the list and the chat channels, each on its own, so a failed
  // list read never skips the channels.
  const refreshLists = async (): Promise<void> => {
    await list().then(
      (communities) => ctx.send({ kind: 'communities', communities }),
      (e: unknown) => console.error('[communities] list failed', e)
    )
    await sendChannels()
  }
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
    // unity-explorer's create payload: text fields, and the picture as a `thumbnail.png` file part
    // (always labelled image/png; the service reads the real type from the bytes).
    const files =
      msg.thumbnail != null
        ? [{ name: 'thumbnail', filename: 'thumbnail.png', contentType: 'image/png', bytes: dataUrlBytes(msg.thumbnail) }]
        : []
    const fields = {
      name: msg.name,
      description: msg.description,
      privacy: msg.privacy,
      visibility: msg.discoverable ? 'all' : 'unlisted',
      // as unity-explorer, always sent on create, `[]` when no place is linked
      placeIds: JSON.stringify(msg.placeIds)
    }
    // the service url is resolved inside the chain, so a failure there is a reported error too
    const result = await base('v1')
      .then((b) => signedForm<{ id?: string }>(b, 'POST', fields, files))
      .then((created): CommunitySaveResultMessage => ({ kind: 'communitySaveResult', mode: 'create', id: created?.id ?? '' }))
      .catch((e: unknown): CommunitySaveResultMessage => {
        console.error('[communities] create failed', e)
        return { kind: 'communitySaveResult', mode: 'create', ...saveFailure(e) }
      })
    ctx.send(result)
    await refreshLists()
  })
  ctx.on('updateCommunity', async (msg) => {
    // unity-explorer's update: a multipart PUT with only the fields that changed, and the picture
    // as a `thumbnail.png` file part when a new one was chosen.
    const fields: Record<string, string> = {}
    if (msg.name != null) fields.name = msg.name
    if (msg.description != null) fields.description = msg.description
    if (msg.privacy != null) fields.privacy = msg.privacy
    if (msg.discoverable != null) fields.visibility = msg.discoverable ? 'all' : 'unlisted'
    // the whole new set; the service keeps the current one when the field is absent
    if (msg.placeIds != null) fields.placeIds = JSON.stringify(msg.placeIds)
    const files =
      msg.thumbnail != null
        ? [{ name: 'thumbnail', filename: 'thumbnail.png', contentType: 'image/png', bytes: dataUrlBytes(msg.thumbnail) }]
        : []
    const id = encodeURIComponent(msg.id)
    const result = await base('v1')
      .then((b) => signedForm(`${b}/${id}`, 'PUT', fields, files))
      .then((): CommunitySaveResultMessage => ({ kind: 'communitySaveResult', mode: 'update', id: msg.id }))
      .catch((e: unknown): CommunitySaveResultMessage => {
        console.error('[communities] update failed', e)
        return { kind: 'communitySaveResult', mode: 'update', ...saveFailure(e) }
      })
    ctx.send(result)
    if (result.id == null) return
    // the card, the browse list and My Communities all show the saved values
    const raw = await rawOne(msg.id).catch(() => undefined)
    if (raw != null) {
      const [community] = await communities([raw])
      if (community != null) ctx.send({ kind: 'community', community })
    }
    // the card stays open behind the form: its Places tab shows the places just saved
    if (msg.placeIds != null) {
      const fresh = await detail(msg.id).catch((e: unknown) => {
        console.error('[communities] detail after update failed', e)
        return undefined
      })
      if (fresh != null) ctx.send({ kind: 'communityDetail', id: msg.id, ...fresh })
    }
    await refreshLists()
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
  ctx.on('deleteCommunity', async (msg) => {
    // unity-explorer's delete: DELETE /v1/communities/{id}; on success the card closes. Not through
    // `write`, which re-reads the community it acted on, and this one is gone.
    const deleted = await base('v1')
      .then((b) => signed(`${b}/${encodeURIComponent(msg.id)}`, 'DELETE'))
      .then(
      () => true,
      (e: unknown) => {
        console.error('[communities] delete failed', e)
        ctx.send({ kind: 'communityActionFailed', id: msg.id, action: 'delete', message: e instanceof Error ? e.message : String(e) })
        return false
      }
    )
    if (deleted) ctx.send({ kind: 'communityDeleted', id: msg.id })
    await refreshLists()
  })
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
  // The card's delete, as unity-explorer's: DELETE /v1/communities/{id}/places/{placeId}; then the
  // card's places are read again. A failure is shown on the card.
  ctx.on('removeCommunityPlace', async (msg) => {
    const removed = await base('v1')
      .then((b) => signed(`${b}/${encodeURIComponent(msg.id)}/places/${encodeURIComponent(msg.placeId)}`, 'DELETE'))
      .then(
        () => true,
        (e: unknown) => {
          console.error('[communities] remove place failed', e)
          ctx.send({ kind: 'communityActionFailed', id: msg.id, action: 'removePlace', message: e instanceof Error ? e.message : String(e) })
          return false
        }
      )
    if (!removed) return
    const fresh = await detail(msg.id).catch(() => undefined)
    if (fresh != null) ctx.send({ kind: 'communityDetail', id: msg.id, ...fresh })
  })
  // The card's member menu, as unity-explorer's MembersListController: promote and demote set the
  // member's role, a transfer sets it to owner (the service makes the old owner a moderator), a
  // kick removes the member and a ban bans them. A failure is shown on the card; on success the
  // card's members are read again, and after a transfer the community too, since the user's own
  // role changed.
  ctx.on('communityMemberAction', async (msg) => {
    const member = `${await base('v1')}/${encodeURIComponent(msg.id)}/members/${encodeURIComponent(msg.address)}`
    const run = async (): Promise<unknown> => {
      switch (msg.action) {
        case 'promote':
          return await signed(member, 'PATCH', { role: 'moderator' })
        case 'demote':
          return await signed(member, 'PATCH', { role: 'member' })
        case 'transfer':
          return await signed(member, 'PATCH', { role: 'owner' })
        case 'kick':
          return await signed(member, 'DELETE')
        case 'ban':
          return await signed(`${member}/bans`, 'POST')
      }
    }
    const done = await run().then(
      () => true,
      (e: unknown) => {
        console.error(`[communities] ${msg.action} failed`, e)
        ctx.send({ kind: 'communityActionFailed', id: msg.id, action: msg.action, message: e instanceof Error ? e.message : String(e) })
        return false
      }
    )
    if (!done) return
    const fresh = await detail(msg.id).catch(() => undefined)
    if (fresh != null) ctx.send({ kind: 'communityDetail', id: msg.id, ...fresh })
    if (msg.action !== 'transfer') return
    const raw = await rawOne(msg.id).catch(() => undefined)
    const [community] = raw != null ? await communities([raw]) : []
    if (community != null) ctx.send({ kind: 'community', community })
    await refreshLists()
  })
  ctx.on('getCommunityPlaceOptions', async (msg) => {
    const options = await placeOptionsFor(msg.communityId).catch((e: unknown): CommunityPlaceOptionsMessage => {
      // the places service could not even be located: nothing could be read
      console.error('[communities] place options failed', e)
      const errors = [GET_PLACES_ERROR, ...(msg.communityId != null ? [GET_COMMUNITY_PLACES_ERROR] : [])]
      return { kind: 'communityPlaceOptions', communityId: msg.communityId, owned: [], linked: [], hiddenLinkedIds: [], linkedFailed: msg.communityId != null, errors }
    })
    ctx.send(options)
  })
  ctx.on('getCommunityDetail', async (msg) => {
    const { members, posts, places, events, photos } = await detail(msg.id).catch(() => ({ members: [], posts: [], places: [], events: [], photos: [] }))
    ctx.send({ kind: 'communityDetail', id: msg.id, members, posts, places, events, photos })
  })
}
