// The player's home: one parcel of Genesis City, or a world. Kept by the engine for the
// signed-in wallet; this is the page's copy of it and the way to change it.

import type { MapPlace } from './mapApi'
import { isWorldPlace } from './mapApi'
import { parseParcel, type Vec2 } from './mapCoords'

export interface Home {
  /** The home parcel, when home is in Genesis City. */
  parcel: Vec2 | null
  /** The home world's name, when home is a world. */
  world: string | null
}

export const NO_HOME: Home = { parcel: null, world: null }

export type HomeChange = { parcel: Vec2 } | { world: string } | null

export interface HomeStore {
  subscribe: (listener: () => void) => () => void
  getSnapshot: () => Home
  /** The engine's word on what home is. */
  receive: (home: Home) => void
  /** Make a parcel or a world home, or (`null`) have no home. */
  set: (change: HomeChange) => void
  /** Wire the engine in; null when it goes away. */
  setTransport: (send: ((change: HomeChange) => void) | null) => void
}

export function createHomeStore(): HomeStore {
  let home = NO_HOME
  let send: ((change: HomeChange) => void) | null = null
  const listeners = new Set<() => void>()
  const commit = (next: Home): void => {
    home = next
    listeners.forEach((l) => l())
  }
  return {
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getSnapshot: () => home,
    receive: commit,
    set: (change) => {
      commit(change == null ? NO_HOME : 'world' in change ? { parcel: null, world: change.world } : { parcel: change.parcel, world: null })
      send?.(change)
    },
    setTransport: (transport) => {
      send = transport
    }
  }
}

/** The home an engine answer describes. A pinned realm other than the default one is a world. */
export function homeOf(scene: { realm: string; parcel: Vec2; pinned: boolean }, defaultRealm: string): Home {
  if (!scene.pinned) return NO_HOME
  return scene.realm === defaultRealm ? { parcel: { x: scene.parcel.x, y: scene.parcel.y }, world: null } : { parcel: null, world: scene.realm }
}

export const homeStore = createHomeStore()

/** Home is one parcel, so any parcel of a place makes the whole place home. */
export function isHomePlace(home: Home, place: MapPlace): boolean {
  if (isWorldPlace(place)) return home.world === place.world_name
  const at = home.parcel
  if (home.world != null || at == null) return false
  if (place.positions.length > 0) return place.positions.some((p) => p === `${at.x},${at.y}`)
  const base = parseParcel(place.base_position)
  return base != null && base.x === at.x && base.y === at.y
}
