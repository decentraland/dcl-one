// Randomize outfit (Backpack): rolls a random look from the player's own and the base wearables,
// keeping the body shape and colors, with undo/redo over the looks of the Backpack session. Mirrors
// Unity's RandomizeOutfitController: the requests per roll do not grow with the inventory.
//   from: randomizeOutfit / undoOutfit / redoOutfit / resetOutfitHistory (the page),
//         catalyst GET /explorer/:address/wearables via ./wearables (the base set once per session and
//         body shape, the per-category owned totals once per session, then per roll one page per chosen
//         category)
//   to:   the Backpack's look (./avatarDraft, deployed on close); the equipped set and outfitHistory to the page
import { getPlayer } from '@dcl/sdk/players'
import type { Ctx } from '../bridge'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import { OutfitRandomizer, isCoreCategory, rollCategories, type RollItem } from '@explorer-core/engine/outfitRandomizer'
import { currentLook, editLook } from './avatarDraft'
import { deployableUrn, fetchWearablesPage, sendEquipped } from './wearables'
import { identity } from '../identity'

/** Size of the owned page a roll fetches per category; the rest of the page is the category's repick pool. */
const OWNED_PAGE_SIZE = 30
/** Large enough for every base wearable to arrive in one page. */
const BASE_PAGE_SIZE = 1000
const OWNED_COLLECTIONS = ['on-chain', 'third-party']

const randomizer = new OutfitRandomizer()
// The base set, with the hides of `baseBodyShape`: a new body shape loads it again.
let baseItems: RollItem[] | null = null
let baseBodyShape = ''
// Owned totals per category. A category whose request failed is left out, so the next roll asks again.
const ownedTotals = new Map<string, number>()
const undoStack: AvatarLook[] = []
const redoStack: AvatarLook[] = []
let rolling = false
// Bumped by a reset, so a roll that started before it drops its result.
let session = 0

function reset(): void {
  session += 1
  baseItems = null
  baseBodyShape = ''
  ownedTotals.clear()
  undoStack.length = 0
  redoStack.length = 0
  rolling = false
}

async function loadBaseItems(address: string): Promise<RollItem[]> {
  const page = await fetchWearablesPage(address, { page: 0, pageSize: BASE_PAGE_SIZE, collectionTypes: ['base-wearable'] })
  return page.items
}

// One single-item request per category still without a total, read only for the total it reports. A
// failed request reports none, so that category uses base wearables on this roll and is asked again.
async function loadOwnedTotals(address: string, bodyShape: string): Promise<Array<readonly [string, number]>> {
  const missing = rollCategories(bodyShape).filter((category) => !ownedTotals.has(category))
  const pages = await Promise.all(
    missing.map(async (category) => {
      const page = await fetchWearablesPage(address, {
        page: 0,
        pageSize: 1,
        category,
        collectionTypes: OWNED_COLLECTIONS,
        includeEntities: false
      })
      return [category, page] as const
    })
  )
  return pages.filter(([, page]) => !page.failed).map(([category, page]) => [category, page.total] as const)
}

type OwnedPage = { categories: string[]; items: RollItem[]; offset: number }

// For each chosen category with owned wearables, the page holding a random owned index; the chosen
// optional categories share one request. The item at that index is the category's first custom pick.
async function loadOwnedPages(address: string, chosen: readonly string[]): Promise<OwnedPage[]> {
  const requests: Array<Promise<OwnedPage>> = []
  const optional: string[] = []
  let optionalTotal = 0
  for (const category of chosen) {
    const total = ownedTotals.get(category) ?? 0
    if (total === 0) continue
    if (isCoreCategory(category)) requests.push(loadOwnedPage(address, [category], randomizer.pickIndex(total)))
    else {
      optional.push(category)
      optionalTotal += total
    }
  }
  if (optional.length > 0) requests.push(loadOwnedPage(address, optional, randomizer.pickIndex(optionalTotal)))
  return await Promise.all(requests)
}

async function loadOwnedPage(address: string, categories: string[], index: number): Promise<OwnedPage> {
  const page = await fetchWearablesPage(address, {
    page: Math.floor(index / OWNED_PAGE_SIZE),
    pageSize: OWNED_PAGE_SIZE,
    categories,
    collectionTypes: OWNED_COLLECTIONS
  })
  return { categories, items: page.items, offset: index % OWNED_PAGE_SIZE }
}

export function registerRandomizer(ctx: Ctx): void {
  const emit = (): void => {
    ctx.send({ kind: 'outfitHistory', canUndo: undoStack.length > 0, canRedo: redoStack.length > 0, rolling })
  }

  // Another account: its looks and wearables are not this one's, and a roll in flight no longer unlocks the bar.
  identity.onChange(() => {
    reset()
    emit()
  })

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
      const cached = baseItems != null && baseBodyShape === start.bodyShape ? baseItems : null
      const base = cached ?? (await loadBaseItems(me.userId))
      if (mine !== session) return
      // A fresh set carries the hides of the body shape the look had when it arrived
      if (cached == null && currentLook()?.bodyShape !== start.bodyShape) return
      if (base.length === 0) {
        console.log('[randomize] base wearables failed to load, the outfit is kept')
        return
      }
      baseItems = base
      baseBodyShape = start.bodyShape
      const totals = await loadOwnedTotals(me.userId, start.bodyShape)
      if (mine !== session) return
      for (const [category, total] of totals) ownedTotals.set(category, total)

      randomizer.buildCatalog(base, start.bodyShape)
      const chosen = randomizer.chooseCategories(ownedTotals)
      const pages = await loadOwnedPages(me.userId, chosen)
      // A page that lands after a reset belongs to a closed Backpack session or another account
      if (mine !== session) return
      // A category missing from its page is left to the base wearables
      for (const page of pages) for (const category of page.categories) randomizer.addOwnedPage(category, page.items, page.offset)
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
    try {
      await restore(target)
    } finally {
      // The stacks already moved, so the bar follows them even when the re-send fails
      emit()
    }
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
