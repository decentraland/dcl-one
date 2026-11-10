// Outfits: the player's saved avatar looks (backpack Outfits tab).
//   load  ← what this session last saved or read, else catalyst GET /lambdas/outfits/:address
//           (404 → none saved yet; any other failure refuses the save or delete)
//   save/delete → BevyApi.deployOutfits: the engine signs and deploys the `outfits` entity
//                 (slots past FREE_SLOTS carry the player's owned names)
//                 (a guest's saves last until the scene restarts)
//                 a refused one → outfitSaveFailed to the page
//   thumbnails → BevyApi.get/setOutfitThumbnail(s): kept by the engine on this device
//   equip → the Backpack's look (body shape + colors + wearables; ./avatarDraft deploys it on close)
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import { catalystBase, getJson, HttpError } from '../http'
import { fetchOwnedNames } from './profile'
import { sendEquipped } from './wearables'
import { currentLook, editLook } from './avatarDraft'
import { captureAvatarThumbnail, waitFrames } from './avatarPreview'
import type { AvatarLook } from '../../../src/engine/avatarEquip'
import type { OutfitsData } from '../../../src/engine/generated'
import type { Ctx } from '../bridge'
import type { Outfit, OutfitsMetadata, RGBColor } from '../../../src/engine/protocol'

const EMPTY: OutfitsMetadata = { outfits: [], namesForExtraSlots: [] }
const GREY: RGBColor = { r: 0.5, g: 0.5, b: 0.5 }
// The catalyst takes slots 5 to 9 only with a name the deployer owns.
const FREE_SLOTS = 5
// Wait for an equipped outfit's wearables to load before picturing it.
const EQUIP_THUMBNAIL_DELAY_FRAMES = 180

// The catalyst's lambdas lag a deploy, so this session's last known outfits win over a fresh read.
const known = new Map<string, OutfitsMetadata>()
// Each account's slot pictures, read from the engine once per session.
const thumbnails = new Map<string, Promise<Map<number, string>>>()

function thumbnailsOf(address: string): Promise<Map<number, string>> {
  const key = address.toLowerCase()
  let found = thumbnails.get(key)
  if (found == null) {
    found = BevyApi.getOutfitThumbnails()
      .then((stored) => new Map(stored.map((t) => [t.slot, t.thumbnail])))
      .catch((e) => {
        console.error('[outfits] thumbnails read failed', e)
        return new Map<number, string>()
      })
    thumbnails.set(key, found)
  }
  return found
}

async function withThumbnails(address: string, metadata: OutfitsMetadata): Promise<OutfitsMetadata> {
  const pictures = await thumbnailsOf(address)
  return {
    ...metadata,
    outfits: metadata.outfits.map((o) => {
      const thumbnail = pictures.get(o.slot)
      return thumbnail != null ? { ...o, thumbnail } : o
    })
  }
}

async function writeThumbnail(address: string, slot: number, thumbnail: string | null): Promise<void> {
  const pictures = await thumbnailsOf(address)
  if (thumbnail != null) pictures.set(slot, thumbnail)
  else pictures.delete(slot)
  BevyApi.setOutfitThumbnail(slot, thumbnail)
}

// The entity's metadata alone: pictures stay on the device.
function toDeploy(metadata: OutfitsMetadata): OutfitsData {
  return {
    outfits: metadata.outfits.map(({ slot, outfit }) => ({
      slot,
      outfit: {
        bodyShape: outfit.bodyShape,
        eyes: { color: outfit.eyes.color },
        hair: { color: outfit.hair.color },
        skin: { color: outfit.skin.color },
        wearables: outfit.wearables,
        forceRender: outfit.forceRender ?? []
      }
    })),
    namesForExtraSlots: metadata.namesForExtraSlots
  }
}

// A guest's outfits stay in this session; anyone else's are deployed, and kept once the catalyst
// takes them.
async function store(address: string, isGuest: boolean, next: OutfitsMetadata): Promise<OutfitsMetadata> {
  let kept = next
  if (!isGuest) {
    const extra = next.outfits.some((o) => o.slot >= FREE_SLOTS)
    if (extra && next.namesForExtraSlots.length === 0) {
      kept = { ...next, namesForExtraSlots: await fetchOwnedNames(address) }
    }
    await BevyApi.deployOutfits(toDeploy(kept))
  }
  known.set(address.toLowerCase(), kept)
  return kept
}

// Saves and deletes run one at a time, each on the outfits the previous one left.
let pending: Promise<unknown> = Promise.resolve()
function serially<T>(change: () => Promise<T>): Promise<T> {
  const run = pending.then(change, change)
  pending = run.catch(() => undefined)
  return run
}

// This session's outfits, else the catalyst lambdas endpoint, which returns
// { metadata: OutfitsMetadata }, or 404 for an address that never saved one.
async function loadMetadata(address: string): Promise<OutfitsMetadata> {
  const key = address.toLowerCase()
  const local = known.get(key)
  if (local != null) return local
  const base = await catalystBase()
  const res = await getJson<{ metadata?: OutfitsMetadata }>(`${base}/lambdas/outfits/${address}`).catch((e: unknown) => {
    if (e instanceof HttpError && e.status === 404) return undefined
    throw e
  })
  if (res?.metadata == null) return EMPTY
  // A change stored while this read was in flight is newer.
  const stored = known.get(key)
  if (stored != null) return stored
  known.set(key, res.metadata)
  return res.metadata
}

// Capture the Backpack's CURRENT look into a deployable Outfit (colors default to grey if absent).
function currentOutfit(look: AvatarLook): Outfit {
  return {
    bodyShape: look.bodyShape,
    eyes: { color: look.eyes ?? GREY },
    hair: { color: look.hair ?? GREY },
    skin: { color: look.skin ?? GREY },
    wearables: look.wearables,
    forceRender: look.forceRender
  }
}

export function registerOutfits(ctx: Ctx): void {
  const emit = async (metadata: OutfitsMetadata): Promise<void> => {
    const me = getPlayer()
    ctx.send({ kind: 'outfits', metadata: me != null ? await withThumbnails(me.userId, metadata) : metadata })
  }

  // Applies a change to the stored outfits. A failed read refuses it; a change the catalyst
  // refuses shows the outfits as they were. Either way the page is told why.
  const apply = async (
    me: { userId: string; isGuest: boolean },
    change: { slot: number; action: 'save' | 'delete' },
    edit: (metadata: OutfitsMetadata) => OutfitsMetadata
  ): Promise<OutfitsMetadata | null> => {
    let metadata: OutfitsMetadata
    try {
      metadata = await loadMetadata(me.userId)
    } catch (e) {
      ctx.send({ kind: 'outfitSaveFailed', ...change, message: String(e) })
      return null
    }
    try {
      return await store(me.userId, me.isGuest, edit(metadata))
    } catch (e) {
      await emit(metadata)
      ctx.send({ kind: 'outfitSaveFailed', ...change, message: String(e) })
      return null
    }
  }

  ctx.on('getOutfits', async () => {
    const me = getPlayer()
    if (me == null) {
      await emit(EMPTY)
      return
    }
    // Shown, not kept: a save or delete reads again.
    const metadata = await loadMetadata(me.userId).catch((e: unknown) => {
      console.error('[outfits] read failed', e)
      return EMPTY
    })
    await emit(metadata)
  })

  // Save the current look into `slot`, replacing any existing outfit there, then re-emit.
  ctx.on('saveOutfit', async (msg) => {
    const me = getPlayer()
    const look = currentLook()
    if (me == null || look == null) return
    const thumbnail = await captureAvatarThumbnail()
    await serially(async () => {
      const next = await apply(me, { slot: msg.slot, action: 'save' }, (metadata) => {
        const outfits = metadata.outfits.filter((o) => o.slot !== msg.slot)
        outfits.push({ slot: msg.slot, outfit: currentOutfit(look) })
        outfits.sort((a, b) => a.slot - b.slot)
        return { ...metadata, outfits }
      })
      if (next == null) return
      await writeThumbnail(me.userId, msg.slot, thumbnail)
      await emit(next)
    })
  })

  ctx.on('deleteOutfit', async (msg) => {
    const me = getPlayer()
    if (me == null) return
    await serially(async () => {
      const next = await apply(me, { slot: msg.slot, action: 'delete' }, (metadata) => ({
        ...metadata,
        outfits: metadata.outfits.filter((o) => o.slot !== msg.slot)
      }))
      if (next == null) return
      await writeThumbnail(me.userId, msg.slot, null)
      await emit(next)
    })
  })

  // Put a saved outfit's body shape, colors and wearables on the Backpack's look (keeps emotes).
  ctx.on('equipOutfit', async (msg) => {
    const me = getPlayer()
    if (me == null) return
    const metadata = await loadMetadata(me.userId)
    const found = metadata.outfits.find((o) => o.slot === msg.slot)
    if (found == null) return
    const { outfit } = found
    editLook({
      bodyShape: outfit.bodyShape,
      eyes: outfit.eyes.color,
      hair: outfit.hair.color,
      skin: outfit.skin.color,
      wearables: outfit.wearables,
      forceRender: outfit.forceRender
    })
    // Re-emit the equipped set resolved from the outfit's wearables (by urn, independent of the
    // loaded catalog page). Otherwise off-page outfit items never reach the HUD's category slots
    // and the next single-item equip drops them.
    await sendEquipped(ctx)
    if (!(await thumbnailsOf(me.userId)).has(msg.slot)) {
      await waitFrames(EQUIP_THUMBNAIL_DELAY_FRAMES)
      // Changed since: the preview no longer shows this outfit.
      if (JSON.stringify(currentLook()?.wearables) !== JSON.stringify(outfit.wearables)) return
      const thumbnail = await captureAvatarThumbnail()
      if (thumbnail == null) return
      await writeThumbnail(me.userId, msg.slot, thumbnail)
      await emit(await loadMetadata(me.userId))
    }
  })
}
