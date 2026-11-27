// The local user's scene or world ban for the scene they stand in, relayed to the page.
//   from: SystemApi.getSceneBanStream (the engine's ban_block answer, whole on every change)
//   to:   the page, as `sceneBan`
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { SceneBanNotice } from '@explorer-core/engine/generated'

export function registerSceneBan(ctx: Ctx): void {
  let latest: SceneBanNotice | null = null
  const send = (): void => {
    if (latest != null) ctx.send({ kind: 'sceneBan', ban: latest })
  }
  // Re-sends the last notice on every `hello`.
  ctx.on('hello', send)

  const open = BevyApi.getSceneBanStream
  if (open == null) return
  relay('sceneBan', async () => await open(), (ban) => {
    latest = ban
    send()
  })
}
