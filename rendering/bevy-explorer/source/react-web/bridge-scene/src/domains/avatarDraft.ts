// The Backpack's unsaved look. Equips, emote slots and outfits edit this draft while the Backpack is
// open, the preview avatar wears it, and closing the Backpack deploys it in one setAvatar (Unity
// publishes the profile when its Backpack closes: BackpackController.Deactivate).
//   from: equip / equipEmote / equipOutfit (editLook), commitAvatar (the page, on close or Retry),
//         revertAvatar (the page, Revert), /lambdas/collections (which body shapes each item fits,
//         via ./collections), profileSaveSettled (./profile, a profile save that replaced a commit)
//   to:   BevyApi.setAvatar; avatarSaveFailed to the page
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
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
// A commit that a later setAvatar replaced before it deployed: its draft and the look it sent. The
// later save carries that look, so its outcome settles the commit (`profileSaveSettled`; another
// close and the welcome page's save cover it with their own result).
let superseded: { edited: AvatarLook; look: AvatarLook } | null = null
// The look the stage wears before there is a player: the guest start page's pick, which the
// sign-in then carries, so the player arrives already wearing it.
let standIn: AvatarLook | null = null
identity.onChange(() => {
  draft = null
  deployed = null
  superseded = null
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
  superseded = null
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
export async function deployLookAs(name: string): Promise<void> {
  const look = currentLook()
  if (look == null) throw new Error('no player')
  const edited = draft
  try {
    await BevyApi.setAvatar({
      base: {
        name,
        bodyShapeUrn: look.bodyShape,
        eyesColor: look.eyes,
        hairColor: look.hair,
        skinColor: look.skin,
        talking: null
      },
      equip: { wearableUrns: look.wearables, emoteUrns: look.emotes, forceRender: look.forceRender }
    })
  } catch (e) {
    // a failed save took a replaced commit's look out of the engine with its own
    if (!isCancelled(e)) superseded = null
    throw e
  }
  deployed = look
  superseded = null
  if (draft === edited) draft = null
}

/** A profile save (name, about) finished. It settles a Backpack commit it replaced before that one
 *  deployed: the save carried the commit's look, so a save that deployed put the look on the
 *  server, and a failed one took it out of the engine with the rest of the edit, leaving the draft
 *  for the next close or Retry. */
export function profileSaveSettled(saved: boolean): void {
  const replaced = superseded
  superseded = null
  if (replaced == null || !saved) return
  deployed = replaced.look
  if (draft === replaced.edited) draft = null
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
    const from = deployed
    const me = getPlayer()
    if (edited == null || from == null || me == null) return
    // the account can change while this waits; the look must not land on the next one
    const signedOut = (): boolean => getPlayer()?.userId !== me.userId
    const look = await fittingLook(edited)
    if (signedOut()) return
    // The page keeps its own copy of the equipped set, which still has the dropped wearables.
    const dropped = look.wearables.length !== edited.wearables.length
    if (sameLook(look, from)) {
      if (draft === edited) draft = null
      if (dropped) await sendEquipped()
      return
    }
    try {
      await BevyApi.setAvatar(lookDeploy(look, from, me.name))
    } catch (e) {
      if (signedOut()) return
      // "cancelled" isn't a failure of its own: a later setAvatar (another close, a profile save)
      // replaced this one before it deployed and carries this look. The server only has it once
      // that one lands, and if that one fails the engine goes back to the look from before both,
      // so the draft and `deployed` stay as they are until the later save settles them.
      if (isCancelled(e)) {
        superseded = { edited, look }
        return
      }
      superseded = null
      console.error('[avatar] deploy failed', e)
      ctx.send({ kind: 'avatarSaveFailed', message: String(e) })
      return
    }
    if (signedOut()) return
    deployed = look
    superseded = null
    if (draft === edited) draft = null
    if (dropped) await sendEquipped()
  })

  // Give up on a look the server keeps rejecting (a bad item, say): drop the draft, so the stage shows
  // the player again. The engine put the deployed look back on the player when the deploy failed, so
  // nothing deploys; the page re-reads the equipped sets straight after sending this.
  ctx.on('revertAvatar', () => {
    draft = null
  })
}
