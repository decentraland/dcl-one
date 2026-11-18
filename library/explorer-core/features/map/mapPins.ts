// The map pins of portable experiences, as the engine last sent them.

import type { MapPinInfo } from '../../engine/generated'
import { parcelCenter, type Vec2 } from './mapCoords'

export interface PinMarker {
  id: string
  key: Vec2
  position: Vec2
  title: string
  icon: string | null
}

let pins: PinMarker[] = []
const listeners = new Set<() => void>()

export function pinMarkers(list: readonly MapPinInfo[]): PinMarker[] {
  return list.map((pin) => {
    const key = { x: pin.x, y: pin.y }
    return { id: pin.id, key, position: parcelCenter(key), title: pin.title, icon: pin.icon }
  })
}

export const mapPins = {
  subscribe: (listener: () => void): (() => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  },
  getSnapshot: (): readonly PinMarker[] => pins,
  receive(list: readonly MapPinInfo[]): void {
    pins = pinMarkers(list)
    listeners.forEach((l) => l())
  },
  clear(): void {
    if (pins.length === 0) return
    pins = []
    listeners.forEach((l) => l())
  }
}
