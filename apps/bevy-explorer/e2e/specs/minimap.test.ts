// explorer-automation `MinimapTests` (5): the minimap widget at the top left.
//
// | Unity test                    | Here                                                     | Class   |
// | TestMinimapHudPresence        | the map circle and the parcel the player stands on       | Partial |
// | TestToggleFavoriteFromMinimap | the minimap has no favourite control                     | Gap     |
// | TestOpenMinimapContextMenu    | no kebab menu; the gear opens a different settings menu  | Gap     |
// | TestCollapseAndExpandMinimap  | Collapse minimap and Expand minimap swap                 | Port    |
// | TestOpenMapFromMinimap        | Open map opens the Map page                              | Port    |
//
// Partial:
// - TestMinimapHudPresence reads the place name and the "x, y" label; both are spans with no role,
//   so the HUD matcher cannot reach them, and neither is checked. What is checked is the widget
//   expanded: the map render (the `Open map` button, actionable), its zoom and settings controls,
//   and that the player has a position for it to centre on (`/world_state`); the parcel that
//   position is in (floor(x / 16), floor(z / 16)) is reported as a diagnostic, not asserted. The
//   compass "N" is aria-hidden and cannot be addressed, so it is not checked.
//
// The HUD remembers whether the minimap is open per realm type and opens it collapsed in a World,
// so each scenario reads the state first, expands the minimap when it needs the map, and puts it
// back as it found it. A failed scenario leaves it as it was when the failure came.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { PAGES, PAGE_OPEN_TIMEOUT_MS, closePage } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const PARCEL_METERS = 16

const COLLAPSE: Selector = { name: 'Collapse minimap', role: 'button' }
const EXPAND: Selector = { name: 'Expand minimap', role: 'button' }
const OPEN_MAP: Selector = { name: 'Open map', role: 'button' }
/** The widget's other controls, shown only while it is expanded. */
const WIDGET_CONTROLS = [
  { name: 'Zoom in', role: 'button' },
  { name: 'Zoom out', role: 'button' },
  { name: 'Minimap settings', role: 'button' }
] satisfies Selector[]

/** Click once the control is one a user could click, as explorer-automation's Click waits for it. */
async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
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

scenario('the minimap shows the map circle and the parcel the player stands on', async ({ explorer, t }) => {
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
})

scenario('the favourite heart toggles the current place and restores it', async () => {}, {
  todo: 'the Bevy HUD has no favourite control: the minimap has none, and the Places page lists favourites without a way to add or remove one'
})

scenario('the kebab menu offers Set as Home, Copy Link and Reload Scene, and Escape closes it', async () => {}, {
  todo: 'the Bevy minimap has no kebab menu; its gear opens a settings menu of rotation, map style and markers, which has none of these three entries'
})

scenario('the collapse and expand chevrons swap', async ({ explorer }) => {
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await press(explorer, COLLAPSE)
  await explorer.hud.wait(EXPAND, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal(await explorer.hud.isVisible(COLLAPSE), false, 'only one of the pair should show: Expand minimap is up')
  await press(explorer, EXPAND)
  await explorer.hud.wait(COLLAPSE, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal(await explorer.hud.isVisible(EXPAND), false, 'only one of the pair should show: Collapse minimap is up')
  await setExpanded(explorer, wasExpanded)
})

scenario('the map circle opens the Map page', async ({ explorer }) => {
  const wasExpanded = await isExpanded(explorer)
  await setExpanded(explorer, true)
  await press(explorer, OPEN_MAP)
  await explorer.hud.wait(PAGES.map.marker, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
  await closePage(explorer, 'map')
  await setExpanded(explorer, wasExpanded)
})
