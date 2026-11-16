// explorer-automation `NavbarTests` (4): the sidebar rail and the menus behind its buttons.
//
// | Unity test                           | Here                                                         | Class   |
// | TestSidebarShowsAllNavigationButtons | the rail's fixed buttons are visible; Me, Discover and System list the rest | Partial |
// | TestOpenAndCloseNotificationsPanel   | the bell opens and closes the notifications panel            | Partial |
// | TestOpenHelpMenu                     | System opens a panel with the help rows, Escape closes it    | Partial |
// | TestOpenControlsPanelFromHelpMenu    | the System panel has no controls row                         | Gap     |
// | (none)                               | on the web, the Me panel's Sign Out row asks first; Cancel and Escape keep the session | New |
// | (none)                               | on the web, the confirmed Sign Out ends the engine session   | New     |
//
// Partial:
// - TestSidebarShowsAllNavigationButtons reads the client's feature flags; the Bevy HUD reads none,
//   so the expectation is the rail's fixed set of buttons. Unity's Events, Places, Communities,
//   Backpack, Gallery, Settings and profile entries are rows of the panels the rail's Discover, Me,
//   Camera and System buttons open; each panel is opened and its rows read. Presence only: Portable
//   experiences shows only while one is listed, and Create (a System row) in editor builds only,
//   so neither is required nor forbidden. Unity logs that Map is absent from the rail by design;
//   here Map is a rail button, which shows and hides the minimap. "Exactly these buttons" is not
//   checked: a list cannot be scoped to the rail's `navigation` landmark.
// - TestOpenAndCloseNotificationsPanel reads the panel title ("NOTIFICATIONS"); the title is a
//   span with no role, so it cannot be read. The panel is recognised by its `Close notifications`
//   button, and the bell reports `aria-pressed` while it is open.
// - TestOpenHelpMenu expects Mouse and Key Controls, FAQ, Contact Support and Discord. The System
//   panel has FAQ, Report Bug, Contact Support and Discord under its Settings row, and no controls
//   row. The rows are read, not pressed: each opens a browser tab.
import assert from 'node:assert/strict'
import { readTarget, spec, until, type Explorer, type OpenOptions, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)
const target = readTarget()

const CONTROL_TIMEOUT_MS = 20_000
const PRESENT_TIMEOUT_MS = 10_000

/** The rail's buttons in every build, top to bottom, by accessible name; the Unity entry each
 *  stands for is beside it. */
const RAIL_BUTTONS = [
  'Sidebar settings', // Sidebar config
  'Me', // My profile
  'Map',
  'Discover',
  'Shop', // Marketplace
  'System', // Help
  'Notifications',
  'Scene permissions',
  'Skybox',
  'Volume',
  'Emotes',
  'Friends',
  'Camera',
  'Nearby Voice', // Nearby voice chat
  'Chat'
]

/** The rail buttons that open a list of rows, and the rows every build has. */
const RAIL_ROWS: Array<{ button: string; rows: string[] }> = [
  { button: 'Me', rows: ['Backpack', 'Profile'] },
  { button: 'Camera', rows: ['Take A Shot', 'Gallery'] },
  { button: 'Discover', rows: ['Events', 'Places', 'Communities'] },
  { button: 'System', rows: ['Settings'] }
]

/** The System panel's help rows; Unity's help menu has Mouse and Key Controls in place of Report Bug. */
const HELP_ROWS = ['FAQ', 'Report Bug', 'Contact Support', 'Discord']

const BELL: Selector = { name: 'Notifications', role: 'button' }
const ME: Selector = { name: 'Me', role: 'button' }
const ME_PANEL: Selector = { name: 'Me', role: 'dialog' }
const SIGN_OUT_ROW: Selector = { name: 'Sign Out', role: 'button' }
/** The question the Sign Out row asks: a dialog named as its title, with Cancel and Sign Out. */
const SIGN_OUT_QUESTION: Selector = { name: 'Sign out?', role: 'dialog' }
const NOTIFICATIONS_PANEL: Selector = { name: 'Close notifications', role: 'button' }
/** The Sign Out row is for an account on the web: the standalone and a guest have none. */
const NO_SIGN_OUT = target.transport === 'native' ? 'the standalone: its Me panel has no Sign Out row' : undefined
/** The confirmed sign-out ends its session, so it has one of its own. */
const SIGN_OUT_SESSION: OpenOptions = { signIn: 'fresh', firstLogin: { name: 'E2eSignOut' } }
/** The page loads again and the engine starts again before it can answer. */
const SIGNED_OUT_TIMEOUT_MS = 180_000

/** Click once the control is one a user could click, as explorer-automation's Click waits for it. */
async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

/** The names in `expected` that no visible button has. */
async function missingButtons(explorer: Explorer, expected: string[]): Promise<string[]> {
  const names = (await explorer.hud.list('button')).map((entry) => entry.name.toLowerCase())
  return expected.filter((name) => !names.includes(name.toLowerCase()))
}

/** Open a rail button's panel (a dialog named as the button), wait for `rows`, close it with Escape. */
async function readPanelRows(explorer: Explorer, button: string, rows: string[]): Promise<void> {
  const railButton: Selector = { name: button, role: 'button' }
  const panel: Selector = { name: button, role: 'dialog' }
  await press(explorer, railButton)
  await explorer.hud.wait(panel, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal((await explorer.hud.get(railButton)).checked, true, `${button} should read pressed while its panel is open`)
  await until(() => missingButtons(explorer, rows), (missing) => missing.length === 0, {
    timeoutMs: PRESENT_TIMEOUT_MS,
    what: `every row of the ${button} panel to be visible`
  })
  await explorer.escape()
  await explorer.hud.waitHidden(panel, CONTROL_TIMEOUT_MS)
}

scenario('the sidebar shows its navigation buttons, and its panels the rows behind them', async ({ explorer }) => {
  await explorer.hud.wait({ name: 'Me', role: 'button' }, { timeoutMs: CONTROL_TIMEOUT_MS })
  await until(() => missingButtons(explorer, RAIL_BUTTONS), (missing) => missing.length === 0, {
    timeoutMs: PRESENT_TIMEOUT_MS,
    what: 'every rail button to be visible'
  })
  for (const { button, rows } of RAIL_ROWS) await readPanelRows(explorer, button, rows)
})

scenario('the bell opens and closes the notifications panel', async ({ explorer }) => {
  await press(explorer, BELL)
  await explorer.hud.wait(NOTIFICATIONS_PANEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal((await explorer.hud.get(BELL)).checked, true, 'the bell should read pressed while the panel is open')
  await press(explorer, BELL)
  await explorer.hud.waitHidden(NOTIFICATIONS_PANEL, CONTROL_TIMEOUT_MS)
})

scenario('the System button opens a panel with the help rows, Escape closes it', async ({ explorer }) => {
  await readPanelRows(explorer, 'System', HELP_ROWS)
  assert.equal((await explorer.hud.get({ name: 'System', role: 'button' })).checked, false, 'System should not read pressed once its panel is closed')
})

// Only the refusals: the confirmed sign-out ends the session, so it has a scenario and a session of its own.
scenario("the Me panel's Sign Out row asks first, and Cancel and Escape keep the session", async ({ explorer }) => {
  const { address } = await explorer.userData()
  await press(explorer, ME)
  await explorer.hud.wait(ME_PANEL, { timeoutMs: CONTROL_TIMEOUT_MS })
  await press(explorer, SIGN_OUT_ROW)
  await explorer.hud.wait(SIGN_OUT_QUESTION, { timeoutMs: CONTROL_TIMEOUT_MS })
  await press(explorer, { name: 'Cancel', role: 'button' })
  await explorer.hud.waitHidden(SIGN_OUT_QUESTION, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(ME_PANEL), 'the Me panel should stay open after Cancel')
  await press(explorer, SIGN_OUT_ROW)
  await explorer.hud.wait(SIGN_OUT_QUESTION, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.escape()
  await explorer.hud.waitHidden(SIGN_OUT_QUESTION, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(ME_PANEL), 'Escape should close the question before the Me panel')
  assert.equal((await explorer.userData()).address, address, 'the session should still be signed in')
  await explorer.escape()
  await explorer.hud.waitHidden(ME_PANEL, CONTROL_TIMEOUT_MS)
}, { skip: NO_SIGN_OUT })

// The page loads again without the stored sign-in and without the place in its address, so it
// opens on the start screen. The engine that answers there is a new one, held at the start screen
// with no account; a page that kept the place would start no engine before a sign-in. The session
// is closed after the scenario.
scenario(
  "the Me panel's confirmed Sign Out ends the engine session",
  async ({ explorer }) => {
    assert.equal((await explorer.worldState()).signedIn, true, 'the session should start signed in')
    await press(explorer, ME)
    await explorer.hud.wait(ME_PANEL, { timeoutMs: CONTROL_TIMEOUT_MS })
    await press(explorer, SIGN_OUT_ROW)
    await explorer.hud.wait(SIGN_OUT_QUESTION, { timeoutMs: CONTROL_TIMEOUT_MS })
    // two controls are named Sign Out while the question is open: the row, then the question's button
    await press(explorer, { name: 'Sign Out', role: 'button', index: 1 })
    await until(
      () => explorer.worldState(),
      (state) => !state.signedIn,
      { timeoutMs: SIGNED_OUT_TIMEOUT_MS, what: 'the start screen’s engine to answer signed out after the page loaded again' }
    )
  },
  { session: SIGN_OUT_SESSION, skip: NO_SIGN_OUT, timeoutMs: 300_000, closeSession: true }
)

scenario('Mouse and Key Controls opens the controls panel and its exit button closes it', async () => {}, {
  todo: 'the System panel has no Mouse / Key Controls row and the Bevy HUD has no controls panel; key bindings are a card of the Controls tab of Settings (whose audio tab is labelled Sounds), and the Backpack has a Controls hints popover'
})
