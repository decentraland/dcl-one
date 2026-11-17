// explorer-automation `PlacesTests` (5): the Places page's tabs, search, category filter and
// dropdown. The page as these scenarios drive it: the tabs are Explore, Recent, Favorites and My
// Places; there is no results counter and no search clear button; the categories are chips, named
// in lowercase and pressed while chosen; Filter & Sort opens the sort list. A card opens the
// destination card. Cards are buttons named by their whole text, which carries the rating ("97%");
// the scenarios compare their names without the people count, and none clicks a card. The page
// forgets its state when it closes, so each scenario starts from Explore, all and Most active.
//
// | explorer-automation test      | Here                                                                  | Class   |
// | TestSwitchPlacesTabs          | Explore, Favorites, My Places: the cards change instead of a "(N)" counter | Partial |
// | TestSearchPlaces              | the query is a word of the first card read, not "Genesis Plaza"; an emptied field stands in for the clear button | Partial |
// | TestOpenPlaceDetail           | not written yet: a card opens the destination card                     | Gap     |
// | TestFilterPlacesByCategory    | the art chip, not chip 3; the card list differs, not the first card     | Partial |
// | TestOpenPlacesFilterDropdown  | the Filter & Sort list (Most active, Most liked, Newest, Name), not Trending, Most Active and Compatible Only | Partial |
import assert from 'node:assert/strict'
import { spec, type Explorer } from '../driver/index.ts'
import { PAGES, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const TEXT_TIMEOUT_MS = 10_000
/** The places come from the public places API; explorer-automation waits 40 s for the grid. */
const GRID_TIMEOUT_MS = 40_000

const SEARCH = { name: 'Search places', role: 'searchbox' } as const
/** The results panel, loading (`aria-busy`) from the moment the query changes (a tab, the
 *  category, the search box's debounce) until its answer is shown. */
const RESULTS = { name: 'Places results', role: 'region' } as const
const CATEGORY = 'art'
const ANY_CATEGORY = 'all'
const SORT = { name: 'Filter & Sort', role: 'button' } as const
const SORT_OPTIONS = ['Most active', 'Most liked', 'Newest', 'Name'] as const

/** A card without its people count. Its name is `[n] [Featured] <title> [<creator>] <rating>%
 *  [<coordinates>]`, and the count changes while nothing is reloading. */
function titleOf(cardName: string): string {
  return cardName.replace(/^\d+\s+/, '').trim()
}

/** The place cards on the page, in order: only a card's name carries a rating. */
async function placeCards(explorer: Explorer): Promise<string[]> {
  const buttons = await explorer.hud.list('button')
  return buttons.map((button) => button.name).filter((name) => /\b\d+%(?:\s|$)/.test(name)).map(titleOf)
}

function sameList(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((name, index) => name === b[index])
}

/** The cards that answer the page's current query, once the results panel has them. */
async function loadedCards(explorer: Explorer): Promise<string[]> {
  await explorer.hud.waitReady(RESULTS, GRID_TIMEOUT_MS)
  return placeCards(explorer)
}

/** The current query's cards, which must be some. */
async function waitForCards(explorer: Explorer, what: string): Promise<string[]> {
  const cards = await loadedCards(explorer)
  if (cards.length === 0) throw new Error(`no place cards once ${what}`)
  return cards
}

/** The first word of a card worth searching for (its title leads, after the Featured ribbon); the
 *  whole name when it has none. */
function searchWordOf(card: string): string {
  const words = card.replace(/^Featured\s+/, '').split(/\s+/)
  return words.find((word) => /^[\p{L}\p{N}]{3,}$/u.test(word)) ?? card
}

scenario('the Explore, Favorites and My Places tabs switch the results', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const explore = await waitForCards(explorer, 'the Explore places to load')

  for (const tab of ['Favorites', 'My Places']) {
    await explorer.hud.click({ name: tab, role: 'tab' })
    const cards = await loadedCards(explorer)
    assert.ok(!sameList(cards, explore), `${tab} should list other places than Explore`)
  }

  await explorer.hud.click({ name: 'Explore', role: 'tab' })
  await waitForCards(explorer, 'the Explore places to return')
  await closePage(explorer, 'places')
})

scenario('searching places filters the cards and emptying the field restores them', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const before = await waitForCards(explorer, 'the Explore places to load')
  const query = searchWordOf(before[0]!)
  assert.ok(query.length > 0, `no search word in the first card "${before[0]}"`)

  const typed = await explorer.hud.fill(SEARCH, query)
  assert.equal(typed.value, query, 'the search field should hold the typed query')
  const found = await loadedCards(explorer)
  assert.ok(found.length > 0 && !sameList(found, before), `Search for "${query}" should list other places than Explore`)
  assert.ok(
    found.some((name) => name.toLowerCase().includes(query.toLowerCase())),
    `Search should return a place named "${query}", got: ${found.slice(0, 3).join(' | ')}`
  )

  await explorer.hud.fill(SEARCH, '')
  const restored = await loadedCards(explorer)
  assert.ok(restored.length > 0 && !sameList(restored, found), 'Emptying the search should bring the places back')
  assert.equal(await explorer.hud.value(SEARCH), '', 'the search field should be empty')
  await closePage(explorer, 'places')
})

scenario('a place detail opens from a card', async () => {}, {
  todo: 'not written yet: a place card opens the destination card'
})

scenario('choosing a category reloads the cards and all restores them', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  const before = await waitForCards(explorer, 'the Explore places to load')

  await explorer.hud.click({ name: CATEGORY, role: 'button' })
  const category = await loadedCards(explorer)
  assert.ok(!sameList(category, before), `Selecting the '${CATEGORY}' category should reload the results grid with different places`)

  await explorer.hud.click({ name: ANY_CATEGORY, role: 'button' })
  const restored = await waitForCards(explorer, 'the places after the all chip was chosen')
  assert.ok(!sameList(restored, category), 'The all chip should bring the places back')
  await closePage(explorer, 'places')
})

scenario('Filter & Sort lists the sort options and Escape closes the list', async ({ explorer }) => {
  await openWithKey(explorer, 'places')
  await explorer.hud.click(SORT)
  for (const option of SORT_OPTIONS) {
    await explorer.hud.wait({ name: option, role: 'option' }, { timeoutMs: TEXT_TIMEOUT_MS })
  }

  await explorer.escape()
  for (const option of SORT_OPTIONS) {
    await explorer.hud.waitHidden({ name: option, role: 'option' }, TEXT_TIMEOUT_MS)
  }
  assert.ok(await explorer.hud.isVisible(SORT), 'Escape should close the list and keep Filter & Sort')
  assert.ok(await explorer.hud.isVisible(PAGES.places.marker), 'Escape should close only the dropdown, not the page')
  await closePage(explorer, 'places')
})
