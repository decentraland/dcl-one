// The signed-in wallet's platform ban, for the HUD's ban screen and its refused-travel notice.
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
  // A page that starts late or reloads missed the notice.
  ctx.on('hello', send)

  const open = BevyApi.getPlatformBanStream
  if (open == null) return
  relay('platformBan', async () => await open(), (ban) => {
    latest = ban
    send()
  })
}
