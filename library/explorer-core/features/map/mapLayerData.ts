// What the marker layers draw, read once for every map that shows them (the full map and the
// minimap): live events, points of interest and the connected players. A map asks for a layer
// and holds it, and the reads follow the holders:
//   live events  read at the first hold, then every five minutes until no map holds them;
//   points of interest  read once at the first hold;
//   players  read while any map holds them, and again after each teleport; the engine's nearby
//            players are followed for as long.
// A map holds live events and points of interest from the first time it shows them until it
// goes, and players only while it shows them.

import { fetchLiveEvents, fetchPeers, fetchPlacesAt, fetchPoiParcels, type Peer } from './mapApi'
import { liveEventMarkers, poiMarkers, type EventMarker, type PlaceMarker } from './mapMarkers'
import { nearbyPlayers, travelCount } from './mapPeople'

export const LIVE_EVENTS_POLL_MS = 5 * 60 * 1000

export type MarkerLayer = 'liveEvents' | 'pois' | 'people'

export interface MapLayerSnapshot {
  liveEvents: EventMarker[]
  pois: PlaceMarker[]
  /** The connected players the catalyst lists; the engine's nearby players are in `nearbyPlayers`. */
  peers: Peer[]
}

export interface MapLayerDataStore {
  subscribe: (listener: () => void) => () => void
  getSnapshot: () => MapLayerSnapshot
  /** A map holds `layer`; returns the release. */
  want: (layer: MarkerLayer) => () => void
}

export function createMapLayerDataStore(): MapLayerDataStore {
  let snapshot: MapLayerSnapshot = { liveEvents: [], pois: [], peers: [] }
  const listeners = new Set<() => void>()
  const commit = (next: Partial<MapLayerSnapshot>): void => {
    snapshot = { ...snapshot, ...next }
    listeners.forEach((l) => l())
  }

  let liveHolders = 0
  let livePoll: ReturnType<typeof setInterval> | null = null
  let liveRun = 0
  const holdLive = (): (() => void) => {
    if (liveHolders++ === 0) {
      const run = ++liveRun
      const read = (): void => {
        fetchLiveEvents()
          .then((events) => run === liveRun && commit({ liveEvents: liveEventMarkers(events) }))
          .catch(() => run === liveRun && commit({ liveEvents: [] }))
      }
      read()
      livePoll = setInterval(read, LIVE_EVENTS_POLL_MS)
    }
    return () => {
      if (--liveHolders > 0) return
      liveRun++
      if (livePoll != null) clearInterval(livePoll)
      livePoll = null
      commit({ liveEvents: [] })
    }
  }

  let poiHolders = 0
  let poiRun = 0
  const holdPois = (): (() => void) => {
    if (poiHolders++ === 0) {
      const run = ++poiRun
      fetchPoiParcels()
        .catch(() => [])
        .then((parcels) => fetchPlacesAt(parcels))
        .then((places) => run === poiRun && commit({ pois: poiMarkers(places) }))
        .catch(() => run === poiRun && commit({ pois: [] }))
    }
    return () => {
      if (--poiHolders > 0) return
      poiRun++
      commit({ pois: [] })
    }
  }

  let peopleViewers = 0
  let peersRead: AbortController | null = null
  let stopTravels: (() => void) | null = null
  const readPeers = (): void => {
    peersRead?.abort()
    const abort = new AbortController()
    peersRead = abort
    fetchPeers(abort.signal)
      .then((peers) => {
        if (!abort.signal.aborted) commit({ peers })
      })
      .catch(() => undefined)
  }
  const showPeople = (): void => {
    if (peopleViewers++ > 0) return
    nearbyPlayers.watch(true)
    readPeers()
    let travels = travelCount.getSnapshot()
    stopTravels = travelCount.subscribe(() => {
      const now = travelCount.getSnapshot()
      if (now === travels) return
      travels = now
      readPeers()
    })
  }
  const hidePeople = (): void => {
    if (--peopleViewers > 0) return
    stopTravels?.()
    stopTravels = null
    peersRead?.abort()
    peersRead = null
    nearbyPlayers.watch(false)
    commit({ peers: [] })
  }

  return {
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getSnapshot: () => snapshot,
    want: (layer) => {
      let released = false
      const release = layer === 'liveEvents' ? holdLive() : layer === 'pois' ? holdPois() : (showPeople(), hidePeople)
      return () => {
        if (released) return
        released = true
        release()
      }
    }
  }
}

export const mapLayerData = createMapLayerDataStore()
