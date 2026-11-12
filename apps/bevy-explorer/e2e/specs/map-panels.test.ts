// The QA Automation Suite's MAP cases beyond explorer-automation's NavmapTests and MinimapTests
// (navmap.test.ts clicks every category chip; minimap.test.ts opens the Map from the minimap): the
// Tab key, a category's side list, the search with no match, and what the Bevy Map lacks. A chip
// other than All opens a side list headed by the category with MOST ACTIVE, BEST RATED and NEWEST
// and its places as cards, closed with its `Close` button (MapPage.tsx CategorySidebar). The search
// field is `Search places & worlds`; results show once it holds two characters and something matches,
// each a button named by the title and its base parcel or world; it spends its first Escape clearing
// itself. The place card and its JUMP IN are teleport.test.ts's.
//
// | Suite case | Here                                                                                 | Class   |
// | View       | Tab opens the Map and Escape closes it (M and the minimap: shortcuts and minimap specs) | Port    |
// | Filters    | a category chip opens its side list with the three sorts and place cards; Close closes it | Partial |
// | Search     | a search no place or world matches lists no result                                   | Partial |
// | Layers     | the Layers modal with its pins, details and map types                                | Gap     |
// | Search     | the place card's like, dislike, favourite, share, Start Navigation, photos and events  | Gap     |
//
// Partial, Filters: the suite reads the scenes each filter pins on the map; the map's pins are not
// addressable, so the side list that comes with them is read, for one chip that lists something.
// Nothing asserts zone's listings as literals. Partial, Search: the found place's card is
// teleport.test.ts's; here the empty search, whose result list is absent.
// Gaps: the Bevy Map has no Layers control and a satellite map only (Notion U6); its place card has
// JUMP IN, the location, the parcel count and the description (MapPage.tsx PlacePanel), none of the
// other controls.
import assert from 'node:assert/strict'
import { spec, until, type Explorer } from '../driver/index.ts'
import { PAGES, PAGE_OPEN_TIMEOUT_MS, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const SEARCH = PAGES.map.marker
const SORTS = ['MOST ACTIVE', 'BEST RATED', 'NEWEST'] as const
const CHIPS = ['Art', 'Social', 'Game', 'Music', 'Fashion'] as const
/** A side-list card: the title, then "created by <owner>" or the stats ("👍 85 %", "👤 3"). */
const CARD = /^(.+?)\s+(?:created by |👍|👤)/u
/** A search result: the title and a base parcel, or the world's name and WORLD. */
const RESULT = /\s-?\d+,-?\d+$|\sWORLD$/
const CONTROL_TIMEOUT_MS = 20_000
/** The search waits out its debounce and the places and worlds queries before it shows results. */
const SEARCH_SETTLE_TIMEOUT_MS = 10_000

async function placeCards(explorer: Explorer): Promise<string[]> {
  return (await explorer.hud.list('button')).map((entry) => entry.name).filter((name) => CARD.test(name))
}

// Tab is bound to the Map beside M (inputs.rs SystemAction::Map).
scenario('Tab opens the Map and Escape closes it', async ({ explorer }) => {
  await explorer.key('Tab')
  await explorer.hud.wait(SEARCH, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
  await closePage(explorer, 'map')
})

scenario("a category chip opens its side list with the three sorts and the category's places", async ({ explorer, t }) => {
  await openWithKey(explorer, 'map')
  for (const chip of CHIPS) {
    await explorer.hud.click({ name: chip, role: 'button', index: 0 })
    for (const sort of SORTS) await explorer.hud.wait({ name: sort, role: 'button' }, { timeoutMs: CONTROL_TIMEOUT_MS })
    const cards = await until(() => placeCards(explorer), (found) => found.length > 0, { timeoutMs: CONTROL_TIMEOUT_MS, what: `places in the ${chip} list` }).catch(() => [])
    if (cards.length === 0) continue
    t.diagnostic(`${chip} lists ${cards.length} places`)
    // The side list's Close is the second on the page: the top bar's comes first.
    const closes = (await explorer.hud.list('button')).filter((entry) => entry.name === 'Close').length
    assert.ok(closes >= 2, 'the side list should have its own Close button beside the top bar')
    await explorer.hud.click({ name: 'Close', role: 'button', index: closes - 1 })
    await explorer.hud.waitHidden({ name: SORTS[0], role: 'button' }, CONTROL_TIMEOUT_MS)
    await closePage(explorer, 'map')
    return
  }
  throw new Error(`none of the ${CHIPS.join(', ')} lists showed a place`)
})

scenario('a search nothing matches lists no result', async ({ explorer }) => {
  await openWithKey(explorer, 'map')
  const query = `zzqx${Date.now().toString(36)}`
  assert.equal((await explorer.hud.fill(SEARCH, query)).value, query, 'the search field should hold the query')
  // Nothing to wait for when nothing matches: the list must stay absent for as long as a match would take.
  const appeared = await until(async () => (await explorer.hud.list('button')).some((entry) => RESULT.test(entry.name)), (found) => found, {
    timeoutMs: SEARCH_SETTLE_TIMEOUT_MS,
    what: 'a search result'
  }).catch(() => false)
  assert.equal(appeared, false, `no place or world should match "${query}"`)
  await closePage(explorer, 'map')
})

scenario('the Layers button opens the pins, details and map types', async () => {}, {
  todo: 'the Bevy Map has no Layers control, and only the satellite map (Notion U6)'
})

scenario("a place card offers like, dislike, favourite, share and Start Navigation, with the place's photos and events", async () => {}, {
  todo: 'the Map place card has JUMP IN, the location, the parcel count and the description only (MapPage.tsx PlacePanel; Notion U6)'
})
