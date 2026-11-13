// The lobby's server reads: the landing place and the people there, the recommended and recently
// visited places with their connected users, where online friends are, and the most crowded scene.
// Signed reads (events, communities, the gallery) come through the bridge instead.

import { serviceUrl } from '@hud-host'
import { placePlayers, type DiscoverPlace } from '../places/placesApi'
import { getPref, PREF, setPref } from '../../lib/prefs'

const PLACES_API = `${serviceUrl('places')}/api`
const RECENTS_MAX = 20
const CACHE_TTL = 60_000

/** A place with the wallets connected to it now (with_connected_users). */
export type LobbyPlace = DiscoverPlace & { connected_addresses?: string[] | null }

// Reopening the lobby (or walking between scenes) shouldn't refetch what it just had.
const cache = new Map<string, { at: number; promise: Promise<unknown> }>()
function cached<T>(key: string, load: () => Promise<T>): Promise<T> {
  const hit = cache.get(key)
  if (hit && Date.now() - hit.at < CACHE_TTL) return hit.promise as Promise<T>
  const promise = load()
  cache.set(key, { at: Date.now(), promise })
  promise.catch(() => {
    if (cache.get(key)?.promise === promise) cache.delete(key)
  })
  return promise
}

async function placesData(url: string, init?: RequestInit): Promise<LobbyPlace[]> {
  const res = await fetch(url, init)
  if (!res.ok) throw new Error(`places API failed: ${res.status}`)
  const json = (await res.json()) as { ok?: boolean; data?: LobbyPlace[] }
  if (json.ok === false || !Array.isArray(json.data)) throw new Error('places API returned not-ok')
  return json.data
}

/** People at the place now: its connected wallets, else its user count. */
export function placeOnline(p: LobbyPlace): number {
  return p.connected_addresses?.length ?? placePlayers(p)
}

/** The curated destinations, with who is there. */
export function fetchHighlighted(): Promise<LobbyPlace[]> {
  return cached('highlighted', () => placesData(`${PLACES_API}/destinations?with_realms_detail=true&with_connected_users=true&only_highlighted=true`))
}

/** The place covering a parcel (null when the parcel is empty). */
export function fetchPlaceAt(x: number, y: number): Promise<LobbyPlace | null> {
  return cached(`place:${x},${y}`, async () => (await placesData(`${PLACES_API}/places?with_realms_detail=true&positions=${x},${y}`))[0] ?? null)
}

export function fetchWorld(name: string): Promise<LobbyPlace | null> {
  return cached(`world:${name}`, async () => (await placesData(`${PLACES_API}/worlds?names=${encodeURIComponent(name)}`))[0] ?? null)
}

/** Destinations by id with who is there now, in the order asked; unknown ids are left out. */
export async function fetchDestinationsById(ids: string[]): Promise<LobbyPlace[]> {
  if (ids.length === 0) return []
  const data = await placesData(`${PLACES_API}/destinations?with_connected_users=true`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(ids)
  })
  const byId = new Map(data.map((p) => [p.id, p]))
  return ids.flatMap((id) => byId.get(id) ?? [])
}

/** The landing place: a World by name, else the place at the parcel. */
export function fetchLandingPlace(dest: { world: string | null; parcel: { x: number; y: number } }): Promise<LobbyPlace | null> {
  return dest.world != null ? fetchWorld(dest.world) : fetchPlaceAt(dest.parcel.x, dest.parcel.y)
}

/** Who is at a place now (fresh, never cached). */
export async function fetchConnected(placeId: string): Promise<LobbyPlace | null> {
  return (await fetchDestinationsById([placeId]))[0] ?? null
}

function readRecents(): string[] {
  try {
    const raw = JSON.parse(getPref(PREF.lobbyRecents) ?? '[]') as unknown
    return Array.isArray(raw) ? raw.filter((id): id is string => typeof id === 'string') : []
  } catch {
    return []
  }
}

function rememberPlace(id: string): void {
  const next = [id, ...readRecents().filter((r) => r !== id)].slice(0, RECENTS_MAX)
  setPref(PREF.lobbyRecents, JSON.stringify(next))
}

/** Record the place the player is standing in: Genesis City by parcel, a World by its name. */
export async function recordVisit(visit: { realm: string; genesis: boolean; parcel: { x: number; y: number } }): Promise<void> {
  const place = visit.genesis ? await fetchPlaceAt(visit.parcel.x, visit.parcel.y) : await fetchWorld(visit.realm)
  if (place != null) rememberPlace(place.id)
}

/** The recently visited places, in visit order, with who is there. */
export function fetchRecents(): Promise<LobbyPlace[]> {
  return fetchDestinationsById(readRecents())
}

/** Where an online friend is: a Genesis parcel, a world, or nowhere the services know. */
export type FriendWhere = { kind: 'genesis'; parcel: { x: number; y: number } } | { kind: 'world'; world: string; parcel: { x: number; y: number } | null } | { kind: 'unknown' }

type Peer = { id?: string; address?: string; parcel?: [number, number] }

/** Where each wallet is: the comms peers first, then each one missing asked for the world it is in. */
export async function fetchWhereabouts(wallets: string[]): Promise<Map<string, FriendWhere>> {
  const found = new Map<string, FriendWhere>()
  if (wallets.length === 0) return found
  const query = wallets.map((w) => `id=${encodeURIComponent(w)}`).join('&')
  const res = await fetch(`${serviceUrl('archipelagoStats')}/comms/peers?${query}`)
  if (!res.ok) throw new Error(`peers: ${res.status}`)
  const body = (await res.json()) as { peers?: Peer[] }
  for (const peer of body.peers ?? []) {
    const id = (peer.id ?? peer.address ?? '').toLowerCase()
    if (id && Array.isArray(peer.parcel)) found.set(id, { kind: 'genesis', parcel: { x: peer.parcel[0], y: peer.parcel[1] } })
  }
  await Promise.all(
    wallets
      .filter((w) => !found.has(w.toLowerCase()))
      .map(async (w) => {
        const r = await fetch(`${serviceUrl('worldsServer')}/wallet/${encodeURIComponent(w)}/connected-world`).catch(() => null)
        if (r == null || !r.ok) return
        const data = (await r.json().catch(() => null)) as { world?: string } | null
        if (data?.world) found.set(w.toLowerCase(), { kind: 'world', world: data.world, parcel: null })
      })
  )
  for (const w of wallets) if (!found.has(w.toLowerCase())) found.set(w.toLowerCase(), { kind: 'unknown' })
  return found
}

/** The name a friend's location shows: the place's title, else the world's name or the parcel. */
export async function whereTitle(where: FriendWhere): Promise<string> {
  if (where.kind === 'unknown') return 'Lobby'
  if (where.kind === 'world') return (await fetchWorld(where.world).catch(() => null))?.title || where.world
  return (await fetchPlaceAt(where.parcel.x, where.parcel.y).catch(() => null))?.title || `${where.parcel.x},${where.parcel.y}`
}

/** The base parcel of the most crowded scene, where "crowd" goes. */
export async function fetchCrowd(): Promise<{ x: number; y: number }> {
  const res = await fetch(`${serviceUrl('realmProvider')}/hot-scenes`)
  if (!res.ok) throw new Error(`hot-scenes: ${res.status}`)
  const scenes = (await res.json()) as Array<{ baseCoords?: [number, number] }>
  const top = scenes[0]?.baseCoords
  if (!Array.isArray(top)) throw new Error('hot-scenes: none')
  return { x: top[0], y: top[1] }
}

/** Whether a world's realm answers, as a realm change probes it before going. */
export async function worldReachable(world: string): Promise<boolean> {
  const r = await fetch(`${serviceUrl('worldsServer')}/world/${encodeURIComponent(world)}/about`, { method: 'HEAD' }).catch(() => null)
  return r != null && r.ok
}
