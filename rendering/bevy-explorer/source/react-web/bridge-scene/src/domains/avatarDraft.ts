// The Backpack's unsaved look. Equips, emote slots and outfits edit this draft while the Backpack is
// open, the preview avatar wears it, and closing the Backpack deploys it in one setAvatar (Unity
// publishes the profile when its Backpack closes: BackpackController.Deactivate).
//   from: equip / equipEmote / equipOutfit (editLook), commitAvatar (the page, on close or Retry),
//         revertAvatar (the page, Revert), /lambdas/collections (which body shapes each item fits,
//         via ./collections)
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
// The look the stage wears before there is a player: a new account's pick, which the sign-in then
// carries, so the player arrives already wearing it.
let standIn: AvatarLook | null = null
// Compare a close against the latest look sent to the engine, including an in-flight close
// that the player is undoing. Finish replies can arrive in either order.
const submitted = new Map<number, AvatarLook>()
let latestClose = 0
let committedClose = 0
let latestSubmission = 0
let draftAccount: (() => boolean) | undefined
function clearAccountLook(): void {
  draft = null
  deployed = null
  standIn = null
  submitted.clear()
  draftAccount = undefined
}
identity.onChange(clearAccountLook)

function checkDraftAccount(): void {
  if (draftAccount && !draftAccount()) clearAccountLook()
}

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
  checkDraftAccount()
  return draft ?? playerLook() ?? standIn
}

/** The look to show while there is no player. A look left unsaved by the account signed out since
 *  (its deploy failed, or sign-out beat it) is not this one's: without it the stage would keep
 *  showing it, and never take up the pick. */
export function setStandInLook(look: AvatarLook): void {
  clearAccountLook()
  standIn = look
}

/** Change the draft (seeded from the live player on the first edit). Nothing deploys until commit. */
export function editLook(change: Partial<AvatarLook>): void {
  const from = currentLook()
  if (from == null) return
  if (draft == null) {
    deployed = from
    draftAccount = identity.session(() => getPlayer()?.userId ?? null)
  }
  draft = { ...from, ...change }
}

/** Whether the look has unsaved edits. */
export function hasDraft(): boolean {
  checkDraftAccount()
  return draft != null
}

/** Deploy the Welcome look and apply a name only when Welcome supplies one. A stale session
 *  drops its result. A failure in the current session rejects so Welcome can offer a retry. */
export async function deployLookAs(name: string | undefined, isCurrent: () => boolean): Promise<void> {
  const look = currentLook()
  if (look == null) throw new Error('no player')
  const edited = draft
  await BevyApi.setAvatar({
    base: {
      name: name ?? '',
      bodyShapeUrn: look.bodyShape,
      eyesColor: look.eyes,
      hairColor: look.hair,
      skinColor: look.skin,
      talking: null
    },
    equip: { wearableUrns: look.wearables, emoteUrns: look.emotes, forceRender: look.forceRender }
  })
  if (!isCurrent()) return
  deployed = look
  if (draft === edited) draft = null
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
    checkDraftAccount()
    const edited = draft
    if (edited == null || deployed == null || getPlayer() == null) return
    const isCurrent = identity.session(() => getPlayer()?.userId ?? null)
    const close = ++latestClose
    try {
      const from = Array.from(submitted.values()).pop() ?? deployed
      const changedWearables = edited.bodyShape !== from.bodyShape ||
        JSON.stringify(edited.wearables) !== JSON.stringify(from.wearables)
      const look = changedWearables ? await fittingLook(edited) : edited
      // A newer close can finish fitting before this one. Only submit the latest intent.
      if (!isCurrent() || close !== latestClose) return
      const pending = Array.from(submitted.values())
      const last = pending[pending.length - 1]
      if (last && sameLook(look, last)) return // Its engine reply owns this same draft.
      const patch = lookDeploy(look, deployed, ...pending)
      if (patch.base || patch.equip) {
        latestSubmission = close
        submitted.set(close, look)
        await BevyApi.setAvatar(patch)
        if (!isCurrent() || close < committedClose) return
        committedClose = close
        deployed = look
        for (const id of submitted.keys()) if (id <= close) submitted.delete(id)
      }
      if (draft && sameLook(draft, edited)) draft = null
      if (look.wearables.length !== edited.wearables.length) await sendEquipped()
    } catch (e) {
      if (!isCurrent() || close < latestSubmission) return
      console.error('[avatar] deploy failed', e)
      ctx.send({ kind: 'avatarSaveFailed', message: String(e) })
    } finally {
      submitted.delete(close)
    }
  })

  // Give up on a look the server keeps rejecting (a bad item, say): drop the draft, so the stage shows
  // the player again. The engine put the deployed look back on the player when the deploy failed, so
  // nothing deploys; the page re-reads the equipped sets straight after sending this.
  ctx.on('revertAvatar', () => {
    latestClose++ // A fit still in progress belongs to the discarded draft.
    draft = null
  })
}
