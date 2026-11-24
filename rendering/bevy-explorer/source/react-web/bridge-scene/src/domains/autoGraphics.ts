// What Auto Graphics lowered for the session, relayed to the page.
//   from: SystemApi.getAutoGraphicsStream (one notice each time it settles on lower graphics)
//   to:   the page, as `autoGraphics`
// A notice is a one-shot event, so it is not sent again on `hello`.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerAutoGraphics(ctx: Ctx): void {
  const open = BevyApi.getAutoGraphicsStream
  if (open == null) return
  relay('autoGraphics', async () => await open(), (notice) => {
    ctx.send({ kind: 'autoGraphics', notice })
  })
}
