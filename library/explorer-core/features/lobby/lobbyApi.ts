// The lobby's data: the curated destinations, the landing place for the home destination, and the
// places visited most recently.

import { serviceUrl } from '@hud-host'
import type { DiscoverPlace } from '../places/placesApi'
import { getPref, PREF, setPref } from '../../lib/prefs'

const PLACES_API = `${serviceUrl('places')}/api`
const RECENTS_MAX = 20
const RECENTS_SHOWN = 3
const CACHE_TTL = 60_000

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

async function placesData(url: string, init?: RequestInit): Promise<DiscoverPlace[]> {
  const res = await fetch(url, init)
  if (!res.ok) throw new Error(`places API failed: ${res.status}`)
  const json = (await res.json()) as { ok?: boolean; data?: DiscoverPlace[] }
  if (json.ok === false || !Array.isArray(json.data)) throw new Error('places API returned not-ok')
  return json.data
}

export function fetchHighlighted(): Promise<DiscoverPlace[]> {
  return cached('highlighted', () => placesData(`${PLACES_API}/destinations?with_realms_detail=true&only_highlighted=true`))
}

/** The place covering a parcel (null when the parcel is empty). */
export function fetchPlaceAt(x: number, y: number): Promise<DiscoverPlace | null> {
  return cached(`place:${x},${y}`, async () => (await placesData(`${PLACES_API}/places?with_realms_detail=true&positions=${x},${y}`))[0] ?? null)
}

export function fetchWorld(name: string): Promise<DiscoverPlace | null> {
  return cached(`world:${name}`, async () => (await placesData(`${PLACES_API}/worlds?names=${encodeURIComponent(name)}`))[0] ?? null)
}

/** The landing place for the home destination: a World by name, else the place at its parcel. */
export function fetchHomePlace(home: { realm: string | null; parcel: string }): Promise<DiscoverPlace | null> {
  if (home.realm != null && home.realm.includes('.')) return fetchWorld(home.realm)
  const [x, y] = home.parcel.split(',').map(Number)
  return Number.isFinite(x) && Number.isFinite(y) ? fetchPlaceAt(x, y) : Promise.resolve(null)
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

/** The recently visited places, in visit order. */
export async function fetchRecents(): Promise<DiscoverPlace[]> {
  const ids = readRecents()
  if (ids.length === 0) return []
  const data = await placesData(`${PLACES_API}/destinations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(ids)
  })
  const byId = new Map(data.map((p) => [p.id, p]))
  return ids.flatMap((id) => byId.get(id) ?? []).slice(0, RECENTS_SHOWN)
}
