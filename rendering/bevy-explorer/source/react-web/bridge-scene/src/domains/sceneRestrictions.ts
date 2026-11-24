// What the scene the player stands in restricts, relayed as the engine reports it: the minimap's
// restrictions list.
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { relay } from '../system-helpers'
import type { SceneRestrictions } from '@explorer-core/engine/generated'

export function registerSceneRestrictions(ctx: Ctx): void {
  const open = BevyApi.getSceneRestrictionsStream
  if (open == null) return
  let latest: SceneRestrictions | null = null
  const send = (): void => {
    if (latest != null) ctx.send({ kind: 'sceneRestrictions', state: latest })
  }
  relay('sceneRestrictions', async () => await open(), (state) => {
    latest = state
    send()
  })
  // A page that reloads or joins late missed the last push, and the state may not change again.
  ctx.on('hello', send)
}
