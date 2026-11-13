// What the lobby's widgets show, read while the page is up: the landing place and who is there, the
// recent and recommended places, where each online friend is, and (from the session) the events,
// communities and photos. A failed read keeps what the widget last had.

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

/** Where Jump In goes: the home destination, a world by name or a Genesis parcel. */
export interface LandingDestination {
  world: string | null
  parcel: { x: number; y: number }
}

/** The landing destination from the engine's home scene (a realm with a dot is a world). */
export function landingDestination(home: { realm: string | null; parcel: string } | null): LandingDestination | null {
  if (home == null) return null
  const [x, y] = home.parcel.split(',').map(Number)
  const parcel = Number.isFinite(x) && Number.isFinite(y) ? { x, y } : { x: 0, y: 0 }
  return { world: home.realm != null && home.realm.includes('.') ? home.realm : null, parcel }
}

/** What the landing card says when its place can't be read: the world, Genesis Plaza at 0,0, or the parcel. */
export function offlineTitle(dest: LandingDestination): string {
  if (dest.world != null) return dest.world
  return dest.parcel.x === 0 && dest.parcel.y === 0 ? 'Genesis Plaza' : `${dest.parcel.x},${dest.parcel.y}`
}

/** A read that keeps its last good value through failures; `loaded` once any read has finished. */
function useKept<T>(load: (() => Promise<T>) | null, deps: unknown[]): { value: T | null; loaded: boolean } {
  const [state, setState] = useState<{ value: T | null; loaded: boolean }>({ value: null, loaded: false })
  useEffect(() => {
    if (load == null) return
    let live = true
    load()
      .then((value) => live && setState({ value, loaded: true }))
      .catch(() => live && setState((s) => ({ ...s, loaded: true })))
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
  return state
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
  const place = useKept(() => (dest == null ? Promise.resolve(null) : fetchLandingPlace(dest)), [key])
  const placeId = place.value?.id ?? null
  const live = useKept(placeId == null ? null : () => fetchConnected(placeId), [placeId])
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
  const recent = useKept(fetchRecents, [])
  const recommended = useKept(fetchHighlighted, [])
  return { recent: recent.value, recommended: recommended.value }
}

export interface FriendLocation {
  where: FriendWhere
  /** "Locating…" until known, then the place's title, or "Lobby" when nowhere. */
  title: string
}

/** Where each online friend is, read once per set of online friends; "Locating…" until then. */
export function useFriendLocations(online: Friend[]): Map<string, FriendLocation> {
  const ids = useMemo(() => online.map((f) => f.address.toLowerCase()).sort(), [online])
  const key = ids.join(',')
  const [locations, setLocations] = useState<Map<string, FriendLocation>>(new Map())
  const known = useRef(locations)
  known.current = locations
  useEffect(() => {
    const missing = ids.filter((id) => !known.current.has(id))
    if (missing.length === 0) return
    let live = true
    fetchWhereabouts(missing)
      .then(async (found) => {
        const titled = await Promise.all([...found].map(async ([id, where]) => [id, { where, title: await whereTitle(where) }] as const))
        if (live) setLocations((prev) => new Map([...prev, ...titled]))
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  return locations
}

/** Ask again where one friend is (JOIN re-checks before going). */
export async function refreshWhere(address: string): Promise<FriendWhere> {
  return (await fetchWhereabouts([address])).get(address.toLowerCase()) ?? { kind: 'unknown' }
}
