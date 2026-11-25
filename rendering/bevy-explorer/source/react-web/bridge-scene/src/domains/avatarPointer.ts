// Nearby-avatar click → profile card. Ported from bevy-ui-scene's avatar-tracker: nearby avatars are
// ECS entities carrying PlayerIdentityData (spawned by the engine, hold the address). We attach a
// standard SDK7 pointer-down to each one (hover "Show Profile") and forward the click to React, which
// opens the ProfileCard anchored at the cursor.
import { engine, pointerEventsSystem, InputAction, PlayerIdentityData, type Entity } from '@dcl/sdk/ecs'
import { getPlayer } from '@dcl/sdk/players'
import type { AvatarModifierSnapshot } from './avatarModifiers'
import type { Ctx } from '../bridge'
import { throttleByDt } from '../system-helpers'

export function registerAvatarPointer(ctx: Ctx, avatarModifiers: () => AvatarModifierSnapshot): void {
  const registered = new Set<Entity>()
  // Scene trigger areas still use the collider when a modifier hides the avatar.
  // Remove its pointer handler so "Show Profile" cannot reveal the hidden avatar.
  ctx.push(
    throttleByDt(0.5, () => {
      const { passportDisabled } = avatarModifiers()
      // Never attach our own avatar — PlayerIdentityData is written for the local player too (profile
      // sync), unlike most foreign-only ECS data.
      const me = getPlayer()?.userId?.toLowerCase()
      const present = new Set<Entity>()
      for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
        if (data.address === '' || data.address.toLowerCase() === me) continue
        if (passportDisabled.has(data.address.toLowerCase())) continue
        present.add(entity)
        if (registered.has(entity)) continue
        registered.add(entity)
        pointerEventsSystem.onPointerDown(
          { entity, opts: { button: InputAction.IA_POINTER, hoverText: 'Show Profile' } },
          () => {
            const address = PlayerIdentityData.getOrNull(entity)?.address
            if (address == null || address === '') return
            // Read the latest snapshot in case the rescan has yet to remove this handler.
            if (avatarModifiers().passportDisabled.has(address.toLowerCase())) return
            // Only the address travels — React resolves the name (nearby roster) and the card's
            // anchor (live DOM cursor) itself.
            ctx.send({ kind: 'avatarClick', address })
          }
        )
      }
      for (const entity of registered) {
        if (present.has(entity)) continue
        registered.delete(entity)
        pointerEventsSystem.removeOnPointerDown(entity)
      }
    })
  )
}
