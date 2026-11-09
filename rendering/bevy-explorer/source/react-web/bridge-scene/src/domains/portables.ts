// Portable experiences: relays the engine's list of the portables the player can see and stop
// (every portable but the HUD's own scene and the startup ones, tagged by kind) to the HUD. The
// engine sends the whole list on subscribe and on every change. Stop and Restart go through the
// engine console (`/stop_portable <hash>`, `/smart_wearable restart <urn>`, domains/world.ts).
//   from: SystemApi.getPortablesStream.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { PortableInfo } from '../../../src/engine/generated'

export function registerPortables(ctx: Ctx): void {
  let last: PortableInfo[] = []
  relay('portables', async () => await BevyApi.getPortablesStream(), (portables) => {
    last = portables
    ctx.send({ kind: 'portables', portables })
  })
  // A reloaded page starts with no list, so it has to be sent the current one again.
  ctx.on('hello', () => {
    ctx.send({ kind: 'portables', portables: last })
  })
}
