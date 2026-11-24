// Map pins: relays the map pins of portable experiences to the HUD.
//   from: SystemApi.getMapPinsStream.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { MapPinInfo } from '@explorer-core/engine/generated'

export function registerMapPins(ctx: Ctx): void {
  let last: MapPinInfo[] = []
  relay('mapPins', async () => await BevyApi.getMapPinsStream(), (pins) => {
    last = pins
    ctx.send({ kind: 'mapPins', pins })
  })
  // A reloaded page starts with no list, so it has to be sent the current one again.
  ctx.on('hello', () => {
    ctx.send({ kind: 'mapPins', pins: last })
  })
}
