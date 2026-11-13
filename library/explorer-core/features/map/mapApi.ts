// The full-screen map's server reads and writes.
//
// Requests the services answer per signer (places, events, a place's photos, and every write)
// go through the bridge scene, where the engine signs them as the player. The two reads that
// carry no identity (the points-of-interest list and the connected players) are plain fetches.

import { serviceUrl } from '@hud-host'
import type { MapApiRequest, MapApiResultMessage } from '../../engine/protocol'
import { parcelKey, type Vec2 } from './mapCoords'

type Send = (id: number, request: MapApiRequest) => void
type Pending = { resolve: (data: unknown) => void; reject: (error: Error) => void }

let send: Send | null = null
let nextId = 1
const pending = new Map<number, Pending>()

/** Wire the bridge in (the session does, once it has a driver); null when the bridge goes away. */
export function setMapApiTransport(transport: Send | null): void {
  send = transport
  if (transport == null) {
    for (const p of pending.values()) p.reject(new Error('The map lost its connection'))
    pending.clear()
  }
}

/** Deliver the bridge's answer to the request that is waiting for it. */
export function resolveMapApi(msg: MapApiResultMessage): void {
  const p = pending.get(msg.id)
  if (p == null) return
  pending.delete(msg.id)
  if (msg.ok) p.resolve(msg.data)
  else p.reject(new Error(msg.error ?? 'The request failed'))
}

function request<T>(req: MapApiRequest): Promise<T> {
  if (send == null) return Promise.reject(new Error('The map is not connected'))
  const id = nextId++
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (data: unknown) => void, reject })
    send?.(id, req)
  })
}

export interface MapPlace {
  id: string
  title: string
  description?: string | null
  image?: string | null
  owner?: string | null
  contact_name?: string | null
  creator_address?: string | null
  positions: string[]
  base_position: string
  categories?: string[]
  world_name?: string | null
  likes?: number
  dislikes?: number
  favorites?: number
  like_rate?: number | string | null
  user_count?: number
  user_like?: boolean
  user_dislike?: boolean
  user_favorite?: boolean
}

export const EMPTY_PLACE_ID = 'fake_id'
const EMPTY_PLACE_IMAGE = `${serviceUrl('catalyst')}/content/contents/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y`

/** What the map shows for a parcel no place covers. */
export function emptyPlace(parcel: Vec2): MapPlace {
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

export const isEmptyPlace = (place: MapPlace): boolean => place.id === EMPTY_PLACE_ID
export const isWorldPlace = (place: MapPlace): boolean => place.world_name != null && place.world_name !== ''

const ROADS = ['Tram Line', 'Long Road']
/** Roads span the city, so their place is identified by the parcel the player picked. */
export const isRoad = (title: string): boolean => ROADS.includes(title)

export function likeRate(place: MapPlace): number | null {
  const raw = place.like_rate
  if (raw == null || raw === '') return null
  const value = typeof raw === 'number' ? raw : Number(raw)
  return Number.isFinite(value) ? value : null
}

export async function fetchPlacesAt(parcels: readonly Vec2[]): Promise<MapPlace[]> {
  if (parcels.length === 0) return []
  const res = await request<{ data?: MapPlace[] }>({ op: 'places', positions: parcels.map(parcelKey) })
  return res?.data ?? []
}

export async function fetchPlaceAt(parcel: Vec2): Promise<MapPlace | null> {
  return (await fetchPlacesAt([parcel]))[0] ?? null
}

export type MapSort = 'most_active' | 'like_score' | 'created_at'
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
  places: MapPlace[]
  total: number
}

const queryValue = (value: string): string => encodeURIComponent(value).replace(/%20/g, '+')

export function searchQuery(search: MapSearch): string {
  const params = [
    `search=${queryValue(search.text)}`,
    `offset=${search.page * SEARCH_PAGE_SIZE}`,
    `limit=${SEARCH_PAGE_SIZE}`,
    `order_by=${search.sort}`,
    'order=desc',
    `categories=${search.favorites ? '' : search.category}`,
    'with_realms_detail=true',
    'only_sdk7=true'
  ]
  if (search.favorites) params.push('only_favorites=true')
  return params.join('&')
}

export async function searchPlaces(search: MapSearch): Promise<MapSearchResult> {
  const res = await request<{ data?: MapPlace[]; total?: number }>({ op: 'destinations', query: searchQuery(search) })
  const places = res?.data ?? []
  return { places, total: res?.total ?? places.length }
}

export interface LikeResult {
  likes: number
  dislikes: number
  user_like: boolean
  user_dislike: boolean
}

/** Like (true), dislike (false) or clear (null) the player's vote on a place. */
export async function setPlaceLike(id: string, like: boolean | null): Promise<LikeResult> {
  const res = await request<{ data?: LikeResult }>({ op: 'placeLike', id, like })
  if (res?.data == null) throw new Error('The place did not take the vote')
  return res.data
}

export interface FavoriteResult {
  favorites: number
  user_favorite: boolean
}

export async function setPlaceFavorite(id: string, favorite: boolean): Promise<FavoriteResult> {
  const res = await request<{ data?: FavoriteResult }>({ op: 'placeFavorite', id, favorite })
  if (res?.data == null) throw new Error('The place did not take the favourite')
  return res.data
}

/** Share of likes among the votes, 0..1, as the services compute it; null when nobody voted. */
export function likeRateOf(likes: number, dislikes: number): number | null {
  const total = likes + dislikes
  return total > 0 ? likes / total : null
}

export interface MapEvent {
  id: string
  name: string
  image?: string | null
  description?: string | null
  x: number
  y: number
  coordinates?: [number, number]
  world?: boolean
  server?: string | null
  live?: boolean
  attending?: boolean
  total_attendees?: number
  user_name?: string | null
  start_at: string
  finish_at?: string
  next_start_at?: string
  next_finish_at?: string
  recurrent_dates?: string[]
}

async function events(query: string): Promise<MapEvent[]> {
  const res = await request<{ data?: MapEvent[] }>({ op: 'events', query })
  return Array.isArray(res?.data) ? res.data : []
}

export function fetchLiveEvents(): Promise<MapEvent[]> {
  return events('list=live')
}

export function fetchEventsAt(parcels: readonly Vec2[], onlyLive: boolean): Promise<MapEvent[]> {
  if (parcels.length === 0) return Promise.resolve([])
  const positions = parcels.map((p) => `positions[]=${p.x},${p.y}`).join('&')
  return events(onlyLive ? `${positions}&list=live` : positions)
}

export async function setEventAttending(id: string, attending: boolean): Promise<void> {
  await request<unknown>({ op: 'eventAttend', id, attending })
}

export interface PlacePhoto {
  id: string
  url: string
  thumbnailUrl?: string
  dateTime?: string
}

export interface PlacePhotos {
  images: PlacePhoto[]
  /** How many photos the place has in all. */
  total: number
}

export const PHOTOS_PAGE_SIZE = 100

export async function fetchPlacePhotos(placeId: string, offset: number): Promise<PlacePhotos> {
  const res = await request<{ images?: PlacePhoto[]; maxImages?: number }>({ op: 'placePhotos', id: placeId, offset, limit: PHOTOS_PAGE_SIZE })
  const images = (res?.images ?? []).filter((i) => typeof i.url === 'string')
  return { images, total: res?.maxImages ?? images.length }
}

let fixedPublicReads: { pois: Vec2[]; peers: Peer[] } | null = null

/** Answer the unsigned reads from fixed data instead of the network (the HUD's mock mode). */
export function fixMapPublicReads(data: { pois: Vec2[]; peers: Peer[] } | null): void {
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

export async function fetchPeers(signal?: AbortSignal): Promise<Peer[]> {
  if (fixedPublicReads != null) return fixedPublicReads.peers
  const res = await fetch(`${serviceUrl('archipelagoStats')}/comms/peers`, { signal })
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
