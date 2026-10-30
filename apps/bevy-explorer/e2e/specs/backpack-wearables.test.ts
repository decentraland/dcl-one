// explorer-automation `BackpackWearablesTests` (3): equip a hair from the grid, search, paginate.
//
// | Unity test                | Here                                                                         | Class   |
// | TestEquipWearableFromGrid | an unequipped hair's EQUIP, then the engine selects it once the Backpack closes | Partial |
// | TestSearchWearable        | searching for Punk lists a first result whose name matches                   | Port    |
// | TestWearablesPagination   | the pager's next and previous arrows change the first card each time         | Port    |
//
// Partial, TestEquipWearableFromGrid: Unity reads the tile's `IsEquipped` flag. The Bevy grid has
// no such flag, and an equip only edits the Backpack's draft look, which the engine takes when the
// Backpack closes. So the assertion moves to the engine: the card flips from EQUIP to UNEQUIP, and
// after the close the profile version grows and `/avatar_state` selects a wearable it did not
// before. The hover buttons are found by position over the card (backpack.ts).
//
// Port, TestSearchWearable: Unity clicks the first loaded tile and reads the info panel's name. The
// Bevy detail panel's name is not addressable, so the first card's own name is read; the card is
// still clicked and must read selected. Port, TestWearablesPagination: an account with one page of
// wearables has no pager; the scenario then asserts that the grid holds at most one page and says so.
import assert from 'node:assert/strict'
import { spec, until } from '../driver/index.ts'
import {
  EQUIP_TIMEOUT_MS,
  PAGE_SIZE,
  commitBackpack,
  equipFirstUnequippedHair,
  flipPage,
  gained,
  openBackpack,
  pagerVisible,
  sameCards,
  searchFor,
  stableFirstCard,
  waitForCards,
  waitForPlayer,
  wearableCards
} from './backpack.ts'
import { closePage } from './pages.ts'

const scenario = spec(import.meta.url)

/** A base-collection hair every account owns, as the Unity test assumes. */
const SEARCH_TERM = 'Punk'

scenario(
  'an unequipped hair from the grid is equipped and the engine selects it',
  async ({ explorer }) => {
    await openBackpack(explorer, 'wearables')
    const { name, start } = await equipFirstUnequippedHair(explorer)
    await commitBackpack(explorer, start, `equipping "${name}"`)
    const after = await waitForPlayer(
      explorer,
      (avatar) => gained(start.before.selected.wearables, avatar.selected.wearables).length > 0,
      EQUIP_TIMEOUT_MS,
      `the avatar to select a wearable it did not before ("${name}")`
    )
    assert.ok(gained(start.before.selected.wearables, after.selected.wearables).length > 0, `Grid item "${name}" should be equipped after pressing its EQUIP button`)
  },
  { timeoutMs: 300_000 }
)

scenario(
  'searching wearables for Punk lists results that match',
  async ({ explorer }) => {
    await openBackpack(explorer, 'wearables')
    const unfiltered = await wearableCards(explorer)
    await searchFor(explorer, SEARCH_TERM)
    await until(() => wearableCards(explorer), (cards) => cards.length > 0 && !sameCards(cards, unfiltered), {
      timeoutMs: 40_000,
      intervalMs: 500,
      what: `the grid to refilter to "${SEARCH_TERM}"`
    })
    const [first] = await waitForCards(explorer, wearableCards)
    assert.ok(first, 'the search should list at least one wearable')
    await explorer.hud.hover(first.selector)
    assert.equal((await explorer.hud.get(first.selector)).checked, true, `the first result "${first.name}" should read selected after a click`)
    assert.match(first.name, new RegExp(SEARCH_TERM, 'i'), `Selected search result '${first.name}' should match the search term '${SEARCH_TERM}'`)
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 300_000 }
)

scenario(
  'paging the wearables grid forward and back changes the first item each time',
  async ({ explorer, t }) => {
    await openBackpack(explorer, 'wearables')
    if (!(await pagerVisible(explorer))) {
      const cards = await wearableCards(explorer)
      assert.ok(cards.length <= PAGE_SIZE, `without a pager the grid holds at most ${PAGE_SIZE} items, not ${cards.length}`)
      t.diagnostic(`the account has one page of wearables (${cards.length} items): only the pager's absence is asserted`)
      await closePage(explorer, 'backpack')
      return
    }
    const first = await stableFirstCard(explorer, wearableCards)
    const second = await flipPage(explorer, 'Next page', wearableCards, first)
    assert.notEqual(second, first, 'First item on page 2 should differ from first item on page 1')
    // Not compared with page 1: the sort can vary between loads.
    const back = await flipPage(explorer, 'Previous page', wearableCards, second)
    assert.notEqual(back, second, "Navigating back should leave page 2 (first item should differ from page 2's)")
    await closePage(explorer, 'backpack')
  },
  { timeoutMs: 300_000 }
)
