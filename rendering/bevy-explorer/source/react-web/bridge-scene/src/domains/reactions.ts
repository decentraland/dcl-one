// Situational reactions: the emoji the local player picks, which the engine shows over their
// avatar.
//   from: BevyApi.sendReaction()
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'

export function registerReactions(ctx: Ctx): void {
  ctx.on('sendReaction', (msg) => {
    BevyApi.sendReaction?.(msg.emoji)
  })
}
