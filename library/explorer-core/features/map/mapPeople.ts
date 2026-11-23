// What the engine tells the map about the world around the player: the other players it holds,
// and the player's own teleports.

import type { MapPlayer } from '../../engine/protocol'

export function store<T>(initial: T): { subscribe: (listener: () => void) => () => void; getSnapshot: () => T; set: (value: T) => void } {
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

const players = store<MapPlayer[]>([])
const travels = store(0)
let watch: ((on: boolean) => void) | null = null

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
