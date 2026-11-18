// explorer-automation `MinimapTests` (5): the minimap widget beside the rail, which the rail's Map
// button shows and hides.
//
// | Unity test                    | Here                                                     | Class   |
// | TestMinimapHudPresence        | the square map and the parcel the player stands on       | Partial |
// | TestToggleFavoriteFromMinimap | Favorite marks the place under the player, then unmarks  | Port    |
// | TestOpenMinimapContextMenu    | the dots menu has Set as Home, Copy Link, Reload Scene   | Port    |
// | TestCollapseAndExpandMinimap  | Collapse minimap and Expand minimap swap                 | Port    |
// | TestOpenMapFromMinimap        | Open map opens the Map page                              | Port    |
//
// Partial:
// - TestMinimapHudPresence reads the place name and the "x,y" label; both are spans with no role,
//   so the HUD matcher cannot reach them, and neither is checked. What is checked is the widget
//   expanded: the map render (the `Open map` button, actionable), its zoom and settings controls,
//   and that the player has a position for it to centre on (`/world_state`); the parcel that
//   position is in (floor(x / 16), floor(z / 16)) is reported as a diagnostic, not asserted. The
//   compass "N" is aria-hidden and cannot be addressed, so it is not checked.
//
// The minimap shows only while the rail's Map button is selected (it reads pressed then); the HUD
// remembers that choice and starts with the minimap hidden. The HUD also remembers whether the
// widget is expanded per realm type and opens it collapsed in a World. So each scenario reads both
// first, shows the minimap and expands it when it needs the map, and puts both back as it found
// them. A failed scenario leaves them as they were when the failure came.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { PAGES, PAGE_OPEN_TIMEOUT_MS, closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const PARCEL_METERS = 16

/** The rail button that shows and hides the minimap; pressed while it shows. */
const MAP_BUTTON: Selector = { name: 'Map', role: 'button' }
const COLLAPSE: Selector = { name: 'Collapse minimap', role: 'button' }
const EXPAND: Selector = { name: 'Expand minimap', role: 'button' }
const OPEN_MAP: Selector = { name: 'Open map', role: 'button' }
/** The widget's other controls, shown only while it is expanded. */
const WIDGET_CONTROLS = [
  { name: 'Zoom in', role: 'button' },
  { name: 'Zoom out', role: 'button' },
  { name: 'Minimap settings', role: 'button' }
] satisfies Selector[]

const SETTINGS: Selector = { name: 'Minimap settings', role: 'button' }
/** The heart for the place the player stands in; pressed while it is a favourite. */
const FAVORITE: Selector = { name: 'Favorite', role: 'button' }
const MENU: Selector = { name: 'Minimap settings', role: 'menu' }
/** The menu's first entries, in Genesis City where the place has a link. */
const MENU_ENTRIES = [
  { name: 'Set as Home', role: 'switch' },
  { name: 'Copy Link', role: 'menuitem' },
  { name: 'Reload Scene', role: 'menuitem' }
] satisfies Selector[]

/** Click once the control is one a user could click, as explorer-automation's Click waits for it. */
async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

/** Show or hide the minimap with the rail's Map button; answers whether it was shown before. */
async function setShown(explorer: Explorer, wanted: boolean): Promise<boolean> {
  await explorer.hud.wait(MAP_BUTTON, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  const shown = (await explorer.hud.get(MAP_BUTTON)).checked === true
  if (shown !== wanted) {
    await explorer.hud.click(MAP_BUTTON)
    await until(async () => (await explorer.hud.get(MAP_BUTTON)).checked === true, (now) => now === wanted, {
      timeoutMs: CONTROL_TIMEOUT_MS,
      what: `the rail's Map button to read ${wanted ? 'pressed' : 'not pressed'}`
    })
  }
  return shown
}

/** Whether the minimap is expanded. The widget shows exactly one of the two chevrons' names. */
async function isExpanded(explorer: Explorer): Promise<boolean> {
  const expanded = await until<boolean | undefined>(
    async () => {
      if (await explorer.hud.isVisible(COLLAPSE)) return true
      if (await explorer.hud.isVisible(EXPAND)) return false
      return undefined
    },
    (state) => state !== undefined,
    { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the minimap widget (Collapse minimap or Expand minimap)' }
  )
  return expanded === true
}

async function setExpanded(explorer: Explorer, wanted: boolean): Promise<void> {
  if ((await isExpanded(explorer)) === wanted) return
  await press(explorer, wanted ? EXPAND : COLLAPSE)
  await explorer.hud.wait(wanted ? COLLAPSE : EXPAND, { timeoutMs: CONTROL_TIMEOUT_MS })
}

scenario('the minimap shows the square map and the parcel the player stands on', async ({ explorer, t }) => {
  const wasShown = await setShown(explorer, true)
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await explorer.hud.wait(OPEN_MAP, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  for (const control of WIDGET_CONTROLS) {
    assert.ok(await explorer.hud.isVisible(control), `the expanded minimap should show its ${control.name} control`)
  }
  const { position } = await explorer.worldState()
  assert.ok(position, 'the player should have a position for the minimap to centre on')
  t.diagnostic(`the parcel label is not addressable; the player stands in ${Math.floor(position[0] / PARCEL_METERS)}, ${Math.floor(position[2] / PARCEL_METERS)}`)
  await setExpanded(explorer, wasExpanded)
  await setShown(explorer, wasShown)
})

scenario('the favourite heart toggles the current place and restores it', async ({ explorer }) => {
  const wasShown = await setShown(explorer, true)
  await explorer.hud.wait(FAVORITE, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  const before = (await explorer.hud.get(FAVORITE)).checked === true
  for (const wanted of [!before, before]) {
    await explorer.hud.click(FAVORITE)
    await until(async () => (await explorer.hud.get(FAVORITE)).checked === true, (now) => now === wanted, {
      timeoutMs: CONTROL_TIMEOUT_MS,
      what: `the minimap's Favorite to read ${wanted ? 'pressed' : 'not pressed'}`
    })
  }
  await setShown(explorer, wasShown)
})

scenario('the kebab menu offers Set as Home, Copy Link and Reload Scene, and Escape closes it', async ({ explorer }) => {
  const wasShown = await setShown(explorer, true)
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await press(explorer, SETTINGS)
  await explorer.hud.wait(MENU, { timeoutMs: CONTROL_TIMEOUT_MS })
  for (const entry of MENU_ENTRIES) {
    assert.ok(await explorer.hud.isVisible(entry), `the minimap menu should offer ${entry.name}`)
  }
  await explorer.escape()
  await explorer.hud.waitHidden(MENU, CONTROL_TIMEOUT_MS)
  await setExpanded(explorer, wasExpanded)
  await setShown(explorer, wasShown)
})

scenario('the collapse and expand chevrons swap', async ({ explorer }) => {
  const wasShown = await setShown(explorer, true)
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await press(explorer, COLLAPSE)
  await explorer.hud.wait(EXPAND, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal(await explorer.hud.isVisible(COLLAPSE), false, 'only one of the pair should show: Expand minimap is up')
  await press(explorer, EXPAND)
  await explorer.hud.wait(COLLAPSE, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal(await explorer.hud.isVisible(EXPAND), false, 'only one of the pair should show: Collapse minimap is up')
  await setExpanded(explorer, wasExpanded)
  await setShown(explorer, wasShown)
})

scenario('the map opens the Map page', async ({ explorer }) => {
  const wasShown = await setShown(explorer, true)
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await press(explorer, OPEN_MAP)
  await explorer.hud.wait(PAGES.map.marker, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
  await closePage(explorer, 'map')
  await setExpanded(explorer, wasExpanded)
  await setShown(explorer, wasShown)
})
