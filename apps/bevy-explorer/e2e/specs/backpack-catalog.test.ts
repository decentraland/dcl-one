// The QA Automation Suite's WEARABLES cases beyond explorer-automation's BackpackWearablesTests
// (backpack-wearables.test.ts: equip, search, pagination): the category tiles, FILTER & SORT, the
// empty search with its Marketplace link, and the cases the HUD or one client cannot show. Names are
// the HUD's (BackpackPage.tsx): a category tile is a button named by its category, pressed while it
// filters the grid, which then shows a `Clear <category>` chip; an empty grid shows a text with a
// `Marketplace` link (EmptyResults) instead of the breadcrumb. FILTER & SORT opens a menu of
// `menuitemradio` options, checked when chosen: Sort by (Newest, Oldest, Rarest, Less rare,
// Name A-Z, Name Z-A) and View (All Items, Collectibles only, Smart Wearables only).
//
// | Suite case                    | Here                                                                   | Class   |
// | Category Selection            | each of the 18 tiles filters the grid: pressed, then its chip with cards or the empty state | Port    |
// | Filters                       | Name A-Z and Name Z-A reorder the first page; each View option loads    | Partial |
// | Search                        | a search nothing matches empties the grid and links to the Marketplace  | Port    |
// | Wearable Details Panel        | the side panel's name, type and description                             | Gap     |
// | Smart Wearables               | an equipped smart wearable starts its portable experience               | Gap     |
// | Avatar Updates for Other Users| another user sees the new look                                          | Gap     |
//
// Partial, Filters: the suite applies "different filters" and reads the list. Here the sort is
// checked through the first page's first card, which differs between Name A-Z and Name Z-A and comes
// first alphabetically under A-Z; the View options are checked to load (the cards carry no
// collectible or smart flag the console can read: Smart Wearables only may well be empty). The
// sort and the view are put back to Newest and All Items, whatever the outcome.
// Gaps: the detail panel's name, rarity and description are spans with no role (DetailPanel), so
// `/hud_read` cannot name them; the card being selected is backpack-wearables'. A smart wearable is
// equipped as plain clothing in Bevy (Notion K7). Another user's view of the look needs a second
// client in the realm; the suite drives one.
import assert from 'node:assert/strict'
import { spec, until, type Explorer } from '../driver/index.ts'
import { CATALOG, CATEGORIES, GRID_TIMEOUT_MS, normalize, openBackpack, searchFor, wearableCards, type Category } from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const FILTER_BUTTON = { name: 'FILTER & SORT', role: 'button' } as const
const MARKETPLACE = { name: 'Marketplace', role: 'link' } as const
const CONTROL_TIMEOUT_MS = 10_000
const NO_MATCH = 'zzqx automation nothing'

const option = (name: string) => ({ name, role: 'menuitemradio' }) as const

/** The grid answered its newest query: a region out of `aria-busy`. */
async function catalogReady(explorer: Explorer): Promise<void> {
  await explorer.hud.waitReady(CATALOG, GRID_TIMEOUT_MS)
}

/** Choose an option of FILTER & SORT, opening the menu when it is closed and closing it after. */
async function choose(explorer: Explorer, name: string): Promise<void> {
  if (!(await explorer.hud.isVisible(option(name)))) await explorer.hud.click(FILTER_BUTTON)
  await explorer.hud.click(option(name))
  await until(async () => (await explorer.hud.find(option(name)))?.checked ?? true, (checked) => checked, { timeoutMs: CONTROL_TIMEOUT_MS, what: `${name} to read checked` })
  // The button toggles the menu; a menu that closed on the pick is left closed.
  if (await explorer.hud.isVisible(option(name))) await explorer.hud.click(FILTER_BUTTON)
  await catalogReady(explorer)
}

scenario(
  'each category tile filters the grid to its category',
  async ({ explorer, t }) => {
    await openBackpack(explorer, 'wearables')
    const counts: string[] = []
    let previous: Category | undefined
    for (const category of CATEGORIES) {
      const tile = { name: category, role: 'button', index: 0 } as const
      if ((await explorer.hud.get(tile)).checked !== true) await explorer.hud.click(tile)
      await until(async () => (await explorer.hud.get(tile)).checked, (pressed) => pressed === true, { timeoutMs: CONTROL_TIMEOUT_MS, what: `the ${category} tile to read pressed` })
      await catalogReady(explorer)
      // A category with items shows its chip and cards; an empty one the text with the Marketplace link.
      const shown = await until(
        async () => ((await explorer.hud.isVisible({ name: `Clear ${category}`, role: 'button' })) ? 'chip' : (await explorer.hud.isVisible(MARKETPLACE)) ? 'empty' : undefined),
        (state) => state !== undefined,
        { timeoutMs: GRID_TIMEOUT_MS, what: `the ${category} grid to show its chip or the empty state` }
      )
      const cards = shown === 'chip' ? (await wearableCards(explorer)).length : 0
      counts.push(`${category}: ${shown === 'chip' ? `${cards} cards` : 'empty'}`)
      // One category filters at a time: the tile pressed before lets go.
      if (previous) {
        assert.notEqual((await explorer.hud.find({ name: previous, role: 'button', index: 0 }))?.checked, true, `${previous} should not stay pressed once ${category} filters the grid`)
      }
      previous = category
    }
    t.diagnostic(counts.join('; '))
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 420_000 }
)

scenario(
  'Name A-Z and Name Z-A reorder the grid, and every View option loads',
  async ({ explorer, restore, t }) => {
    await openBackpack(explorer, 'wearables')
    restore(async () => {
      if (!(await explorer.hud.isVisible(FILTER_BUTTON))) await openBackpack(explorer, 'wearables')
      await choose(explorer, 'Newest')
      await choose(explorer, 'All Items')
      await closePage(explorer, 'backpack')
    })
    await choose(explorer, 'Name A-Z')
    const ascending = await wearableCards(explorer)
    await choose(explorer, 'Name Z-A')
    const descending = await wearableCards(explorer)
    assert.ok(ascending.length > 0 && descending.length > 0, 'the grid should list wearables under both name sorts')
    if (ascending.length > 1) {
      const [first, last] = [normalize(ascending[0]!.name), normalize(descending[0]!.name)]
      assert.notEqual(first, last, 'Name A-Z and Name Z-A should start the grid with different cards')
      assert.ok(first.localeCompare(last, undefined, { sensitivity: 'base' }) <= 0, `Name A-Z should start before Name Z-A: "${first}" vs "${last}"`)
    }

    const views: string[] = []
    for (const view of ['Collectibles only', 'Smart Wearables only', 'All Items']) {
      await choose(explorer, view)
      views.push(`${view}: ${(await wearableCards(explorer)).length} cards`)
    }
    t.diagnostic(views.join('; '))
  },
  { timeoutMs: 240_000 }
)

scenario('a search nothing matches empties the grid and links to the Marketplace', async ({ explorer, restore }) => {
  await openBackpack(explorer, 'wearables')
  restore(async () => {
    if (await explorer.hud.isVisible({ name: 'Search item', role: 'textbox' })) {
      await searchFor(explorer, '')
      await closePage(explorer, 'backpack')
    }
  })
  await searchFor(explorer, NO_MATCH)
  await explorer.hud.wait(MARKETPLACE, { timeoutMs: GRID_TIMEOUT_MS })
  assert.deepEqual(await wearableCards(explorer), [], `no card should match "${NO_MATCH}"`)
  await searchFor(explorer, '')
  await closePage(explorer, 'backpack')
})

scenario("a selected wearable's side panel shows its name, type and description", async () => {}, {
  todo: "the detail panel's name, rarity and description are spans with no role (BackpackPage DetailPanel), which /hud_read cannot name"
})

scenario('an equipped smart wearable starts its portable experience', async () => {}, {
  todo: 'Bevy equips a smart wearable as plain clothing and starts no portable experience (Notion K7)'
})

scenario("another user sees the player's new look", async () => {}, {
  todo: "needs a second client in the realm to read the player's avatar as another user's; the suite drives one"
})
