// What each marker layer of the full-screen map draws for the data it is given.

import type { MapEvent, MapPlace } from './mapApi'
import { isWorldPlace } from './mapApi'
import { parcelCenter, parcelKey, parseParcel, type Vec2 } from './mapCoords'
import { parcelsCenter } from './parcelsCenter'

const MAX_TITLE_LENGTH = 29
const EMPTY_PARCEL_NAME = 'Empty parcel'

export const markerTitle = (title: string): string => (title.length > MAX_TITLE_LENGTH ? title.slice(0, MAX_TITLE_LENGTH) : title)

export interface PlaceMarker {
  id: string
  /** The parcel the marker is keyed and clustered by. */
  key: Vec2
  position: Vec2
  title: string
  place: MapPlace
}

export interface EventMarker {
  id: string
  key: Vec2
  position: Vec2
  title: string
  event: MapEvent
}

function placeParcels(place: MapPlace): Vec2[] {
  const out: Vec2[] = []
  for (const p of place.positions ?? []) {
    const parcel = parseParcel(p)
    if (parcel != null) out.push(parcel)
  }
  return out
}

/** The parcel a place's marker stands on, or null for a place without parcels. */
export function placeMarkerParcel(place: MapPlace): Vec2 | null {
  const parcels = placeParcels(place)
  return parcels.length > 0 ? parcelsCenter(parcels) : null
}

/** One marker per live event that is not in a world; where two share a parcel the first stands. */
export function liveEventMarkers(events: readonly MapEvent[]): EventMarker[] {
  const seen = new Set<string>()
  const out: EventMarker[] = []
  for (const event of events) {
    if (event.world === true) continue
    const key = { x: event.x, y: event.y }
    const id = parcelKey(key)
    if (seen.has(id)) continue
    seen.add(id)
    out.push({ id, key, position: parcelCenter(key), title: markerTitle(event.name), event })
  }
  return out
}

/** Points of interest, keyed and drawn on each place's centre parcel. */
export function poiMarkers(places: readonly MapPlace[]): PlaceMarker[] {
  const seen = new Set<string>()
  const out: PlaceMarker[] = []
  for (const place of places) {
    const center = placeMarkerParcel(place)
    if (center == null) continue
    const id = parcelKey(center)
    if (seen.has(id) || place.title === EMPTY_PARCEL_NAME) continue
    seen.add(id)
    out.push({ id, key: center, position: parcelCenter(center), title: markerTitle(place.title), place })
  }
  return out
}

/** Search and category results: keyed by base parcel, drawn on the centre parcel; worlds have no place on the map. */
export function resultMarkers(places: readonly MapPlace[]): PlaceMarker[] {
  const seen = new Set<string>()
  const out: PlaceMarker[] = []
  for (const place of places) {
    if (isWorldPlace(place)) continue
    const key = parseParcel(place.base_position)
    const center = placeMarkerParcel(place)
    if (key == null || center == null) continue
    const id = parcelKey(key)
    if (seen.has(id) || place.title === EMPTY_PARCEL_NAME) continue
    seen.add(id)
    out.push({ id, key, position: parcelCenter(center), title: markerTitle(place.title), place })
  }
  return out
}
