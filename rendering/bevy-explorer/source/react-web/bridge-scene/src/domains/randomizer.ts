// Randomize outfit (Backpack): rolls a random look from the player's own and the base wearables,
// keeping the body shape and colors, with undo/redo over the looks of the Backpack session. Mirrors
// Unity's RandomizeOutfitController: the requests per roll do not grow with the inventory.
//   from: randomizeOutfit / undoOutfit / redoOutfit / resetOutfitHistory (the page),
//         catalyst GET /explorer/:address/wearables via ./wearables (the base set and the per-category
//         owned totals once per session, then per roll one page per chosen category)
//   to:   the Backpack's look (./avatarDraft, deployed on close); the equipped set and outfitHistory to the page
import { getPlayer } from '@dcl/sdk/players'
import type { Ctx } from '../bridge'
import type { AvatarLook } from '../../../src/engine/avatarEquip'
import { OUTFIT_CATEGORIES, OutfitRandomizer, isCoreCategory, type RollItem } from '../../../src/engine/outfitRandomizer'
import { currentLook, editLook } from './avatarDraft'
import { deployableUrn, fetchWearablesPage, sendEquipped } from './wearables'
import { identity } from '../identity'

/** Size of the owned page a roll fetches per category; the rest of the page is the category's repick pool. */
const OWNED_PAGE_SIZE = 30
/** Large enough for every base wearable to arrive in one page. */
const BASE_PAGE_SIZE = 1000
const OWNED_COLLECTIONS = ['on-chain', 'third-party']

const randomizer = new OutfitRandomizer()
let baseItems: RollItem[] | null = null
let ownedTotals: Map<string, number> | null = null
const undoStack: AvatarLook[] = []
const redoStack: AvatarLook[] = []
let rolling = false
// Bumped by a reset, so a roll that started before it drops its result.
let session = 0

function reset(): void {
  session += 1
  baseItems = null
  ownedTotals = null
  undoStack.length = 0
  redoStack.length = 0
  rolling = false
}
identity.onChange(reset)

async function loadBaseItems(address: string): Promise<RollItem[]> {
  const page = await fetchWearablesPage(address, { page: 0, pageSize: BASE_PAGE_SIZE, collectionTypes: ['base-wearable'] })
  return page.items
}

// One single-item request per category, read only for the total it reports. A failed request reports 0,
// so that category uses base wearables for the rest of the session.
async function loadOwnedTotals(address: string): Promise<Map<string, number>> {
  const totals = await Promise.all(
    OUTFIT_CATEGORIES.map(async (category) => {
      const page = await fetchWearablesPage(address, { page: 0, pageSize: 1, category, collectionTypes: OWNED_COLLECTIONS })
      return [category, page.total] as const
    })
  )
  return new Map(totals)
}

// For each chosen category with owned wearables, the page holding a random owned index; the chosen
// optional categories share one request. The item at that index is the category's first custom pick.
async function loadOwnedPages(address: string, chosen: readonly string[], totals: ReadonlyMap<string, number>): Promise<void> {
  const requests: Array<Promise<void>> = []
  const optional: string[] = []
  let optionalTotal = 0
  for (const category of chosen) {
    const total = totals.get(category) ?? 0
    if (total === 0) continue
    if (isCoreCategory(category)) requests.push(loadOwnedPage(address, [category], randomizer.pickIndex(total)))
    else {
      optional.push(category)
      optionalTotal += total
    }
  }
  if (optional.length > 0) requests.push(loadOwnedPage(address, optional, randomizer.pickIndex(optionalTotal)))
  await Promise.all(requests)
}

// A category missing from the page is left to the base wearables.
async function loadOwnedPage(address: string, categories: string[], index: number): Promise<void> {
  const page = await fetchWearablesPage(address, {
    page: Math.floor(index / OWNED_PAGE_SIZE),
    pageSize: OWNED_PAGE_SIZE,
    categories,
    collectionTypes: OWNED_COLLECTIONS
  })
  for (const category of categories) randomizer.addOwnedPage(category, page.items, index % OWNED_PAGE_SIZE)
}

export function registerRandomizer(ctx: Ctx): void {
  const emit = (): void => {
    ctx.send({ kind: 'outfitHistory', canUndo: undoStack.length > 0, canRedo: redoStack.length > 0, rolling })
  }

  // Puts a whole look back, emotes aside: the wheel is edited separately.
  const restore = async (look: AvatarLook): Promise<void> => {
    editLook({ bodyShape: look.bodyShape, eyes: look.eyes, hair: look.hair, skin: look.skin, wearables: look.wearables, forceRender: look.forceRender })
    await sendEquipped(ctx)
  }

  ctx.on('randomizeOutfit', async () => {
    const me = getPlayer()
    const start = currentLook()
    if (rolling || me == null || start == null) return
    const mine = session
    rolling = true
    emit()
    try {
      const base = baseItems ?? (await loadBaseItems(me.userId))
      if (mine !== session) return
      if (base.length === 0) {
        console.log('[randomize] base wearables failed to load, the outfit is kept')
        return
      }
      baseItems = base
      const totals = ownedTotals ?? (await loadOwnedTotals(me.userId))
      if (mine !== session) return
      ownedTotals = totals

      randomizer.buildCatalog(base, start.bodyShape)
      const chosen = randomizer.chooseCategories(totals)
      await loadOwnedPages(me.userId, chosen, totals)
      if (mine !== session) return
      const outfit = randomizer.roll(chosen)

      // The Backpack stays usable while the roll loads, so the look is read again to keep edits made meanwhile
      const latest = currentLook()
      // The picks were filtered for the old body shape
      if (latest == null || latest.bodyShape !== start.bodyShape) return
      if (outfit.length === 0) {
        console.log('[randomize] no wearables rolled, the outfit is kept')
        return
      }
      undoStack.push(latest)
      redoStack.length = 0
      editLook({ wearables: outfit.map((item) => deployableUrn(item.urn)), forceRender: [] })
      await sendEquipped(ctx)
    } catch (e) {
      console.error('[randomize] roll failed', e)
    } finally {
      if (mine === session) {
        rolling = false
        emit()
      }
    }
  })

  const step = async (from: AvatarLook[], to: AvatarLook[]): Promise<void> => {
    const current = currentLook()
    if (rolling || current == null) return
    const target = from.pop()
    if (target == null) return
    to.push(current)
    await restore(target)
    emit()
  }

  ctx.on('undoOutfit', async () => {
    await step(undoStack, redoStack)
  })

  ctx.on('redoOutfit', async () => {
    await step(redoStack, undoStack)
  })

  // The Backpack closed: the next session starts clean and sees new purchases.
  ctx.on('resetOutfitHistory', () => {
    reset()
    emit()
  })
}
