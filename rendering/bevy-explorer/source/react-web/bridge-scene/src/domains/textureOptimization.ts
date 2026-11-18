// Texture optimization: the preview server's background texture import status for the previewed
// scene (dcl-scene-cli's TextureOptimization on the preview socket), relayed as the engine reports
// it, `replay` included. The page decides what to show and offers the reload (reloadScene with the
// scene id, in world.ts).
//   from: BevyApi.getTextureOptimizationStream (preview mode only).
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerTextureOptimization(ctx: Ctx): void {
  const open = BevyApi.getTextureOptimizationStream
  if (open == null) return
  relay('textureOptimization', async () => await open.call(BevyApi), (s) => {
    ctx.send({
      kind: 'textureOptimization',
      sceneId: s.sceneId,
      state: s.state,
      done: s.done,
      total: s.total,
      failed: s.failed,
      bytesBefore: s.bytesBefore,
      bytesAfter: s.bytesAfter,
      enabled: s.enabled,
      replay: s.replay
    })
  })
}
