// What the marker layers draw: live events, points of interest and the other players. Each is
// read the first time its layer is shown; live events are then read again every five minutes.

import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { fetchLiveEvents, fetchPeers, fetchPlacesAt, fetchPoiParcels, type Peer } from '@explorer-core/features/map/mapApi'
import { userMarkerPosition, type Vec2 } from '@explorer-core/features/map/mapCoords'
import type { MapLayer } from '@explorer-core/features/map/mapLayersStore'
import { liveEventMarkers, poiMarkers, type EventMarker, type PlaceMarker } from '@explorer-core/features/map/mapMarkers'
import { nearbyPlayers, travelCount } from '@explorer-core/features/map/scenePins'

const LIVE_EVENTS_POLL_MS = 5 * 60 * 1000

export interface MapLayerData {
  liveEvents: EventMarker[]
  pois: PlaceMarker[]
  /** The connected players, as map positions. */
  people: Vec2[]
}

/**
 * @param shown the map is open
 * @param self  the local player's address, left out of the people layer
 */
export function useMapLayerData(shown: boolean, layers: ReadonlySet<MapLayer>, self: string | undefined): MapLayerData {
  const [liveEvents, setLiveEvents] = useState<EventMarker[]>([])
  const [pois, setPois] = useState<PlaceMarker[]>([])
  const [peers, setPeers] = useState<Peer[]>([])
  const nearby = useSyncExternalStore(nearbyPlayers.subscribe, nearbyPlayers.getSnapshot)
  const travels = useSyncExternalStore(travelCount.subscribe, travelCount.getSnapshot)
  const liveStarted = useRef(false)
  const poisStarted = useRef(false)

  const liveOn = shown && layers.has('liveEvents')
  useEffect(() => {
    if (!liveOn || liveStarted.current) return
    liveStarted.current = true
    const read = (): void => {
      fetchLiveEvents()
        .then((events) => setLiveEvents(liveEventMarkers(events)))
        .catch(() => setLiveEvents([]))
    }
    read()
    // the poll outlives the map being closed, so the markers are current the next time it opens
    setInterval(read, LIVE_EVENTS_POLL_MS)
  }, [liveOn])

  const poisOn = shown && layers.has('pois')
  useEffect(() => {
    if (!poisOn || poisStarted.current) return
    poisStarted.current = true
    fetchPoiParcels()
      .catch(() => [])
      .then((parcels) => fetchPlacesAt(parcels))
      .then((places) => setPois(poiMarkers(places)))
      .catch(() => setPois([]))
  }, [poisOn])

  // The players the engine holds are followed as they move; everyone else connected is read
  // once when the layer comes on and again after each teleport.
  const peopleOn = shown && layers.has('people')
  useEffect(() => {
    if (!peopleOn) {
      setPeers([])
      return
    }
    nearbyPlayers.watch(true)
    return () => nearbyPlayers.watch(false)
  }, [peopleOn])
  useEffect(() => {
    if (!peopleOn) return
    const abort = new AbortController()
    fetchPeers(abort.signal)
      .then(setPeers)
      .catch(() => undefined)
    return () => abort.abort()
  }, [peopleOn, travels])

  const people = useMemo(() => {
    const me = self?.toLowerCase()
    const held = new Set(nearby.map((p) => p.address.toLowerCase()))
    const out = nearby.map((p) => userMarkerPosition(p.x, p.z))
    for (const peer of peers) {
      const address = peer.address.toLowerCase()
      if (address !== me && !held.has(address)) out.push(userMarkerPosition(peer.x, peer.z))
    }
    return out
  }, [nearby, peers, self])

  return { liveEvents, pois, people }
}
