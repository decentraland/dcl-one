// Shared by the Backpack specs (wearables, emotes, outfits, visibility): open the Backpack on a
// view, find the item cards and the emote and outfit slots by their accessible names, equip, and
// take the look to the engine. Names are the HUD's (react-web/src/features/backpack/BackpackPage.tsx,
// design/WearableCard.tsx, design/Pager.tsx).
//
// Two facts about the HUD shape every equip check:
// - A card's EQUIP and UNEQUIP buttons are nested in the card and exist (non-empty box) only while
//   the pointer is on it. `hud.hover` puts the pointer there with a real click, which also selects
//   the card; the button is then picked by where it sits (over the card), never by its name alone.
// - Equipping edits the Backpack's draft look: the preview avatar wears it, and the player's
//   profile does not change. The look is deployed when the Backpack closes (useEngineSession's
//   commitAvatar, bridge-scene avatarDraft.ts), and only if the net look differs from the deployed
//   one. So the engine's profile version and `/avatar_state` grow after the close, not after the
//   press: `commitBackpack` closes the page and waits for the version.
import { TimeoutError, settled, until, type AvatarReport, type Control, type Explorer, type HudEntry, type Selector } from '../driver/index.ts'
import { closePage, openWithKey } from './pages.ts'

/** One grid page: the catalogue's shared budget (the catalogue can still be arriving). */
export const GRID_TIMEOUT_MS = 90_000
/** An equip taking effect in the HUD. */
export const EQUIP_TIMEOUT_MS = 8_000
/** What the engine renders after a look change (the catalogue's thumbnail budget). */
export const THUMBNAIL_TIMEOUT_MS = 30_000
/** The engine taking the look once the Backpack closed: the deploy path resolves the items over the
 *  network before the engine bumps the version. */
export const COMMIT_TIMEOUT_MS = 20_000
/** Items per grid page (BackpackPage's PAGE_SIZE). */
export const PAGE_SIZE = 16

const CONTROL_TIMEOUT_MS = 10_000
const HOVER_TIMEOUT_MS = 3_000
/** The EQUIP button hangs below its card; this is how far under the card's box its centre may be. */
const BELOW_CARD_PX = 80

/** The 18 category tiles, as the HUD names them (`humanize(category)`). */
export const CATEGORIES = [
  'Body Shape',
  'Hair',
  'Eyebrows',
  'Eyes',
  'Mouth',
  'Facial Hair',
  'Upper Body',
  'Hands Wear',
  'Lower Body',
  'Feet',
  'Hat',
  'Eyewear',
  'Earring',
  'Mask',
  'Tiara',
  'Top Head',
  'Helmet',
  'Skin'
] as const
export type Category = (typeof CATEGORIES)[number]

export const SEARCH: Selector = { name: 'Search item', role: 'textbox' }
/** The pager shows only past one page. Its `Pages` landmark is a bare `nav`, which the HUD matcher
 *  gives no role, so the pager is known by its next arrow. */
export const PAGER: Selector = { name: 'Next page', role: 'button' }

export const normalize = (text: string): string => text.replace(/\s+/g, ' ').trim().toLowerCase()

/** A card of the grid: its name is its content, and the selector tells it from controls that share it. */
export interface Card {
  name: string
  selector: Selector
}

export type CardFinder = (explorer: Explorer) => Promise<Card[]>

/** One selector per entry of a `/hud_list` result: its name, and an index among the entries that
 *  share it (case-insensitively, as the HUD matches) when there are several. */
function selectorsOf(entries: HudEntry[]): Selector[] {
  return entries.map((entry, position) => {
    const key = normalize(entry.name)
    const sameName = entries.filter((other) => normalize(other.name) === key)
    if (sameName.length === 1) return { name: entry.name, role: entry.role }
    const index = entries.slice(0, position).filter((other) => normalize(other.name) === key).length
    return { name: entry.name, role: entry.role, index }
  })
}

// --- cards and slots ---------------------------------------------------------------------------

/** Where a page's body starts in a `/hud_list` result: after its top bar, which ends with the Close
 *  button. The top bar's profile chip carries the player's name, which could look like a slot. */
function bodyStart(entries: HudEntry[]): number {
  return entries.findIndex((entry) => entry.name === 'Close') + 1
}

// The wearables grid follows the breadcrumb (All, the category chip and its clear box, the search
// chip and its clear box, the colour button); the pager and the hover buttons are not cards.
const CHIP_END = new RegExp(`^(All|Clear search|Clear (${CATEGORIES.join('|')})|(Skin|Hair|Eye) color)$`)
const NOT_A_CARD = /^(EQUIP|UNEQUIP|Previous page|Next page|\d+)$/

/** The wearable cards on screen, in grid order (top-left first). Empty while the grid loads. */
export async function wearableCards(explorer: Explorer): Promise<Card[]> {
  const entries = await explorer.hud.list('button')
  const selectors = selectorsOf(entries)
  let start = -1
  entries.forEach((entry, i) => {
    if (CHIP_END.test(entry.name)) start = i
  })
  if (start < 0) return []
  return entries.flatMap((entry, i) => (i > start && !NOT_A_CARD.test(entry.name) ? [{ name: entry.name, selector: selectors[i]! }] : []))
}

// An emote slot is `<digit> <emote name or None>`; the ten slots lead the Emotes tab. The cards
// follow the slots and the `Emote <n>` breadcrumb chip (whose icon adds the digit to its name).
const EMOTE_SLOT = /^(\d) (.+)$/
const EMOTE_CHIP = /^(\d+ )?Emote \d+$/

/** The emote cards on screen, in grid order. Empty while the list loads. */
export async function emoteCards(explorer: Explorer): Promise<Card[]> {
  const entries = await explorer.hud.list('button')
  const selectors = selectorsOf(entries)
  const body = bodyStart(entries)
  let slots = 0
  let start = -1
  entries.forEach((entry, i) => {
    if (i < body) return
    if (slots < 10 && EMOTE_SLOT.test(entry.name)) {
      slots += 1
      start = i
    } else if (slots === 10 && EMOTE_CHIP.test(entry.name)) {
      start = i
    }
  })
  if (slots < 10) return []
  return entries.flatMap((entry, i) =>
    i > start && !NOT_A_CARD.test(entry.name) && !entry.name.startsWith('Unequip ') ? [{ name: entry.name, selector: selectors[i]! }] : []
  )
}

export interface EmoteSlot {
  /** The wheel slot, 0 to 9: the profile's emote slot, and the index into `selected.emotes`. */
  slot: number
  /** The slot control's name: `<slot> <emote name>`, or `<slot> None` when empty. */
  label: string
  /** The emote name in the label, `None` when empty. */
  emote: string
  selector: Selector
  /** The slot's own Unequip button, when it holds an emote. */
  unequip?: Selector
}

/** The ten emote slots, in the order the HUD lists them (1 to 9, then 0). */
export async function emoteSlots(explorer: Explorer): Promise<EmoteSlot[]> {
  const entries = await explorer.hud.list('button')
  const selectors = selectorsOf(entries)
  const slots: EmoteSlot[] = []
  const body = bodyStart(entries)
  entries.forEach((entry, i) => {
    if (i < body || slots.length === 10) return
    const match = EMOTE_SLOT.exec(entry.name)
    if (!match) return
    const next = entries[i + 1]
    const unequip = next !== undefined && next.name.startsWith('Unequip ') ? selectors[i + 1] : undefined
    slots.push({ slot: Number(match[1]), label: entry.name, emote: match[2]!, selector: selectors[i]!, unequip })
  })
  return slots
}

export async function emoteSlot(explorer: Explorer, slot: number): Promise<EmoteSlot> {
  const found = (await emoteSlots(explorer)).find((candidate) => candidate.slot === slot)
  if (!found) throw new Error(`the Emotes tab shows no slot ${slot}`)
  return found
}

/** Make a slot the target of the next equip (a click on it), and wait for it to read pressed. */
export async function selectEmoteSlot(explorer: Explorer, slot: number): Promise<EmoteSlot> {
  const target = await emoteSlot(explorer, slot)
  await explorer.hud.click(target.selector)
  await until(() => explorer.hud.get(target.selector), (control) => control.checked === true, {
    timeoutMs: 5_000,
    intervalMs: 250,
    what: `emote slot ${slot} to be the target slot`
  })
  return target
}

/** Wait for the grid to show cards, then for two reads to agree: the catalogue keeps arriving and
 *  re-sorting, and a filter change leaves the old page up until the new one loads. */
export async function waitForCards(explorer: Explorer, find: CardFinder, timeoutMs = GRID_TIMEOUT_MS): Promise<Card[]> {
  await until(() => find(explorer), (cards) => cards.length > 0, { timeoutMs, what: 'the Backpack grid to show items' })
  return settled(() => find(explorer), {
    timeoutMs: 15_000,
    intervalMs: 500,
    what: 'the Backpack grid to stop changing',
    same: (a, b) => a.map((card) => card.name).join('\n') === b.map((card) => card.name).join('\n')
  })
}

export function sameCards(a: Card[], b: Card[]): boolean {
  return a.map((card) => card.name).join('\n') === b.map((card) => card.name).join('\n')
}

// --- hover and the buttons it reveals ---------------------------------------------------------

export interface CardAction {
  label: 'EQUIP' | 'UNEQUIP'
  control: Control
  selector: Selector
}

/** The `labels` button sitting over `over` (a card or slot read before the pointer arrived): visible,
 *  actionable, and centred on the card, whatever else shares its name. */
async function actionOver(explorer: Explorer, over: Control, labels: readonly string[]): Promise<CardAction | undefined> {
  const entries = await explorer.hud.list('button')
  const selectors = selectorsOf(entries)
  for (const [i, entry] of entries.entries()) {
    if (!labels.includes(entry.name)) continue
    const selector = selectors[i]!
    const read = await explorer.hud.read(selector)
    if (!read.found || !read.actionable) continue
    const x = read.x + read.width / 2
    const y = read.y + read.height / 2
    if (x >= over.x && x <= over.x + over.width && y >= over.y && y <= over.y + over.height + BELOW_CARD_PX) {
      return { label: entry.name === 'EQUIP' ? 'EQUIP' : 'UNEQUIP', control: read, selector }
    }
  }
  return undefined
}

export interface Hovered {
  /** The card as read before the pointer arrived. */
  control: Control
  /** The button it offers: undefined for an incompatible item, or an equipped one of a required category. */
  action: CardAction | undefined
}

/** Put the pointer on a card (a real click: it selects the card) and read the button it reveals. */
export async function hoverCard(explorer: Explorer, card: Card, labels: readonly string[] = ['EQUIP', 'UNEQUIP']): Promise<Hovered> {
  await explorer.hud.wait(card.selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  const control = await explorer.hud.hover(card.selector)
  try {
    const action = await until(() => actionOver(explorer, control, labels), (found) => found !== undefined, {
      timeoutMs: HOVER_TIMEOUT_MS,
      intervalMs: 200,
      what: `a ${labels.join(' or ')} button over "${card.name}"`
    })
    return { control, action }
  } catch (error) {
    if (error instanceof TimeoutError) return { control, action: undefined }
    throw error
  }
}

/** Wait for the `label` button over a card whose pointer is still on it (an equip flips EQUIP to UNEQUIP). */
export async function waitForAction(explorer: Explorer, over: Control, label: 'EQUIP' | 'UNEQUIP', timeoutMs: number): Promise<CardAction> {
  const found = await until(() => actionOver(explorer, over, [label]), (action) => action !== undefined, {
    timeoutMs,
    intervalMs: 250,
    what: `the card to offer ${label}`
  })
  if (!found) throw new Error(`the card never offered ${label}`)
  return found
}

// --- the engine's side ------------------------------------------------------------------------

export interface LookStart {
  before: AvatarReport
  version: number
}

/** The engine's view of the local player and its profile version, before a change. */
export async function startLook(explorer: Explorer): Promise<LookStart> {
  return { before: await explorer.myAvatar(), version: await explorer.profileVersion() }
}

/** Close the Backpack, which deploys its look, and wait for the engine's profile version to grow. */
export async function commitBackpack(explorer: Explorer, start: LookStart, what: string): Promise<number> {
  await closePage(explorer, 'backpack')
  return until(() => explorer.profileVersion(), (version) => version > start.version, {
    timeoutMs: COMMIT_TIMEOUT_MS,
    what: `the profile version to grow after the Backpack closed (${what})`
  })
}

export function waitForPlayer(explorer: Explorer, done: (avatar: AvatarReport) => boolean, timeoutMs: number, what: string): Promise<AvatarReport> {
  return until(() => explorer.myAvatar(), done, { timeoutMs, intervalMs: 500, what })
}

/** Urns equal as the engine treats them: case-insensitively, a token urn being its item urn plus `:<id>`. */
export function sameUrn(a: string, b: string): boolean {
  const x = a.toLowerCase()
  const y = b.toLowerCase()
  return x === y || x.startsWith(`${y}:`) || y.startsWith(`${x}:`)
}

/** What `after` selects that `before` did not. */
export function gained(before: readonly string[], after: readonly string[]): string[] {
  return after.filter((urn) => !before.some((known) => sameUrn(known, urn)))
}

/** The urn the engine renders in a category (`hair`, `eyes`, `eyebrows`, `mouth`, `upper_body`, ...). */
export function renderedUrn(avatar: AvatarReport, category: string): string | undefined {
  return avatar.rendered?.wearables.find((wearable) => wearable.category === category)?.urn
}

// --- opening ----------------------------------------------------------------------------------

export type BackpackView = 'wearables' | 'emotes' | 'outfits'

/** Make the search field empty (it persists across closes) and check the typing left it so. */
export async function searchFor(explorer: Explorer, text: string): Promise<void> {
  await explorer.hud.wait(SEARCH, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  const field = await explorer.hud.fill(SEARCH, text)
  if (field.value !== text) throw new Error(`the search field holds "${field.value ?? ''}" after typing "${text}"`)
}

export async function pagerVisible(explorer: Explorer): Promise<boolean> {
  return explorer.hud.isVisible(PAGER)
}

/** Back to the first page when the pager is up (the page persists across closes). */
async function firstPage(explorer: Explorer): Promise<void> {
  if (await pagerVisible(explorer)) await explorer.hud.click({ name: '1', role: 'button' })
}

const OUTFIT_EMPTY = /^Save outfit in slot (\d+)$/
const OUTFIT_FULL = /^Outfit (\d+)$/

/** Saved Outfits slots: slot number to whether an outfit is saved in it. */
export async function outfitSlots(explorer: Explorer): Promise<[number, boolean][]> {
  const slots: [number, boolean][] = []
  for (const entry of await explorer.hud.list('button')) {
    const empty = OUTFIT_EMPTY.exec(entry.name)
    const full = OUTFIT_FULL.exec(entry.name)
    if (empty) slots.push([Number(empty[1]), false])
    else if (full) slots.push([Number(full[1]), true])
  }
  return slots.sort(([a], [b]) => a - b)
}

/** The slots once they have loaded (they show empty until the saved outfits arrive) and stopped changing. */
export async function waitForOutfitSlots(explorer: Explorer, timeoutMs = 20_000): Promise<[number, boolean][]> {
  await until(() => outfitSlots(explorer), (slots) => slots.length >= 5, { timeoutMs, what: 'the Saved Outfits slots' })
  return settled(() => outfitSlots(explorer), { timeoutMs: 10_000, intervalMs: 1_500, what: 'the Saved Outfits slots to stop changing' })
}

/** The Wearables categories with no category filter, no search and the first page, loaded. The
 *  Backpack keeps its filter, search and page across closes, so every spec starts from here. */
export async function showWearableGrid(explorer: Explorer): Promise<void> {
  const { hud } = explorer
  await hud.click({ name: 'Wearables', role: 'tab' })
  await hud.click({ name: 'Categories', role: 'tab' })
  // A search that found nothing hides the breadcrumb, and with it the `All` chip, so empty the search first.
  await searchFor(explorer, '')
  // The `All` chip clears the category and the search together; it shows once there are items or a load.
  await hud.wait({ name: 'All', role: 'button' }, { timeoutMs: GRID_TIMEOUT_MS })
  await hud.click({ name: 'All', role: 'button' })
  await waitForCards(explorer, wearableCards)
  await firstPage(explorer)
  await waitForCards(explorer, wearableCards)
}

/** Open the Backpack by its key and bring one view up: the Wearables categories with no filter or
 *  search, the Emotes tab with no search, or the Saved Outfits, each on its first page and loaded. */
export async function openBackpack(explorer: Explorer, view: BackpackView): Promise<void> {
  const { hud } = explorer
  await openWithKey(explorer, 'backpack')
  if (view === 'wearables') {
    await showWearableGrid(explorer)
  } else if (view === 'emotes') {
    await hud.click({ name: 'Emotes', role: 'tab' })
    await searchFor(explorer, '')
    await waitForCards(explorer, emoteCards)
    await firstPage(explorer)
    await waitForCards(explorer, emoteCards)
  } else {
    await hud.click({ name: 'Wearables', role: 'tab' })
    await hud.click({ name: 'Saved Outfits', role: 'tab' })
    await waitForOutfitSlots(explorer)
  }
}

/** Filter the grid to a category and wait for its page. The tile is a toggle: it is pressed only when off. */
export async function showCategory(explorer: Explorer, category: Category): Promise<Card[]> {
  const tile: Selector = { name: category, role: 'button', index: 0 }
  const control = await explorer.hud.get(tile)
  if (control.checked !== true) {
    const before = await wearableCards(explorer)
    await explorer.hud.click(tile)
    await until(() => wearableCards(explorer), (cards) => cards.length > 0 && !sameCards(cards, before), {
      timeoutMs: GRID_TIMEOUT_MS,
      what: `the grid to show ${category} items`
    })
  }
  await explorer.hud.wait({ name: `Clear ${category}`, role: 'button' }, { timeoutMs: CONTROL_TIMEOUT_MS })
  return waitForCards(explorer, wearableCards)
}

// --- paging -----------------------------------------------------------------------------------

/** The name of the grid's first card once two reads a second apart agree. */
export async function stableFirstCard(explorer: Explorer, find: CardFinder, timeoutMs = 40_000): Promise<string> {
  let previous: string | undefined
  const agreed = await until(
    async () => {
      const first = (await find(explorer))[0]?.name
      const same = first !== undefined && first === previous ? first : undefined
      previous = first
      return same
    },
    (name) => name !== undefined,
    { timeoutMs, intervalMs: 1_000, what: "the grid's first card to stop changing" }
  )
  if (agreed === undefined) throw new Error('the first card never settled')
  return agreed
}

/** Press a pager arrow, give the first card up to 12 s to leave `from`, and return where it settles. */
export async function flipPage(explorer: Explorer, arrow: 'Next page' | 'Previous page', find: CardFinder, from: string): Promise<string> {
  await explorer.hud.click({ name: arrow, role: 'button' })
  try {
    await until(async () => (await find(explorer))[0]?.name, (first) => first !== undefined && first !== from, {
      timeoutMs: 12_000,
      intervalMs: 500,
      what: `the first card to change from "${from}"`
    })
  } catch (error) {
    if (!(error instanceof TimeoutError)) throw error
  }
  return stableFirstCard(explorer, find)
}

// --- equipping --------------------------------------------------------------------------------

/** Press EQUIP on the first card (of the first page) that offers it, and wait for the card to flip to
 *  UNEQUIP. `start` is the engine's look before the press. The Backpack stays open: the look is only
 *  a draft until it closes (see `commitBackpack`). */
export async function equipFirstUnequipped(explorer: Explorer, cards: Card[]): Promise<{ name: string; start: LookStart }> {
  const start = await startLook(explorer)
  for (const card of cards.slice(0, PAGE_SIZE)) {
    const { control, action } = await hoverCard(explorer, card)
    if (!action || action.label !== 'EQUIP') continue
    await explorer.hud.click(action.selector)
    await waitForAction(explorer, control, 'UNEQUIP', EQUIP_TIMEOUT_MS)
    return { name: card.name, start }
  }
  throw new Error(`Every loaded card is equipped or cannot be equipped: ${cards.map((card) => card.name).join(', ')}`)
}

/** Filter to Hair and equip the first unequipped hair (a draft until the Backpack closes). */
export async function equipFirstUnequippedHair(explorer: Explorer): Promise<{ name: string; start: LookStart }> {
  const cards = await showCategory(explorer, 'Hair')
  return equipFirstUnequipped(explorer, cards)
}

// --- restoring --------------------------------------------------------------------------------
// The zone account is shared by every scenario and run: what a scenario equips, it puts back
// (`ScenarioContext.restore`). The HUD names cards, never urns, so what the avatar wore is found
// as the card that offers UNEQUIP before anything is pressed.

export interface HairPick {
  /** The first card of the Hair page that offers EQUIP. */
  card: Card
  /** The card that offers UNEQUIP: the hair the avatar wears, when it wears one. */
  equipped: Card | undefined
  start: LookStart
}

/** Filter to Hair and pick, among the first page, the first card to equip and the one the avatar
 *  wears. Nothing is pressed. A worn hair that is not on the first page is an error here, before
 *  any change: the scenario could not put it back. */
export async function pickHair(explorer: Explorer): Promise<HairPick> {
  const cards = await showCategory(explorer, 'Hair')
  const start = await startLook(explorer)
  let card: Card | undefined
  let equipped: Card | undefined
  for (const candidate of cards.slice(0, PAGE_SIZE)) {
    if (card && equipped) break
    const { action } = await hoverCard(explorer, candidate)
    if (action?.label === 'EQUIP' && !card) card = candidate
    if (action?.label === 'UNEQUIP' && !equipped) equipped = candidate
  }
  if (!card) throw new Error(`Every loaded card is equipped or cannot be equipped: ${cards.map((c) => c.name).join(', ')}`)
  if (!equipped && renderedUrn(start.before, 'hair') !== undefined) {
    throw new Error(`the hair the avatar wears (${renderedUrn(start.before, 'hair')}) is not on the first page of Hair, so the scenario could not put it back`)
  }
  return { card, equipped, start }
}

/** Press EQUIP on a card and wait for it to flip to UNEQUIP. The Backpack stays open: the look is
 *  only a draft until it closes (`commitBackpack`). */
export async function equipCard(explorer: Explorer, card: Card): Promise<Control> {
  const { control, action } = await hoverCard(explorer, card)
  if (!action || action.label !== 'EQUIP') throw new Error(`"${card.name}" offers ${action?.label ?? 'no button'}, not EQUIP`)
  await explorer.hud.click(action.selector)
  await waitForAction(explorer, control, 'UNEQUIP', EQUIP_TIMEOUT_MS)
  return control
}

/** Put the hair back as `pickHair` found it: the picked card off, the worn one on, the look
 *  committed. A look that already is the original (the equip never committed) changes nothing. */
export async function restoreHair(explorer: Explorer, pick: HairPick): Promise<void> {
  await closePage(explorer, 'backpack')
  const version = await explorer.profileVersion()
  await openBackpack(explorer, 'wearables')
  const cards = await showCategory(explorer, 'Hair')
  const byName = (name: string): Card => {
    const found = cards.find((candidate) => normalize(candidate.name) === normalize(name))
    if (!found) throw new Error(`the Hair page no longer lists "${name}", so the hair cannot be put back`)
    return found
  }
  let changed = false
  const picked = await hoverCard(explorer, byName(pick.card.name))
  if (picked.action?.label === 'UNEQUIP') {
    await explorer.hud.click(picked.action.selector)
    await waitForAction(explorer, picked.control, 'EQUIP', EQUIP_TIMEOUT_MS)
    changed = true
  }
  if (pick.equipped) {
    const worn = await hoverCard(explorer, byName(pick.equipped.name))
    if (worn.action?.label === 'EQUIP') {
      await explorer.hud.click(worn.action.selector)
      await waitForAction(explorer, worn.control, 'UNEQUIP', EQUIP_TIMEOUT_MS)
      changed = true
    }
  }
  await closePage(explorer, 'backpack')
  if (changed) {
    await until(() => explorer.profileVersion(), (current) => current > version, {
      timeoutMs: COMMIT_TIMEOUT_MS,
      what: 'the profile version to grow after the hair was put back'
    })
  }
}

export interface EmoteSlotsRecord {
  /** The ten slots as the HUD showed them. */
  slots: EmoteSlot[]
  /** `selected.emotes` as the engine had them. */
  selected: string[]
}

/** The ten slots as the HUD shows them (the Emotes tab open) and as the engine selects them. */
export async function recordEmoteSlots(explorer: Explorer): Promise<EmoteSlotsRecord> {
  return { slots: await emoteSlots(explorer), selected: [...(await explorer.myAvatar()).selected.emotes] }
}

/** Empty a slot with its own Unequip button, when it holds an emote. */
export async function unequipEmoteSlot(explorer: Explorer, slot: number): Promise<void> {
  const current = await selectEmoteSlot(explorer, slot)
  if (!current.unequip) return
  await explorer.hud.click(current.unequip)
  await until(async () => (await emoteSlot(explorer, slot)).emote, (emote) => emote === 'None', {
    timeoutMs: EQUIP_TIMEOUT_MS,
    intervalMs: 250,
    what: `emote slot ${slot} to read empty after its Unequip`
  })
}

/** Empty every slot with its own Unequip button, as Unity's Emotes.UnequipAll does. */
export async function unequipAllEmotes(explorer: Explorer): Promise<void> {
  for (const slot of [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]) await unequipEmoteSlot(explorer, slot)
}

/** Make `slot` the target, press EQUIP on the card and wait for the slot to be named after it.
 *  Returns the card as read before the pointer arrived, so the caller can wait on its buttons. */
export async function equipEmoteIntoSlot(explorer: Explorer, slot: number, card: Card): Promise<Control> {
  await selectEmoteSlot(explorer, slot)
  const { control, action } = await hoverCard(explorer, card)
  if (!action || action.label !== 'EQUIP') throw new Error(`"${card.name}" offers ${action?.label ?? 'no button'}, not EQUIP`)
  await explorer.hud.click(action.selector)
  await until(async () => (await emoteSlot(explorer, slot)).emote, (emote) => normalize(emote) === normalize(card.name), {
    timeoutMs: EQUIP_TIMEOUT_MS,
    intervalMs: 250,
    what: `emote slot ${slot} to be named "${card.name}"`
  })
  return control
}

/** Put the slots back as `recordEmoteSlots` found them: each slot that differs is emptied and,
 *  when it held an emote, refilled from the card of that name (searched for), then the look is
 *  committed and the engine read until it selects the emotes it did. */
export async function restoreEmoteSlots(explorer: Explorer, record: EmoteSlotsRecord): Promise<void> {
  await closePage(explorer, 'backpack')
  await openBackpack(explorer, 'emotes')
  const current = await emoteSlots(explorer)
  const stale = record.slots.filter((slot) => normalize(slot.emote) !== normalize(current.find((now) => now.slot === slot.slot)?.emote ?? 'None'))
  for (const slot of stale) {
    await unequipEmoteSlot(explorer, slot.slot)
    if (slot.emote === 'None') continue
    await searchFor(explorer, slot.emote)
    const card = await until(
      async () => (await emoteCards(explorer)).find((candidate) => normalize(candidate.name) === normalize(slot.emote)),
      (found) => found !== undefined,
      { timeoutMs: 40_000, intervalMs: 500, what: `the emote grid to list "${slot.emote}" for slot ${slot.slot}` }
    )
    await equipEmoteIntoSlot(explorer, slot.slot, card!)
    await searchFor(explorer, '')
    await waitForCards(explorer, emoteCards)
  }
  await closePage(explorer, 'backpack')
  if (stale.length === 0) return
  await waitForPlayer(
    explorer,
    (avatar) => record.selected.every((urn, index) => sameUrn(urn ?? '', avatar.selected.emotes[index] ?? '')),
    COMMIT_TIMEOUT_MS,
    'the engine to select the emotes it did before the scenario'
  )
}
