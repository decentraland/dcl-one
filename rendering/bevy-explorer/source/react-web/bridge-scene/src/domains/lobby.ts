import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'

export function registerLobby(ctx: Ctx): void {
  ctx.on('quit', () => {
    BevyApi.quit?.()
  })
}
