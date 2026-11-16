// The signed-in wallet's platform ban, relayed to the page.
//   from: SystemApi.getPlatformBanStream (the engine's ban_block answer, whole on every change)
//   to:   the page, as `platformBan`
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { PlatformBanNotice } from '@explorer-core/engine/generated'

export function registerPlatformBan(ctx: Ctx): void {
  let latest: PlatformBanNotice | null = null
  const send = (): void => {
    if (latest != null) ctx.send({ kind: 'platformBan', ban: latest })
  }
  // Re-sends the last notice on every `hello`.
  ctx.on('hello', send)

  const open = BevyApi.getPlatformBanStream
  if (open == null) return
  relay('platformBan', async () => await open(), (ban) => {
    latest = ban
    send()
  })
}
