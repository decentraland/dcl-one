// The Backpack's unsaved look. Equips, emote slots and outfits edit this draft while the Backpack is
// open, the preview avatar wears it, and closing the Backpack deploys it in one setAvatar (Unity
// publishes the profile when its Backpack closes: BackpackController.Deactivate).
//   from: equip / equipEmote / equipOutfit (editLook), commitAvatar (the page, on close or Retry),
//         revertAvatar (the page, Revert), /lambdas/collections (which body shapes each item fits,
//         via ./collections)
//   to:   BevyApi.setAvatar; avatarSaveFailed to the page
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import { avatarSaveSession } from './avatarSaves'
import type { Ctx } from '../bridge'
import { catalystBase } from '../http'
import { resolveDefsByUrn } from './collections'
import { itemUrn } from './urns'
import { lookDeploy, sameLook, type AvatarLook } from '@explorer-core/engine/avatarEquip'
import { bodyShapesOf, isCompatible } from '@explorer-core/engine/bodyShape'
import { identity } from '../identity'

let draft: AvatarLook | null = null
// The look last known to be on the server: the draft's seed, then each successful deploy. A failed
// deploy keeps the draft, so the next close (or Retry) deploys it again; the engine has already put
// the deployed look back on the player.
let deployed: AvatarLook | null = null
// The look the stage wears before there is a player: a new account's pick, which the sign-in then
// carries, so the player arrives already wearing it.
let standIn: AvatarLook | null = null
identity.onChange(() => {
  draft = null
  deployed = null
  standIn = null
})

const isCancelled = (e: unknown): boolean => String(e).includes('cancelled')

function playerLook(): AvatarLook | null {
  const p = getPlayer()
  if (p == null) return null
  return {
    bodyShape: p.avatar?.bodyShapeUrn ?? '',
    eyes: p.avatar?.eyesColor ?? null,
    hair: p.avatar?.hairColor ?? null,
    skin: p.avatar?.skinColor ?? null,
    wearables: (p.wearables ?? []).map(String),
    emotes: (p.emotes ?? []).map(String),
    forceRender: (p.forceRender ?? []).map(String)
  }
}

/** What the stage shows: the draft, else the live player, else the stand-in. */
export function currentLook(): AvatarLook | null {
  return draft ?? playerLook() ?? standIn
}

/** The look to show while there is no player. A look left unsaved by the account signed out since
 *  (its deploy failed, or sign-out beat it) is not this one's: without it the stage would keep
 *  showing it, and never take up the pick. */
export function setStandInLook(look: AvatarLook): void {
  standIn = look
  draft = null
  deployed = null
}

/** Change the draft (seeded from the live player on the first edit). Nothing deploys until commit. */
export function editLook(change: Partial<AvatarLook>): void {
  const from = currentLook()
  if (from == null) return
  if (draft == null) deployed = from
  draft = { ...from, ...change }
}

/** Whether the look has unsaved edits. */
export function hasDraft(): boolean {
  return draft != null
}

/** The welcome page's accept: the look (the draft, else the live one) under `name`, in one setAvatar
 *  that resolves once the deploy lands. Rejects with the engine's error. */
export async function deployLookAs(name: string): Promise<boolean> {
  const session = avatarSaveSession(getPlayer()?.userId ?? null)
  const look = currentLook()
  if (look == null) throw new Error('no player')
  const edited = draft
  try {
    await session.run(
      async () => await BevyApi.setAvatar({
        base: {
          name,
          bodyShapeUrn: look.bodyShape,
          eyesColor: look.eyes,
          hairColor: look.hair,
          skinColor: look.skin,
          talking: null
        },
        equip: { wearableUrns: look.wearables, emoteUrns: look.emotes, forceRender: look.forceRender }
      }),
      'avatar'
    )
  } catch (e) {
    if (!session.isCurrent()) return false
    throw e
  }
  if (!session.isCurrent()) return false
  deployed = look
  if (draft === edited) draft = null
  return true
}

// The look as it deploys: without the wearables that can't render on its body shape (Unity unequips
// them when the body shape changes). The draft keeps them, so switching the body shape and back while
// the Backpack is open loses nothing. An item the catalyst can't resolve is kept.
async function fittingLook(look: AvatarLook): Promise<AvatarLook> {
  const defs = await resolveDefsByUrn('wearables', await catalystBase(), look.wearables.map(itemUrn))
  const fits = (urn: string): boolean => {
    const def = defs.get(itemUrn(urn))
    if (def == null) return true
    return isCompatible({ category: def.data?.category ?? '', bodyShapes: bodyShapesOf(def.data?.representations) }, look.bodyShape || undefined)
  }
  return { ...look, wearables: look.wearables.filter(fits) }
}

// sendEquipped (./wearables) is passed in: it reads this module, so importing it here would be a cycle.
export function registerAvatarDraft(ctx: Ctx, sendEquipped: () => Promise<void>): void {
  ctx.on('commitAvatar', async () => {
    const edited = draft
    const me = getPlayer()
    if (edited == null || deployed == null || me == null) return
    const session = avatarSaveSession(me.userId)
    const syncEquipped = await session.run(async () => {
      const look = await fittingLook(edited)
      if (!session.isCurrent()) return false
      // A queued close may have changed the deployed look while this Backpack was open.
      const from = deployed
      if (from == null) return false
      const dropped = look.wearables.length !== edited.wearables.length
      if (sameLook(look, from)) {
        if (draft === edited) draft = null
        return dropped
      }
      try {
        await BevyApi.setAvatar(lookDeploy(look, from))
      } catch (e) {
        if (!session.isCurrent()) return false
        // The later write owns the result when an overlapping commit replaces this one.
        if (isCancelled(e)) return false
        console.error('[avatar] deploy failed', e)
        ctx.send({ kind: 'avatarSaveFailed', message: String(e) })
        return false
      }
      if (!session.isCurrent()) return false
      deployed = look
      if (draft === edited) draft = null
      return dropped
    }, 'avatar')
    if (syncEquipped) await sendEquipped()
  })

  // Give up on a look the server keeps rejecting (a bad item, say): drop the draft, so the stage shows
  // the player again. The engine put the deployed look back on the player when the deploy failed, so
  // nothing deploys; the page re-reads the equipped sets straight after sending this.
  ctx.on('revertAvatar', () => {
    draft = null
  })
}
