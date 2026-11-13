// What the engine tells the map about the world around the player: the pins scenes put on the
// map, and the other players it holds.

import type { MapPinInfo } from '../../engine/generated'
import type { MapPlayer } from '../../engine/protocol'
import { parcelCenter, type Vec2 } from './mapCoords'

export interface ScenePin {
  id: string
  parcel: Vec2
  position: Vec2
  title: string
  description: string
  /** The scene's own icon; null draws the default pin. */
  icon: string | null
}

function store<T>(initial: T): { subscribe: (listener: () => void) => () => void; getSnapshot: () => T; set: (value: T) => void } {
  let value = initial
  const listeners = new Set<() => void>()
  return {
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getSnapshot: () => value,
    set: (next) => {
      value = next
      listeners.forEach((l) => l())
    }
  }
}

const pins = store<ScenePin[]>([])
const players = store<MapPlayer[]>([])
const travels = store(0)
let watch: ((on: boolean) => void) | null = null

export const scenePins = {
  subscribe: pins.subscribe,
  getSnapshot: pins.getSnapshot,
  receive(list: readonly MapPinInfo[]): void {
    pins.set(
      list.map((pin) => {
        const parcel = { x: pin.x, y: pin.y }
        return { id: pin.id, parcel, position: parcelCenter(parcel), title: pin.title, description: pin.description, icon: pin.icon }
      })
    )
  }
}

export const nearbyPlayers = {
  subscribe: players.subscribe,
  getSnapshot: players.getSnapshot,
  receive(list: MapPlayer[]): void {
    players.set(list)
  },
  /** Wire the engine in; null when it goes away. */
  setTransport(send: ((on: boolean) => void) | null): void {
    watch = send
    if (send == null) players.set([])
  },
  watch(on: boolean): void {
    watch?.(on)
    if (!on) players.set([])
  }
}

/** Counts the player's teleports, so what was read for the old place can be read again. */
export const travelCount = {
  subscribe: travels.subscribe,
  getSnapshot: travels.getSnapshot,
  traveled(): void {
    travels.set(travels.getSnapshot() + 1)
  }
}
