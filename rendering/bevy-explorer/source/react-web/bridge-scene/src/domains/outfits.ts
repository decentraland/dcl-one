// Outfits: the player's saved avatar looks (backpack Outfits tab).
//   load  ← what this session last saved or read, else catalyst GET /lambdas/outfits/:address
//           (404 → none; any other failure refuses the save or delete; never for a guest)
//   save/delete → BevyApi.deployOutfits: the engine signs and deploys the `outfits` entity
//                 (slots past FREE_SLOTS carry names the player owns now)
//                 (a guest's saves last until the scene restarts)
//                 a save only fills an empty slot
//                 with no name owned, deleting an extra slot clears every extra slot
//                 a refused one → outfitSaveFailed to the page
//   busy → outfitBusy to the page around every save, delete, preview and equip, whatever its outcome;
//          a preview's and an equip's lasts until the Backpack's avatar has loaded the outfit
//   pictures → localStorage, per account and slot: a cache, never deployed, a missing one taken once
//              the outfit is on the avatar
//   preview → the Backpack's look, as an equip, until restoreOutfitPreview puts back the look from
//             before the first preview (the page: leaving Saved Outfits, closing the Backpack);
//             revertAvatar (the page, Revert) forgets that look with the draft it drops
//   equip → the Backpack's look (body shape + colors + wearables; ./avatarDraft deploys it on close);
//           it keeps a previewed look
import { getPlayer } from '@dcl/sdk/players'
import { BevyApi } from '../bevy-api'
import { catalystBase, getJson, HttpError } from '../http'
import { readOwnedNames } from './profile'
import { sendEquipped } from './wearables'
import { currentLook, editLook } from './avatarDraft'
import { captureAvatarThumbnail, lookShown } from './avatarPreview'
import { identity } from '../identity'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import type { OutfitColor, OutfitData, OutfitsData } from '@explorer-core/engine/generated'
import type { Ctx } from '../bridge'
import type { Outfit, OutfitsMetadata, RGBColor } from '@explorer-core/engine/protocol'

// The engine gives system scenes a persistent localStorage, keyed by the scene hash: a deploy empties it.
declare const localStorage: {
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
  removeItem: (key: string) => void
}

const EMPTY: OutfitsMetadata = { outfits: [], namesForExtraSlots: [] }
const GREY: OutfitColor = { r: 0.5, g: 0.5, b: 0.5, a: 1 }
// The catalyst takes slots 5 to 9 only with a name the deployer owns.
const FREE_SLOTS = 5
// Each listed name is an ownership check at the catalyst.
const MAX_NAMES = 5

// Pictures are rendered as <img src>, so only a PNG data URL up to 1 MiB is kept or shown.
const THUMBNAIL_PREFIX = 'data:image/png;base64,'
const MAX_THUMBNAIL_LENGTH = 1024 * 1024

// The catalyst's lambdas lag a deploy, so this session's last known outfits win over a fresh read.
const known = new Map<string, OutfitsMetadata>()

// One picture per slot, so a save rewrites one image.
const thumbKey = (address: string, slot: number): string => `outfit-thumb:${address.toLowerCase()}:${slot}`

function isThumbnail(value: string | null): value is string {
  return value != null && value.startsWith(THUMBNAIL_PREFIX) && value.length <= MAX_THUMBNAIL_LENGTH
}

function thumbnailOf(address: string, slot: number): string | null {
  try {
    const value = localStorage.getItem(thumbKey(address, slot))
    return isThumbnail(value) ? value : null
  } catch {
    return null
  }
}

function withThumbnails(address: string, metadata: OutfitsMetadata): OutfitsMetadata {
  return {
    ...metadata,
    outfits: metadata.outfits.map((o) => {
      const thumbnail = thumbnailOf(address, o.slot)
      return thumbnail != null ? { ...o, thumbnail } : o
    })
  }
}

function writeThumbnail(address: string, slot: number, thumbnail: string | null): void {
  try {
    if (isThumbnail(thumbnail)) localStorage.setItem(thumbKey(address, slot), thumbnail)
    else localStorage.removeItem(thumbKey(address, slot))
  } catch (e) {
    console.error('[outfits] thumbnail write failed', e)
  }
}

/** The profile look's colour, which has no `a`; keeping it would make an unchanged look deploy. */
function rgb({ r, g, b }: OutfitColor): RGBColor {
  return { r, g, b }
}

// Other explorers read a missing alpha as 0, so every colour gets one before it is kept or deployed.
function opaque(color: RGBColor & { a?: number }): OutfitColor {
  return { r: color.r, g: color.g, b: color.b, a: color.a ?? 1 }
}

// The entity's fields alone, with opaque colours: fields other explorers add are dropped.
function normalized(outfit: OutfitData): Outfit {
  return {
    bodyShape: outfit.bodyShape,
    eyes: { color: opaque(outfit.eyes.color) },
    hair: { color: opaque(outfit.hair.color) },
    skin: { color: opaque(outfit.skin.color) },
    wearables: outfit.wearables,
    forceRender: outfit.forceRender ?? []
  }
}

function normalizedMetadata(metadata: OutfitsData): OutfitsMetadata {
  return { ...metadata, outfits: metadata.outfits.map(({ slot, outfit }) => ({ slot, outfit: normalized(outfit) })) }
}

// The entity's metadata alone: pictures stay on the device.
function toDeploy(metadata: OutfitsMetadata): OutfitsData {
  return {
    outfits: metadata.outfits.map(({ slot, outfit }) => ({ slot, outfit })),
    namesForExtraSlots: metadata.namesForExtraSlots
  }
}

// Names only when a slot past the free ones is used, read again so a name sold since is not sent.
// A failed lookup falls back to the names last read with the outfits; the catalyst checks them.
async function namesFor(address: string, metadata: OutfitsMetadata): Promise<string[]> {
  if (!metadata.outfits.some((o) => o.slot >= FREE_SLOTS)) return []
  return ((await readOwnedNames(address)) ?? metadata.namesForExtraSlots).slice(0, MAX_NAMES)
}

// A guest's outfits stay in this session; anyone else's are deployed, and kept once the catalyst
// takes them. A refused deploy leaves the catalyst, and so the session's copy, as it was.
async function store(address: string, isGuest: boolean, next: OutfitsMetadata): Promise<OutfitsMetadata> {
  let kept = next
  if (!isGuest) {
    kept = { ...next, namesForExtraSlots: await namesFor(address, next) }
    await BevyApi.deployOutfits(address, toDeploy(kept))
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
// { metadata: OutfitsMetadata }, or 404 for an address that never deployed one. A guest never reads it.
async function loadMetadata(address: string, isGuest: boolean): Promise<OutfitsMetadata> {
  const key = address.toLowerCase()
  const local = known.get(key)
  if (local != null) return local
  if (isGuest) return EMPTY
  const base = await catalystBase()
  const res = await getJson<{ metadata?: OutfitsData }>(`${base}/lambdas/outfits/${address}`).catch((e: unknown) => {
    if (e instanceof HttpError && e.status === 404) return undefined
    throw e
  })
  // A change stored while this read was in flight is newer; a stored entry is never dropped.
  const stored = known.get(key)
  if (stored != null) return stored
  if (res?.metadata == null) return EMPTY
  const metadata = normalizedMetadata(res.metadata)
  known.set(key, metadata)
  return metadata
}

// Same look, whatever the key order. Both sides are normalized: opaque colours, a forceRender list.
function sameOutfit(a: Outfit, b: Outfit): boolean {
  const color = ({ r, g, b, a }: OutfitColor): string => `${r},${g},${b},${a}`
  const list = (l: string[]): string => JSON.stringify(l)
  return (
    a.bodyShape === b.bodyShape &&
    color(a.eyes.color) === color(b.eyes.color) &&
    color(a.hair.color) === color(b.hair.color) &&
    color(a.skin.color) === color(b.skin.color) &&
    list(a.wearables) === list(b.wearables) &&
    list(a.forceRender) === list(b.forceRender)
  )
}

// Capture the Backpack's CURRENT look into a deployable Outfit (colors default to grey if absent).
function currentOutfit(look: OutfitLook): Outfit {
  return {
    bodyShape: look.bodyShape,
    eyes: { color: opaque(look.eyes ?? GREY) },
    hair: { color: opaque(look.hair ?? GREY) },
    skin: { color: opaque(look.skin ?? GREY) },
    wearables: look.wearables,
    forceRender: look.forceRender
  }
}

// What an outfit changes on the Backpack's look as it was before the first preview: what leaving the
// Saved Outfits puts back. An equip keeps the previewed look, so it forgets this.
type OutfitLook = Pick<AvatarLook, 'bodyShape' | 'eyes' | 'hair' | 'skin' | 'wearables' | 'forceRender'>
let beforePreview: OutfitLook | null = null
// Counts previews and restores: a preview still reading its outfit when a later one, or the restore,
// arrives puts nothing on.
let previews = 0
function forgetPreview(): void {
  beforePreview = null
  previews++
}
identity.onChange(forgetPreview)

export function registerOutfits(ctx: Ctx): void {
  // Pictures of the account the outfits belong to, even if the player changed meanwhile.
  const emit = async (address: string | null, metadata: OutfitsMetadata): Promise<void> => {
    ctx.send({ kind: 'outfits', metadata: address != null ? withThumbnails(address, metadata) : metadata })
  }

  // Applies a change to the stored outfits. A failed read refuses it; a change the edit or the
  // catalyst refuses shows the outfits as they are. Either way the page is told why.
  const apply = async (
    me: { userId: string; isGuest: boolean },
    change: { slot: number; action: 'save' | 'delete' },
    edit: (metadata: OutfitsMetadata) => OutfitsMetadata
  ): Promise<OutfitsMetadata | null> => {
    let metadata: OutfitsMetadata
    try {
      metadata = await loadMetadata(me.userId, me.isGuest)
    } catch (e) {
      ctx.send({ kind: 'outfitSaveFailed', ...change, message: String(e) })
      return null
    }
    try {
      const next = edit(metadata)
      return next === metadata ? metadata : await store(me.userId, me.isGuest, next)
    } catch (e) {
      await emit(me.userId, metadata)
      ctx.send({ kind: 'outfitSaveFailed', ...change, message: String(e) })
      return null
    }
  }

  // Tells the page the slot is busy until `work` settles: some outcomes send nothing else.
  const busy = async <T>(slot: number, action: 'save' | 'delete' | 'preview' | 'equip', work: () => Promise<T>): Promise<T> => {
    ctx.send({ kind: 'outfitBusy', slot, action, busy: true })
    try {
      return await work()
    } finally {
      ctx.send({ kind: 'outfitBusy', slot, action, busy: false })
    }
  }

  ctx.on('getOutfits', async () => {
    const me = getPlayer()
    if (me == null) {
      await emit(null, EMPTY)
      return
    }
    // Shown, not kept: a save or delete reads again, and a save never replaces an outfit.
    const metadata = await loadMetadata(me.userId, me.isGuest).catch((e: unknown) => {
      console.error('[outfits] read failed', e)
      return EMPTY
    })
    await emit(me.userId, metadata)
  })

  // Save the current look into the empty `slot`, then re-emit. The page offers only empty slots; one
  // that holds another outfit by the time the save runs is refused, and the page sees what it holds.
  // One that already holds this look (a repeated click) is left as it is.
  ctx.on('saveOutfit', async (msg) => {
    await busy(msg.slot, 'save', async () => {
      const me = getPlayer()
      const look = currentLook()
      if (me == null || look == null) return
      const thumbnail = await captureAvatarThumbnail()
      await serially(async () => {
        const outfit = currentOutfit(look)
        const next = await apply(me, { slot: msg.slot, action: 'save' }, (metadata) => {
          const held = metadata.outfits.find((o) => o.slot === msg.slot)
          if (held != null && sameOutfit(held.outfit, outfit)) return metadata
          if (held != null) throw new Error(`slot ${msg.slot} holds an outfit`)
          const outfits = [...metadata.outfits]
          outfits.push({ slot: msg.slot, outfit })
          outfits.sort((a, b) => a.slot - b.slot)
          return { ...metadata, outfits }
        })
        if (next == null) return
        writeThumbnail(me.userId, msg.slot, thumbnail)
        await emit(me.userId, next)
      })
    })
  })

  ctx.on('deleteOutfit', async (msg) => {
    await busy(msg.slot, 'delete', async () => {
      const me = getPlayer()
      if (me == null) return
      await serially(async () => {
        // With no name owned, the catalyst refuses any deploy that still holds a slot past the free
        // ones, so deleting one of them one at a time is refused while another remains: clear them all.
        const clearExtras =
          msg.slot >= FREE_SLOTS && !me.isGuest && (await readOwnedNames(me.userId).catch(() => undefined))?.length === 0
        const removed = (o: { slot: number }): boolean => (clearExtras ? o.slot >= FREE_SLOTS : o.slot === msg.slot)
        let slots: number[] = []
        const next = await apply(me, { slot: msg.slot, action: 'delete' }, (metadata) => {
          slots = metadata.outfits.filter(removed).map((o) => o.slot)
          return { ...metadata, outfits: metadata.outfits.filter((o) => !removed(o)) }
        })
        if (next == null) return
        for (const slot of slots.length > 0 ? slots : [msg.slot]) writeThumbnail(me.userId, slot, null)
        await emit(me.userId, next)
      })
    })
  })

  // Put a saved outfit's body shape, colors and wearables on the Backpack's look (keeps emotes).
  const wear = async (slot: number, action: 'preview' | 'equip', current: () => boolean): Promise<void> => {
    const me = getPlayer()
    // Not the stored outfit: a stored colour's alpha never reaches the look.
    let put: Outfit | null = null
    const shows = (): boolean => {
      const look = currentLook()
      return put != null && look != null && current() && sameOutfit(currentOutfit(look), put)
    }
    // Busy until the Backpack's avatar has loaded the look; the missing picture below is taken after.
    const worn = await busy(slot, action, async (): Promise<Outfit | null> => {
      if (me == null) return null
      const metadata = await loadMetadata(me.userId, me.isGuest).catch((e: unknown) => {
        console.error('[outfits] read failed', e)
        return null
      })
      const found = metadata?.outfits.find((o) => o.slot === slot)?.outfit
      if (found == null || !current()) return null
      const change: OutfitLook = {
        bodyShape: found.bodyShape,
        eyes: rgb(found.eyes.color),
        hair: rgb(found.hair.color),
        skin: rgb(found.skin.color),
        wearables: found.wearables,
        forceRender: found.forceRender
      }
      editLook(change)
      put = currentOutfit(change)
      // Re-emit the equipped set resolved from the outfit's wearables (by urn, independent of the
      // loaded catalog page). Otherwise off-page outfit items never reach the HUD's category slots
      // and the next single-item equip drops them.
      // Watched from now: the preview takes the look up on its own before the set is resolved.
      const shown = lookShown()
      await sendEquipped(ctx)
      await shown
      return found
    })
    if (me == null || worn == null) return
    if (thumbnailOf(me.userId, slot) == null) {
      // Changed since, or while the picture was taken: the avatar may not show this outfit.
      if (!shows()) return
      const thumbnail = await captureAvatarThumbnail()
      if (thumbnail == null || !shows()) return
      await serially(async () => {
        // A save or delete may have changed the slot meanwhile.
        const now = await loadMetadata(me.userId, me.isGuest).catch(() => null)
        const held = now?.outfits.find((o) => o.slot === slot)
        if (now == null || held == null || !sameOutfit(held.outfit, worn)) return
        if (thumbnailOf(me.userId, slot) != null) return
        writeThumbnail(me.userId, slot, thumbnail)
        await emit(me.userId, now)
      })
    }
  }

  ctx.on('previewOutfit', async (msg) => {
    const look = currentLook()
    if (beforePreview == null && look != null) {
      const { bodyShape, eyes, hair, skin, wearables, forceRender } = look
      beforePreview = { bodyShape, eyes, hair, skin, wearables, forceRender }
    }
    const preview = ++previews
    await wear(msg.slot, 'preview', () => preview === previews)
  })

  ctx.on('equipOutfit', async (msg) => {
    beforePreview = null
    await wear(msg.slot, 'equip', () => true)
  })

  // The draft is dropped (./avatarDraft): a restore would put it back for the next close to deploy.
  ctx.on('revertAvatar', forgetPreview)

  // The look is put back before anything is awaited: the commitAvatar that follows a closing
  // Backpack's restore deploys it.
  ctx.on('restoreOutfitPreview', async () => {
    previews++
    if (beforePreview == null) return
    editLook(beforePreview)
    beforePreview = null
    await sendEquipped(ctx)
  })
}
