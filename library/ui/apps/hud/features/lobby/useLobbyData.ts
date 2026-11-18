import { useEffect, useMemo, useRef, useState } from 'react'
import {
  fetchConnected,
  fetchHighlighted,
  fetchLandingPlace,
  fetchRecents,
  fetchWhereabouts,
  whereTitle,
  type FriendWhere,
  type LobbyPlace
} from '@explorer-core/features/lobby/lobbyApi'
import type { Friend } from '@explorer-core/engine/protocol'
import { parseParcel } from '@explorer-core/features/map/mapCoords'

export interface LandingDestination {
  world: string | null
  parcel: { x: number; y: number }
}

/** A realm with a dot is a world. */
export function landingDestination(home: { realm: string | null; parcel: string } | null): LandingDestination | null {
  if (home == null) return null
  const parcel = parseParcel(home.parcel) ?? { x: 0, y: 0 }
  return { world: home.realm != null && home.realm.includes('.') ? home.realm : null, parcel }
}

export function offlineTitle(dest: LandingDestination): string {
  if (dest.world != null) return dest.world
  return dest.parcel.x === 0 && dest.parcel.y === 0 ? 'Genesis Plaza' : `${dest.parcel.x},${dest.parcel.y}`
}

/** A read that keeps its last good value through failures; `loaded` once a read has finished.
 *  Another key starts empty, so a value never shows under a key it was not read for. */
function useKept<T>(load: (() => Promise<T>) | null, key: string | null): { value: T | null; loaded: boolean } {
  const [state, setState] = useState<{ key: string | null; value: T | null; loaded: boolean }>({ key, value: null, loaded: false })
  useEffect(() => {
    if (load == null) return
    let live = true
    load()
      .then((value) => live && setState({ key, value, loaded: true }))
      .catch(() => live && setState((s) => (s.key === key ? { ...s, loaded: true } : { key, value: null, loaded: true })))
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  return state.key === key ? state : { value: null, loaded: false }
}

export interface LandingState {
  dest: LandingDestination | null
  place: LobbyPlace | null
  /** The place couldn't be read: the card is a stand-in that only jumps in. */
  offline: boolean
  loading: boolean
  online: number | null
  connected: string[]
}

export function useLanding(home: { realm: string | null; parcel: string } | null): LandingState {
  const dest = landingDestination(home)
  const key = dest == null ? null : `${dest.world ?? ''}|${dest.parcel.x},${dest.parcel.y}`
  const place = useKept(() => (dest == null ? Promise.resolve(null) : fetchLandingPlace(dest)), key)
  const placeId = place.value?.id ?? null
  const live = useKept(placeId == null ? null : () => fetchConnected(placeId), placeId)
  const connected = live.value?.connected_addresses ?? []
  return {
    dest,
    place: place.value,
    offline: place.loaded && place.value == null,
    loading: dest == null || !place.loaded,
    online: live.value ? (live.value.connected_addresses?.length ?? live.value.user_count ?? 0) : null,
    connected
  }
}

export function usePlaces(): { recent: LobbyPlace[] | null; recommended: LobbyPlace[] | null } {
  const recent = useKept(fetchRecents, '')
  const recommended = useKept(fetchHighlighted, '')
  return { recent: recent.value, recommended: recommended.value }
}

export interface FriendLocation {
  where: FriendWhere
  title: string
}

const LOCATION_TTL_MS = 60_000
const LOCATION_CHECK_MS = 15_000

/** Read when a friend comes online, and again once a location is over a minute old. */
export function useFriendLocations(online: Friend[]): Map<string, FriendLocation> {
  const ids = useMemo(() => online.map((f) => f.address.toLowerCase()).sort(), [online])
  const key = ids.join(',')
  const [locations, setLocations] = useState<Map<string, FriendLocation>>(new Map())
  const [tick, setTick] = useState(0)
  useEffect(() => {
    const timer = setInterval(() => setTick((t) => t + 1), LOCATION_CHECK_MS)
    return () => clearInterval(timer)
  }, [])
  const known = useRef(locations)
  known.current = locations
  // set when an answer is applied: a failed read marks nothing fresh
  const readAt = useRef(new Map<string, number>())
  // one read at a time, never dropped by a tick: a read slower than the tick still lands
  const reading = useRef(false)
  const mounted = useRef(true)
  const currentKey = useRef(key)
  currentKey.current = key
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  useEffect(() => {
    if (reading.current) return
    const now = Date.now()
    const missing = ids.filter((id) => !known.current.has(id) || now - (readAt.current.get(id) ?? 0) > LOCATION_TTL_MS)
    if (missing.length === 0) return
    reading.current = true
    fetchWhereabouts(missing)
      .then(async (found) => {
        const titled = await Promise.all([...found].map(async ([id, where]) => [id, { where, title: await whereTitle(where) }] as const))
        if (!mounted.current) return
        const at = Date.now()
        for (const [id] of titled) readAt.current.set(id, at)
        setLocations((prev) => new Map([...prev, ...titled]))
      })
      .catch(() => undefined)
      .finally(() => {
        reading.current = false
        if (mounted.current && currentKey.current !== key) setTick((t) => t + 1)
      })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, tick])
  return locations
}

/** JOIN re-checks before going. */
export async function refreshWhere(address: string): Promise<FriendWhere> {
  return (await fetchWhereabouts([address])).get(address.toLowerCase()) ?? { kind: 'unknown' }
}
