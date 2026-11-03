// explorer-automation `NavbarTests` (4): the sidebar rail and the menus behind its buttons.
//
// | Unity test                           | Here                                                    | Class   |
// | TestSidebarShowsAllNavigationButtons | the rail's fixed buttons are visible and Map is not     | Partial |
// | TestOpenAndCloseNotificationsPanel   | the bell opens and closes the notifications panel       | Partial |
// | TestOpenHelpMenu                     | Help & Support opens a browser tab, not a menu          | Gap     |
// | TestOpenControlsPanelFromHelpMenu    | no controls panel is reachable from the rail            | Gap     |
//
// Partial:
// - TestSidebarShowsAllNavigationButtons reads the client's feature flags; the Bevy HUD reads none,
//   so the expectation is the rail's fixed set of buttons (hud-accessible-names, "Sidebar rail").
//   Presence only: Unity's Marketplace Credits, Portable Experiences and Camera entries have no
//   button here, so their present-or-absent cases do not exist; Create shows in dev builds only
//   and is neither required nor forbidden. Unity logs that Map is absent from the rail by design;
//   here it is asserted. "Exactly these buttons" is not checked: the `nav` landmark has no role in
//   the HUD matcher, so the list cannot be scoped to the rail.
// - TestOpenAndCloseNotificationsPanel reads the panel title ("NOTIFICATIONS"); the title is a
//   span with no role, so it cannot be read. The panel is recognised by its `Close notifications`
//   button, and the bell reports `aria-pressed` while it is open.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const PRESENT_TIMEOUT_MS = 10_000

/** The rail's buttons in every build, by accessible name; the Unity entry each stands for is beside it. */
const RAIL_BUTTONS = [
  'Sidebar settings', // Sidebar config
  'Profile', // My profile
  'Notifications',
  'Events',
  'Places',
  'Communities',
  'Backpack',
  'Shop', // Marketplace
  'Gallery',
  'Settings',
  'Help & Support', // Help
  'Report a bug',
  'Nearby Voice', // Nearby voice chat
  'Skybox',
  'Scene permissions',
  'Emotes',
  'Friends',
  'Chat'
]

const BELL: Selector = { name: 'Notifications', role: 'button' }
const NOTIFICATIONS_PANEL: Selector = { name: 'Close notifications', role: 'button' }

/** Click once the control is one a user could click, as explorer-automation's Click waits for it. */
async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

scenario('the sidebar shows its navigation buttons', async ({ explorer }) => {
  await explorer.hud.wait({ name: 'Profile', role: 'button' }, { timeoutMs: CONTROL_TIMEOUT_MS })
  const rail = await until(
    async () => {
      const names = (await explorer.hud.list('button')).map((entry) => entry.name.toLowerCase())
      return { missing: RAIL_BUTTONS.filter((name) => !names.includes(name.toLowerCase())), map: names.includes('map') }
    },
    (read) => read.missing.length === 0,
    { timeoutMs: PRESENT_TIMEOUT_MS, what: 'every rail button to be visible' }
  )
  assert.equal(rail.map, false, 'the rail has no Map button: the map opens from the minimap and the M key')
})

scenario('the bell opens and closes the notifications panel', async ({ explorer }) => {
  await press(explorer, BELL)
  await explorer.hud.wait(NOTIFICATIONS_PANEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal((await explorer.hud.get(BELL)).checked, true, 'the bell should read pressed while the panel is open')
  await press(explorer, BELL)
  await explorer.hud.waitHidden(NOTIFICATIONS_PANEL, CONTROL_TIMEOUT_MS)
})

scenario('the Help button opens a menu with the four help entries, Escape closes it', async () => {}, {
  todo: 'the Help & Support rail button opens decentraland.org/help in a new tab: the Bevy HUD has no help menu, so no Mouse and Key Controls, FAQ, Contact Support or Discord entries to check'
})

scenario('Mouse and Key Controls opens the controls panel and its exit button closes it', async () => {}, {
  todo: 'the Bevy HUD has no controls panel behind a help menu; key bindings are the Key Bindings tab of Settings, and the Backpack has a Controls hints popover'
})
