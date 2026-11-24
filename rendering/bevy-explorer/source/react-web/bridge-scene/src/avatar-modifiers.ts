import { BevyApi } from './bevy-api'
import type { Ctx } from './bridge'
import { singleFlight, throttleByDt } from './system-helpers'

export type AvatarModifierSnapshot = {
  passportDisabled: ReadonlySet<string>
  pulseAbsent: ReadonlySet<string>
}

export function registerAvatarModifiers(ctx: Ctx): () => AvatarModifierSnapshot {
  let snapshot: AvatarModifierSnapshot = { passportDisabled: new Set(), pulseAbsent: new Set() }
  ctx.push(throttleByDt(1, singleFlight(async () => {
    try {
      const states = await BevyApi.getAvatarModifiers()
      const passportDisabled = new Set<string>()
      const pulseAbsent = new Set<string>()
      for (const state of states) {
        const key = state.userId.toLowerCase()
        if (state.hideProfile || state.hideAvatar) passportDisabled.add(key)
        if (state.pulseAbsent) pulseAbsent.add(key)
      }
      snapshot = { passportDisabled, pulseAbsent }
    } catch {
      // Keep the last successful snapshot when the engine cannot answer.
    }
  })))
  return () => snapshot
}
