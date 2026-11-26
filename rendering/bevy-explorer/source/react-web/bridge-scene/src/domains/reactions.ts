// Situational reactions: the emoji the local player picks, which the engine shows over their
// avatar, and each reaction as it plays, for the page to show on screen.
//   from: BevyApi.sendReaction() / getReactionStream()
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerReactions(ctx: Ctx): void {
  ctx.on('sendReaction', (msg) => {
    BevyApi.sendReaction?.(msg.emoji)
  })

  const open = BevyApi.getReactionStream
  if (open == null) return
  relay('reaction', open, (r) => {
    ctx.send({ kind: 'reaction', reaction: { from: r.from.toLowerCase(), emoji: r.emoji } })
  })
}
