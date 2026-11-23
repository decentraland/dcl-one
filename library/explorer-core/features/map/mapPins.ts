// The map pins of portable experiences, as the engine last sent them.

import type { MapPinInfo } from '../../engine/generated'
import { parcelCenter, parcelWithinBounds, type Vec2 } from './mapCoords'
import { store } from './mapPeople'

export interface PinMarker {
  id: string
  key: Vec2
  position: Vec2
  title: string
  icon: string | null
}

const pins = store<PinMarker[]>([])

/** A pin outside the city is dropped: there is no parcel there to pick. */
export function pinMarkers(list: readonly MapPinInfo[]): PinMarker[] {
  return list.flatMap((pin) => {
    const key = { x: pin.x, y: pin.y }
    const position = parcelCenter(key)
    if (parcelWithinBounds(position) == null) return []
    return [{ id: pin.id, key, position, title: pin.title, icon: pin.icon }]
  })
}

export const mapPins = {
  subscribe: pins.subscribe,
  getSnapshot: (): readonly PinMarker[] => pins.getSnapshot(),
  receive(list: readonly MapPinInfo[]): void {
    pins.set(pinMarkers(list))
  },
  clear(): void {
    if (pins.getSnapshot().length > 0) pins.set([])
  }
}
