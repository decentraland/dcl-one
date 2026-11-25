// The time of day as the engine reports it for the Skybox menu: whether the scene the player
// stands in holds it, the clock's speed, and the hour while it is fixed.
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { relay } from '../system-helpers'
import type { SkyTimeState } from '@explorer-core/engine/generated'

export function registerSkyTime(ctx: Ctx): void {
  const open = BevyApi.getSkyTimeStream
  if (open == null) return
  let latest: SkyTimeState | null = null
  const send = (): void => {
    if (latest != null) ctx.send({ kind: 'skyTime', state: latest })
  }
  relay('skyTime', async () => await open(), (state) => {
    latest = state
    send()
  })
  // A page that reloads or joins late missed the last push, and the state may not change again.
  ctx.on('hello', send)
}
