// The route the player is following on the map: one destination parcel, shown until they
// arrive, stop it, or jump in to it.

import type { MapPlace } from './mapApi'
import { parcelCenter, parseParcel, type Vec2 } from './mapCoords'

export interface Destination {
  parcel: Vec2
  position: Vec2
  /** What the banner names it. */
  title: string
}

const EMPTY_PARCEL_NAME = 'Empty parcel'
const ARRIVAL_TOLERANCE_SQUARED = 50
/** The player has to move this far, in squared distance from the origin, for the route to be checked again. */
const MOVE_TOLERANCE = 300

export interface NavigationStore {
  subscribe: (listener: () => void) => () => void
  getSnapshot: () => Destination | null
  /** Route to a place: its only parcel, or its base parcel. */
  start: (place: MapPlace | null) => void
  stop: () => void
  /** The player's marker moved; true when that brought them to the destination. */
  playerMoved: (position: Vec2) => boolean
}

export function destinationParcel(place: MapPlace | null): Vec2 {
  return (place == null ? null : place.positions.length === 1 ? parseParcel(place.positions[0]) : parseParcel(place.base_position)) ?? { x: 0, y: 0 }
}

export function createNavigationStore(): NavigationStore {
  let destination: Destination | null = null
  let lastSqrMagnitude = 0
  const listeners = new Set<() => void>()
  const commit = (next: Destination | null): void => {
    destination = next
    listeners.forEach((l) => l())
  }
  return {
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getSnapshot: () => destination,
    start: (place) => {
      const parcel = destinationParcel(place)
      commit({ parcel, position: parcelCenter(parcel), title: `${place != null ? place.title : EMPTY_PARCEL_NAME} (${parcel.x}, ${parcel.y})` })
    },
    stop: () => {
      if (destination != null) commit(null)
    },
    playerMoved: (position) => {
      const sqrMagnitude = position.x * position.x + position.y * position.y
      if (lastSqrMagnitude !== 0 && Math.abs(sqrMagnitude - lastSqrMagnitude) <= MOVE_TOLERANCE) return false
      lastSqrMagnitude = sqrMagnitude
      if (destination == null) return false
      const dx = position.x - destination.position.x
      const dy = position.y - destination.position.y
      if (dx * dx + dy * dy > ARRIVAL_TOLERANCE_SQUARED) return false
      commit(null)
      return true
    }
  }
}

export const navigation = createNavigationStore()
