// explorer-automation `PlacesTests` (5): the Places page's tabs, search, category filter and
// dropdown. The Bevy page differs: the tabs are Explore all, Favourites and My places (no Recent);
// there is no results counter and no search clear button; the category and the sort are dropdowns
// whose trigger is named by its current value; a card jumps in and closes the page, so there is no
// detail popup. Cards are buttons named by their whole text, ending in the decorative "Jump in";
// the scenarios compare their titles, and none clicks a card. The page forgets its state when it
// closes, so each scenario starts from Explore all, Any Category and Most active.
//
// | Unity test                    | Here                                                                   | Class   |
// | TestSwitchPlacesTabs          | Explore all, Favourites, My places: the cards and the Live Now and Featured Places sections change instead of a "(N)" counter | Partial |
// | TestSearchPlaces              | the query is a word of the first card read, not "Genesis Plaza"; the sections hiding and returning stand in for the counter; an emptied field stands in for the clear button | Partial |
// | TestOpenPlaceDetail           | a card jumps in; there is no detail popup                              | Gap     |
// | TestFilterPlacesByCategory    | the Art entry of the category dropdown, not chip 3; the trigger's name shows the choice; the card list differs, not the first card | Partial |
// | TestOpenPlacesFilterDropdown  | the sort dropdown (Most active, Most liked, Newest, Name), not Trending, Most Active and Compatible Only | Partial |
import assert from 'node:assert/strict'
import { settled, spec, until, type Explorer } from '../driver/index.ts'
import { PAGES, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const TEXT_TIMEOUT_MS = 10_000
/** The places come from the public places API; explorer-automation waits 40 s for the grid. */
const GRID_TIMEOUT_MS = 40_000

const SEARCH = { name: 'Search places', role: 'textbox' } as const
const LOADING = { name: 'Loading', role: 'status' } as const
/** The two sections the page shows only on Explore all with no search and no category. */
const HIGHLIGHTS = [
  { name: 'Live Now', role: 'heading' },
  { name: 'Featured Places', role: 'heading' }
] as const
const CATEGORY = 'Art'
const ANY_CATEGORY = 'Any Category'
const SORT = 'Most active'
const SORT_OPTIONS = [SORT, 'Most liked', 'Newest', 'Name'] as const

/** A card's title. Its name is `[LIVE n] [Featured] <title> [By <creator>] <coordinates or world>
 *  Jump in`, and the live count and the creator arrive after the card does, so lists of names
 *  would change while nothing is reloading. */
function titleOf(cardName: string): string {
  return cardName
    .replace(/^LIVE\s+\d+\s+/, '')
    .replace(/^Featured\s+/, '')
    .replace(/\s+Jump in$/, '')
    .split(/\s+By\s+/)[0]!
    .replace(/\s+(?:-?\d+\s*,\s*-?\d+|\S+\.eth)$/i, '')
    .trim()
}

/** The titles of the place cards on the page, in order: each card's name ends in the "Jump in" tag. */
async function placeCards(explorer: Explorer): Promise<string[]> {
  const buttons = await explorer.hud.list('button')
  return buttons.map((button) => button.name).filter((name) => name.endsWith('Jump in')).map(titleOf)
}

function sameList(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((name, index) => name === b[index])
}

/** The first read with a card in it: the page has loaded places. */
function waitForCards(explorer: Explorer, what: string): Promise<string[]> {
  return until(() => placeCards(explorer), (cards) => cards.length > 0, { timeoutMs: GRID_TIMEOUT_MS, what })
}

/** The cards after a reload: the spinner gone, then two reads agreeing. */
async function loadedCards(explorer: Explorer): Promise<string[]> {
  await until(() => explorer.hud.isVisible(LOADING), (loading) => !loading, { timeoutMs: GRID_TIMEOUT_MS, what: 'the places list to finish loading' })
  return settled(() => placeCards(explorer), { timeoutMs: TEXT_TIMEOUT_MS, intervalMs: 500, what: 'the place cards to stop changing' })
}

async function highlightsShown(explorer: Explorer): Promise<boolean> {
  for (const heading of HIGHLIGHTS) {
    if (await explorer.hud.isVisible(heading)) return true
  }
  return false
}

async function waitHighlightsHidden(explorer: Explorer): Promise<void> {
  for (const heading of HIGHLIGHTS) await explorer.hud.waitHidden(heading, TEXT_TIMEOUT_MS)
}

function waitHighlightsBack(explorer: Explorer): Promise<boolean> {
  return until(() => highlightsShown(explorer), (shown) => shown, { timeoutMs: TEXT_TIMEOUT_MS, what: 'the Live Now or Featured Places section to return' })
}

/** The longest word of a title, to search for; the whole title when it has none. */
function searchWordOf(title: string): string {
  const words = title.split(/\s+/).filter((word) => /^[\p{L}\p{N}]{3,}$/u.test(word))
  return words.sort((a, b) => b.length - a.length)[0] ?? title
}

/** Escape leaves a focused field first; the next one closes the page. */
async function leavePlaces(explorer: Explorer): Promise<void> {
  await explorer.escape()
  await closePage(explorer, 'places')
}

scenario('the Explore all, Favourites and My places tabs switch the results', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const explore = await waitForCards(explorer, 'the Explore all places to load')
  const highlights = await highlightsShown(explorer)

  for (const tab of ['Favourites', 'My places']) {
    await explorer.hud.click({ name: tab, role: 'tab' })
    const cards = await loadedCards(explorer)
    assert.ok(!sameList(cards, explore), `${tab} should list other places than Explore all`)
    if (highlights) await waitHighlightsHidden(explorer)
  }

  await explorer.hud.click({ name: 'Explore all', role: 'tab' })
  await waitForCards(explorer, 'the Explore all places to return')
  if (highlights) await waitHighlightsBack(explorer)
  await closePage(explorer, 'places')
})

scenario('searching places filters the cards and emptying the field restores them', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const before = await waitForCards(explorer, 'the Explore all places to load')
  const highlights = await highlightsShown(explorer)
  const query = searchWordOf(before[0]!)
  assert.ok(query.length > 0, `no search word in the first card "${before[0]}"`)

  const typed = await explorer.hud.fill(SEARCH, query)
  assert.equal(typed.value, query, 'the search field should hold the typed query')
  const found = await until(() => placeCards(explorer), (cards) => cards.length > 0 && !sameList(cards, before), { timeoutMs: GRID_TIMEOUT_MS, what: `the places matching "${query}"` })
  assert.ok(
    found.some((name) => name.toLowerCase().includes(query.toLowerCase())),
    `Search should return a place named "${query}", got: ${found.slice(0, 3).join(' | ')}`
  )
  if (highlights) await waitHighlightsHidden(explorer)

  await explorer.hud.fill(SEARCH, '')
  const restored = await until(() => placeCards(explorer), (cards) => cards.length > 0 && !sameList(cards, found), { timeoutMs: GRID_TIMEOUT_MS, what: 'the places after the search was emptied' })
  assert.ok(restored.length > 0, 'Emptying the search should bring the places back')
  if (highlights) await waitHighlightsBack(explorer)
  assert.equal(await explorer.hud.value(SEARCH), '', 'the search field should be empty')
  await leavePlaces(explorer)
})

scenario('a place detail opens from a card', async () => {}, {
  todo: 'a place card on the Bevy Places page jumps in and closes the page; there is no detail popup'
})

scenario('choosing a category reloads the cards and Any Category restores them', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const before = await waitForCards(explorer, 'the Explore all places to load')
  const highlights = await highlightsShown(explorer)

  await explorer.hud.click({ name: ANY_CATEGORY, role: 'button' })
  await explorer.hud.click({ name: CATEGORY, role: 'option' })
  await explorer.hud.wait({ name: CATEGORY, role: 'button' }, { timeoutMs: TEXT_TIMEOUT_MS })
  const category = await loadedCards(explorer)
  assert.ok(!sameList(category, before), `Selecting the '${CATEGORY}' category should reload the results grid with different places`)
  if (highlights) await waitHighlightsHidden(explorer)

  await explorer.hud.click({ name: CATEGORY, role: 'button' })
  await explorer.hud.click({ name: ANY_CATEGORY, role: 'option' })
  await explorer.hud.wait({ name: ANY_CATEGORY, role: 'button' }, { timeoutMs: TEXT_TIMEOUT_MS })
  const restored = await waitForCards(explorer, 'the places after Any Category was chosen')
  assert.ok(restored.length > 0, 'Any Category should bring the places back')
  if (highlights) await waitHighlightsBack(explorer)
  await closePage(explorer, 'places')
})

scenario('the sort dropdown lists its options and Escape closes it', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  await explorer.hud.click({ name: SORT, role: 'button' })
  for (const option of SORT_OPTIONS) {
    await explorer.hud.wait({ name: option, role: 'option' }, { timeoutMs: TEXT_TIMEOUT_MS })
  }

  await explorer.escape()
  for (const option of SORT_OPTIONS) {
    await explorer.hud.waitHidden({ name: option, role: 'option' }, TEXT_TIMEOUT_MS)
  }
  assert.ok(await explorer.hud.isVisible({ name: SORT, role: 'button' }), `Escape should close the list and keep ${SORT}`)
  assert.ok(await explorer.hud.isVisible(PAGES.places.marker), 'Escape should close only the dropdown, not the page')
  await closePage(explorer, 'places')
})
