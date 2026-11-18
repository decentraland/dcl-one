// What the marker layers draw, from the one read both maps share (explorer-core's mapLayerData).
// Live events and points of interest are held from the first time the map shows them until it
// goes; the players only while it shows them.

import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import { userMarkerPosition, type Vec2 } from '@explorer-core/features/map/mapCoords'
import { mapLayerData, type MarkerLayer } from '@explorer-core/features/map/mapLayerData'
import type { MapLayer } from '@explorer-core/features/map/mapLayersStore'
import type { EventMarker, PlaceMarker } from '@explorer-core/features/map/mapMarkers'
import { nearbyPlayers } from '@explorer-core/features/map/mapPeople'
import { mapPins, type PinMarker } from '@explorer-core/features/map/mapPins'

export interface MapLayerData {
  liveEvents: EventMarker[]
  pois: PlaceMarker[]
  /** The connected players, as map positions. */
  people: Vec2[]
  /** The engine sends them whether or not a map is shown. */
  pins: readonly PinMarker[]
}

const NO_PINS: readonly PinMarker[] = []

/** Holds `layer` while `on`. */
function useLayer(layer: MarkerLayer, on: boolean): void {
  useEffect(() => (on ? mapLayerData.want(layer) : undefined), [layer, on])
}

/** Holds `layer` from the first time `on` until the map goes. */
function useLayerFromFirstShow(layer: MarkerLayer, on: boolean): void {
  const release = useRef<(() => void) | null>(null)
  useEffect(() => {
    if (on && release.current == null) release.current = mapLayerData.want(layer)
  }, [layer, on])
  useEffect(
    () => () => {
      release.current?.()
      release.current = null
    },
    []
  )
}

/**
 * @param shown the map is shown
 * @param self  the local player's address, left out of the people layer
 */
export function useMapLayerData(shown: boolean, layers: ReadonlySet<MapLayer>, self: string | undefined): MapLayerData {
  const data = useSyncExternalStore(mapLayerData.subscribe, mapLayerData.getSnapshot)
  const nearby = useSyncExternalStore(nearbyPlayers.subscribe, nearbyPlayers.getSnapshot)
  const allPins = useSyncExternalStore(mapPins.subscribe, mapPins.getSnapshot)
  const peopleOn = shown && layers.has('people')
  useLayerFromFirstShow('liveEvents', shown && layers.has('liveEvents'))
  useLayerFromFirstShow('pois', shown && layers.has('pois'))
  useLayer('people', peopleOn)

  const people = useMemo(() => {
    if (!peopleOn) return []
    const me = self?.toLowerCase()
    const held = new Set(nearby.map((p) => p.address.toLowerCase()))
    const out = nearby.map((p) => userMarkerPosition(p.x, p.z))
    for (const peer of data.peers) {
      const address = peer.address.toLowerCase()
      if (address !== me && !held.has(address)) out.push(userMarkerPosition(peer.x, peer.z))
    }
    return out
  }, [peopleOn, nearby, data.peers, self])

  const pins = shown && layers.has('pins') ? allPins : NO_PINS

  return { liveEvents: data.liveEvents, pois: data.pois, people, pins }
}
