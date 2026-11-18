// The full-screen map's server reads and writes.
//
// Requests the services answer per signer (places, events, a place's photos, and every write)
// go through the bridge scene's places and events domains, where the engine signs them as the
// player. The two reads that carry no identity (the points-of-interest list and the connected
// players) are plain fetches.

import { serviceUrl } from '@hud-host'
import type {
  EventAttendingMessage,
  EventData,
  EventsMessage,
  GetEventsRequest,
  GetPlacePhotosRequest,
  GetPlacesAtRequest,
  PlaceFavoriteMessage,
  PlaceLikeMessage,
  PlacePhoto,
  PlacePhotosMessage,
  PlaceSearchMessage,
  PlaceSearchOrder,
  PlacesAtMessage,
  SceneToPage,
  SearchPlacesRequest,
  SetEventAttendingRequest,
  SetPlaceFavoriteRequest,
  SetPlaceLikeRequest
} from '../../engine/protocol'
import { nextRequestId } from '../../engine/requestIds'
import type { DiscoverPlace } from '../places/placesApi'
import { parcelKey, type Vec2 } from './mapCoords'

export type { PlacePhoto }

export type MapRequest =
  | GetEventsRequest
  | SetEventAttendingRequest
  | GetPlacesAtRequest
  | SearchPlacesRequest
  | SetPlaceLikeRequest
  | SetPlaceFavoriteRequest
  | GetPlacePhotosRequest

type MapReply = EventsMessage | EventAttendingMessage | PlacesAtMessage | PlaceSearchMessage | PlaceLikeMessage | PlaceFavoriteMessage | PlacePhotosMessage

/** How long the map waits for the bridge's answer before it gives up on a request. */
const MAP_REPLY_TIMEOUT_MS = 10_000

type Waiting = { settle: (msg: MapReply) => void; fail: (error: Error) => void }

let send: ((msg: MapRequest) => void) | null = null
const waiting = new Map<string, Waiting>()

// The answer to a reminder change names its event, every other answer the request it answers.
const replyKey = (msg: MapReply): string => (msg.kind === 'eventAttending' ? `eventAttending:${msg.eventId}` : `${msg.kind}:${msg.request}`)

/** Wire the bridge in (the session does, once it has a driver); null when the bridge goes away. */
export function setMapTransport(transport: ((msg: MapRequest) => void) | null): void {
  send = transport
  if (transport == null) {
    for (const w of waiting.values()) w.fail(new Error('The map lost its connection'))
    waiting.clear()
  }
}

/** Hand the map the bridge's answers to its requests; anything else is ignored. */
export function receiveMapReply(msg: SceneToPage): void {
  switch (msg.kind) {
    case 'events':
    case 'eventAttending':
    case 'placesAt':
    case 'placeSearch':
    case 'placeLike':
    case 'placeFavorite':
    case 'placePhotos':
      waiting.get(replyKey(msg))?.settle(msg)
  }
}

function ask<R extends MapReply>(msg: MapRequest, key: string): Promise<R> {
  const transport = send
  if (transport == null) return Promise.reject(new Error('The map is not connected'))
  waiting.get(key)?.fail(new Error('Asked again'))
  return new Promise<R>((resolve, reject) => {
    const done = (): void => {
      clearTimeout(timer)
      if (waiting.get(key) === entry) waiting.delete(key)
    }
    const entry: Waiting = {
      settle: (reply) => {
        done()
        if ((reply as { failed?: boolean }).failed === true) reject(new Error('The request failed'))
        else resolve(reply as R)
      },
      fail: (error) => {
        done()
        reject(error)
      }
    }
    const timer = setTimeout(() => entry.fail(new Error('The bridge did not answer')), MAP_REPLY_TIMEOUT_MS)
    waiting.set(key, entry)
    transport(msg)
  })
}

type WithoutRequest<T> = T extends unknown ? Omit<T, 'request'> : never

/** Send a request that carries an id, and wait for the `answer` that echoes it. */
function askFor<R extends Exclude<MapReply, EventAttendingMessage>>(answer: R['kind'], msg: WithoutRequest<Exclude<MapRequest, SetEventAttendingRequest>>): Promise<R> {
  const request = nextRequestId()
  return ask<R>({ ...msg, request } as MapRequest, `${answer}:${request}`)
}

const EMPTY_PLACE_ID = 'fake_id'
const EMPTY_PLACE_IMAGE = `${serviceUrl('catalyst')}/content/contents/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y`

/** What the map shows for a parcel no place covers. */
export function emptyPlace(parcel: Vec2): DiscoverPlace {
  return {
    id: EMPTY_PLACE_ID,
    title: 'Empty place',
    description: 'No description',
    image: EMPTY_PLACE_IMAGE,
    owner: 'no owner',
    contact_name: '',
    positions: [parcelKey(parcel)],
    base_position: parcelKey(parcel),
    categories: [],
    world_name: '',
    likes: 0,
    dislikes: 0,
    favorites: 0,
    like_rate: 0,
    user_count: 0,
    user_like: false,
    user_dislike: false,
    user_favorite: false
  }
}

export const isEmptyPlace = (place: DiscoverPlace): boolean => place.id === EMPTY_PLACE_ID
export const isWorldPlace = (place: DiscoverPlace): boolean => place.world_name != null && place.world_name !== ''

const ROADS = ['Tram Line', 'Long Road']
/** Roads span the city, so their place is identified by the parcel the player picked. */
export const isRoad = (title: string): boolean => ROADS.includes(title)

export function likeRate(place: DiscoverPlace): number | null {
  const raw = place.like_rate
  if (raw == null || raw === '') return null
  const value = typeof raw === 'number' ? raw : Number(raw)
  return Number.isFinite(value) ? value : null
}

export async function fetchPlacesAt(parcels: readonly Vec2[]): Promise<DiscoverPlace[]> {
  if (parcels.length === 0) return []
  const res = await askFor<PlacesAtMessage>('placesAt', { kind: 'getPlacesAt', positions: parcels.map(parcelKey) })
  return res.places
}

/** The place covering a parcel: Genesis City's, or in `world` that World's scene there (or its first). */
export async function fetchPlaceAt(parcel: Vec2, world?: string): Promise<DiscoverPlace | null> {
  if (world == null) return (await fetchPlacesAt([parcel]))[0] ?? null
  const res = await askFor<PlacesAtMessage>('placesAt', { kind: 'getPlacesAt', positions: [parcelKey(parcel)], world })
  return res.places[0] ?? null
}

export type MapSort = PlaceSearchOrder
export const SEARCH_PAGE_SIZE = 50

export interface MapSearch {
  text: string
  /** Lowercase category name; '' for none. */
  category: string
  favorites: boolean
  sort: MapSort
  page: number
}

export interface MapSearchResult {
  places: DiscoverPlace[]
  total: number
}

export async function searchPlaces(search: MapSearch): Promise<MapSearchResult> {
  const res = await askFor<PlaceSearchMessage>('placeSearch', {
    kind: 'searchPlaces',
    search: search.text,
    category: search.category,
    favorites: search.favorites,
    orderBy: search.sort,
    offset: search.page * SEARCH_PAGE_SIZE,
    limit: SEARCH_PAGE_SIZE
  })
  return { places: res.places, total: res.total }
}

export interface LikeResult {
  likes: number
  dislikes: number
  user_like: boolean
  user_dislike: boolean
}

/** Like (true), dislike (false) or clear (null) the player's vote on a place. */
export async function setPlaceLike(id: string, like: boolean | null): Promise<LikeResult> {
  const { likes, dislikes, user_like, user_dislike } = await askFor<PlaceLikeMessage>('placeLike', { kind: 'setPlaceLike', placeId: id, like })
  return { likes, dislikes, user_like, user_dislike }
}

export interface FavoriteResult {
  favorites: number
  user_favorite: boolean
}

export async function setPlaceFavorite(id: string, favorite: boolean): Promise<FavoriteResult> {
  const { favorites, user_favorite } = await askFor<PlaceFavoriteMessage>('placeFavorite', { kind: 'setPlaceFavorite', placeId: id, favorite })
  return { favorites, user_favorite }
}

/** Share of likes among the votes, 0..1, as the services compute it; null when nobody voted. */
export function likeRateOf(likes: number, dislikes: number): number | null {
  const total = likes + dislikes
  return total > 0 ? likes / total : null
}

async function events(params: Pick<GetEventsRequest, 'list' | 'positions'>): Promise<EventData[]> {
  return (await askFor<EventsMessage>('events', { kind: 'getEvents', ...params })).events
}

export function fetchLiveEvents(): Promise<EventData[]> {
  return events({ list: 'live' })
}

export function fetchEventsAt(parcels: readonly Vec2[], onlyLive: boolean): Promise<EventData[]> {
  if (parcels.length === 0) return Promise.resolve([])
  const positions = parcels.map((p) => ({ x: p.x, y: p.y }))
  return events(onlyLive ? { positions, list: 'live' } : { positions })
}

export async function setEventAttending(id: string, attending: boolean): Promise<void> {
  await ask<EventAttendingMessage>({ kind: 'setEventAttending', eventId: id, attending }, `eventAttending:${id}`)
}

export interface PlacePhotos {
  images: PlacePhoto[]
  /** How many photos the place has in all. */
  total: number
}

export const PHOTOS_PAGE_SIZE = 100

export async function fetchPlacePhotos(placeId: string, offset: number): Promise<PlacePhotos> {
  const { images, total } = await askFor<PlacePhotosMessage>('placePhotos', { kind: 'getPlacePhotos', placeId, offset, limit: PHOTOS_PAGE_SIZE })
  return { images, total }
}

/** `worlds`: the World each wallet (lowercase) is connected to; a wallet not in it is in none. */
let fixedPublicReads: { pois: Vec2[]; peers: Peer[]; worlds?: Record<string, string> } | null = null

/** Answer the unsigned reads from fixed data instead of the network (the HUD's mock mode). */
export function fixMapPublicReads(data: { pois: Vec2[]; peers: Peer[]; worlds?: Record<string, string> } | null): void {
  fixedPublicReads = data
}

export async function fetchPoiParcels(signal?: AbortSignal): Promise<Vec2[]> {
  if (fixedPublicReads != null) return fixedPublicReads.pois
  const res = await fetch(`${serviceUrl('dclLists')}/pois`, { method: 'POST', signal })
  if (!res.ok) {
    await res.body?.cancel()
    throw new Error(`The points of interest list returned ${res.status}`)
  }
  const body = (await res.json()) as { data?: unknown }
  const out: Vec2[] = []
  for (const pointer of Array.isArray(body.data) ? body.data : []) {
    if (typeof pointer !== 'string') continue
    const [xs, ys] = pointer.split(',')
    const x = Number(xs)
    const y = Number(ys)
    if (ys !== undefined && Number.isFinite(x) && Number.isFinite(y)) out.push({ x, y })
  }
  return out
}

export interface Peer {
  address: string
  /** World metres. */
  x: number
  z: number
}

// 100 wallets keep the peers URL near 5 KB, under common proxy limits
const PEERS_PER_READ = 100

/** The connected players, or with `ids` only those wallets' entries, read 100 wallets at a time.
 *  The stats service matches `id` exactly, and its addresses are lowercase, so ids are sent
 *  lowercased. */
export async function fetchPeers(signal?: AbortSignal, ids?: string | readonly string[]): Promise<Peer[]> {
  const wanted = ids == null ? null : (typeof ids === 'string' ? [ids] : ids).map((id) => id.toLowerCase())
  if (fixedPublicReads != null) {
    if (wanted == null) return fixedPublicReads.peers
    const set = new Set(wanted)
    return fixedPublicReads.peers.filter((p) => set.has(p.address.toLowerCase()))
  }
  if (wanted == null) return fetchPeerBatch('', signal)
  if (wanted.length === 0) return []
  const batches: string[][] = []
  for (let i = 0; i < wanted.length; i += PEERS_PER_READ) batches.push(wanted.slice(i, i + PEERS_PER_READ))
  return (await Promise.all(batches.map((batch) => fetchPeerBatch(`?${batch.map((id) => `id=${encodeURIComponent(id)}`).join('&')}`, signal)))).flat()
}

async function fetchPeerBatch(query: string, signal?: AbortSignal): Promise<Peer[]> {
  const res = await fetch(`${serviceUrl('archipelagoStats')}/comms/peers${query}`, { signal })
  if (!res.ok) {
    await res.body?.cancel()
    throw new Error(`The connected players list returned ${res.status}`)
  }
  const body = (await res.json()) as { peers?: unknown }
  const out: Peer[] = []
  for (const peer of Array.isArray(body.peers) ? body.peers : []) {
    const { address, position } = (peer ?? {}) as { address?: unknown; position?: unknown }
    if (typeof address !== 'string' || !Array.isArray(position)) continue
    const x = Math.round(Number(position[0]))
    const z = Math.round(Number(position[2]))
    if (Number.isFinite(x) && Number.isFinite(z)) out.push({ address, x, z })
  }
  return out
}

/** The World a wallet is connected to (the worlds server's `/wallet/{wallet}/connected-world`),
 *  or null when it is in none. */
export async function fetchConnectedWorld(address: string, signal?: AbortSignal): Promise<string | null> {
  if (fixedPublicReads != null) return fixedPublicReads.worlds?.[address.toLowerCase()] ?? null
  const res = await fetch(`${serviceUrl('worldsServer')}/wallet/${encodeURIComponent(address)}/connected-world`, { signal })
  if (!res.ok) {
    await res.body?.cancel()
    if (res.status === 404) return null
    throw new Error(`The connected world read returned ${res.status}`)
  }
  const { world } = (await res.json()) as { world?: unknown }
  return typeof world === 'string' && world !== '' ? world : null
}
