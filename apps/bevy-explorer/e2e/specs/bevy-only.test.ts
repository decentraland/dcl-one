// QA checklist, Bevy Only (the cases with no Unity counterpart): the web build's link checks, the
// rail's Scene permissions and Create, the Bevy-only graphics settings, the FPS meter and a realm
// that cannot be reached. Names are the HUD's (library/ui/apps/hud: features/gate,
// features/permissions/PermissionsPopover.tsx, features/editorHost/CreatePage.tsx,
// features/settings, features/debug/FpsMeter.tsx, features/session/TravelError.tsx).
//
// | Checklist case                | Here                                                               | Class   |
// | Untrusted link parameters     | a link with a parameter the app does not know opens the app with   | Partial |
// |                               | the "Unrecognised link parameters" dialog naming it, and OK closes |         |
// | Permissions popover           | the rail's Scene permissions opens the popover and closes it again | Partial |
// | Open Create                   | the rail's Create opens the Create page over the world; Close      | Partial |
// |                               | returns to it                                                      |         |
// | Cel shading / Avatar outline  | each switch flips and the engine takes it (`/debug_settings`)      | Partial |
// | Avatar ghosts                 | the switch flips and the engine takes it                           | Partial |
// | Imposters                     | Distant Scene Rendering goes to Off and the engine takes it        | Partial |
// | Parcel grass                  | Empty Parcel Props goes to another density and the engine takes it | Partial |
// | FPS meter                     | none: the meter is `aria-hidden`                                    | Gap     |
// | Crash modal                   | `/world` to a world that does not exist answers "Can't travel      | Partial |
// |                               | there", OK closes it and the player stays in the realm and parcel  |         |
//
// Partial:
// - Untrusted link parameters: Bevy does not ask before a `position` or `realm` on the link (both
//   are accepted parameters; the destination waits for a sign-in instead). It reports a parameter it
//   does not recognise in a dialog and ignores it, which is what is checked, on a second client
//   opened on the suite's own app url with one parameter added. A link that points the app at
//   other infrastructure (`baseDomain`, `systemScene` not the build's own) is stopped by the
//   untrusted-launch gate instead, which renders in place of the app before the engine (and with
//   it the console the suite drives) comes up, so it cannot be read.
// - Permissions popover: in a scene that has asked for nothing the popover says so; listing and
//   changing a permission needs a scene that asks for one (a fixture scene, not built yet).
// - Open Create: the editor's home renders into the page with no roles or names, so only the page
//   itself is checked (the rail gives way to the menu's top bar, whose Close puts the world back),
//   not that the editor loaded.
// - The graphics settings: the engine taking the value is checked, not how avatars or distant
//   scenes look (that would be the Visual suite's, against reference frames).
// - Crash modal: an unreachable realm is not a crash in Bevy; the HUD refuses the travel in a
//   dialog and keeps the player where they are. The crash modal itself shows only on an engine panic
//   (`?simerror=` simulates one, but only before the engine runs), which a run cannot cause safely.
//
// Every change is put back, from the engine values read before it, whatever the outcome.
import assert from 'node:assert/strict'
import { Explorer, openSession, readTarget, spec, until, WebConsole, type Selector } from '../driver/index.ts'
import { chromeArgs } from '../driver/chrome.ts'
import { CLOSE_CHAT, closeChat, openChat, submitLine } from './chat.ts'
import { PAGES, pressRailRow } from './pages.ts'

const scenario = spec(import.meta.url)

const PAGE_TIMEOUT_MS = 20_000
const CONTROL_TIMEOUT_MS = 10_000
const APPLY_TIMEOUT_MS = 10_000
/** A second client's page up to its start page and the dialog over it. */
const SECOND_CLIENT_TIMEOUT_MS = 90_000
/** The destination check of a world: the worlds server's answer for its name. */
const TRAVEL_CHECK_TIMEOUT_MS = 30_000
const PARCEL_METERS = 16

const RAIL_NAVIGATION: Selector = { name: 'Main navigation', role: 'navigation' }
const SETTINGS_HEADING: Selector = { name: 'Settings', role: 'heading' }
const MENU_CLOSE: Selector = { name: 'Close', role: 'button', index: 0 }

type Values = Record<string, number>

async function engineValues(explorer: Explorer, category: string): Promise<Values> {
  return JSON.parse(await explorer.run(`/debug_settings ${category}`)) as Values
}

function changedKeys(before: Values, after: Values): string[] {
  return Object.keys(after).filter((key) => after[key] !== before[key])
}

async function restoreValues(explorer: Explorer, category: string, before: Values): Promise<void> {
  const now = await engineValues(explorer, category)
  for (const key of changedKeys(before, now)) await explorer.run(`/debug_settings ${category} ${key} ${before[key]}`)
}

async function openGraphics(explorer: Explorer): Promise<void> {
  // the rail's System panel holds the Settings row (#929)
  if (!(await explorer.hud.isVisible(SETTINGS_HEADING))) await pressRailRow(explorer, PAGES.settings.rail!)
  await explorer.hud.wait(SETTINGS_HEADING, { timeoutMs: PAGE_TIMEOUT_MS })
  await explorer.hud.click({ name: 'Graphics', role: 'tab' })
  await explorer.hud.wait({ name: 'Avatar Ghosts', role: 'switch' }, { timeoutMs: CONTROL_TIMEOUT_MS })
}

/** Close a menu page (Settings, Create): Escape, then the top bar's Close should it not take. */
async function closeMenuPage(explorer: Explorer, marker: Selector): Promise<void> {
  if (!(await explorer.hud.isVisible(marker))) return
  await explorer.escape()
  await explorer.hud.waitHidden(marker, 5_000).catch(() => undefined)
  if (!(await explorer.hud.isVisible(marker))) return
  if (await explorer.hud.isVisible(MENU_CLOSE)) await explorer.hud.click(MENU_CLOSE)
  await explorer.hud.waitHidden(marker, PAGE_TIMEOUT_MS)
}

/** One engine value of the Graphics category changes, to `value` when given. */
async function engineTook(explorer: Explorer, before: Values, what: string, value?: number): Promise<void> {
  const after = await until(() => engineValues(explorer, 'Graphics'), (values) => changedKeys(before, values).length > 0, {
    timeoutMs: APPLY_TIMEOUT_MS,
    what: `the engine's Graphics settings to take ${what}`
  })
  const changed = changedKeys(before, after)
  assert.equal(changed.length, 1, `one Graphics setting should change, ${what}: ${changed.join(', ')} did`)
  if (value !== undefined) assert.equal(after[changed[0]!], value, `the engine's ${what} should be ${value}`)
}

scenario('a link with an unrecognised parameter opens the app and names it in a dialog', async () => {
  const session = await openSession()
  assert.ok(session.url, 'the suite should serve the web app')
  const target = readTarget()
  const link = new URL(session.url)
  link.searchParams.set('e2eUnknownParam', '1')
  const second = new Explorer(
    await WebConsole.launch({ url: link.toString(), chrome: target.chrome, headless: !target.headed, args: chromeArgs(), windowSize: [1280, 720] })
  )
  try {
    const dialog: Selector = { name: 'Unrecognised link parameters', role: 'alertdialog' }
    // the console comes up with the engine, after the page: until then a /hud_* has nothing to answer
    const read = await until(
      () => second.hud.read(dialog).catch(() => ({ found: false }) as const),
      (control) => control.found,
      { timeoutMs: SECOND_CLIENT_TIMEOUT_MS, intervalMs: 2_000, what: 'the "Unrecognised link parameters" dialog on the second client' }
    )
    assert.ok(read.found && read.text.includes('e2eUnknownParam'), `the dialog should name the parameter it ignored: ${read.found ? read.text.slice(0, 200) : ''}`)
    assert.ok(await second.hud.isVisible({ name: 'e2eUnknownParam', role: 'listitem' }), 'the ignored parameter should be listed')
    await second.hud.click({ name: 'OK', role: 'button' })
    await second.hud.waitHidden(dialog, CONTROL_TIMEOUT_MS)
  } finally {
    await second.close()
  }
}, { timeoutMs: 180_000 })

scenario("the rail's Scene permissions opens the popover and closes it again", async ({ explorer, restore }) => {
  const rail: Selector = { name: 'Scene permissions', role: 'button' }
  const popover: Selector = { name: 'Scene permissions', role: 'dialog' }
  restore(async () => {
    if (await explorer.hud.isVisible(popover)) await explorer.hud.click(rail)
  })
  await explorer.hud.click(rail)
  const open = await explorer.hud.wait(popover, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.ok(open.found && /scene permissions/i.test(open.text), 'the popover should be titled SCENE PERMISSIONS')
  assert.equal((await explorer.hud.get(rail)).checked, true, 'the rail button should read pressed while its popover is open')
  // the scene at the spawn asks for nothing: the popover says so, with the reset beside it
  assert.ok(/nothing decided|this visit|saved answers/i.test(open.text), `the popover should list this scene's permissions or say there are none: ${open.text.slice(0, 160)}`)
  await explorer.hud.click(rail)
  await explorer.hud.waitHidden(popover, CONTROL_TIMEOUT_MS)
  assert.equal((await explorer.hud.get(rail)).checked, false, 'the rail button should read released once the popover is closed')
})

scenario("the rail's Create opens the Create page, and its Close returns to the world", async ({ explorer, restore }) => {
  restore(async () => {
    if (!(await explorer.hud.isVisible(RAIL_NAVIGATION)) && (await explorer.hud.isVisible(MENU_CLOSE))) await explorer.hud.click(MENU_CLOSE)
  })
  // the rail's System panel holds the Create row (#929)
  await pressRailRow(explorer, { button: 'System', row: 'Create' })
  // a menu page takes the screen: the rail gives way to the top bar, with Create among its tabs
  await explorer.hud.waitHidden(RAIL_NAVIGATION, PAGE_TIMEOUT_MS)
  await explorer.hud.wait(MENU_CLOSE, { timeoutMs: PAGE_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible({ name: 'Lobby', role: 'button' }), "the menu's top bar should be up")
  assert.equal(await explorer.hud.isVisible(SETTINGS_HEADING), false, 'the page should be Create, not Settings')
  await explorer.hud.click(MENU_CLOSE)
  await explorer.hud.wait(RAIL_NAVIGATION, { timeoutMs: PAGE_TIMEOUT_MS })
})

for (const name of ['Avatar Cel Shading', 'Avatar Outline', 'Avatar Ghosts']) {
  scenario(`the ${name} switch flips and the engine takes it`, async ({ explorer, restore }) => {
    const control: Selector = { name, role: 'switch' }
    restore(() => closeMenuPage(explorer, SETTINGS_HEADING))
    await openGraphics(explorer)
    const before = await engineValues(explorer, 'Graphics')
    restore(() => restoreValues(explorer, 'Graphics', before))
    const wasOn = (await explorer.hud.get(control)).checked === true
    await explorer.hud.click(control)
    await until(() => explorer.hud.get(control), (read) => read.checked === !wasOn, { timeoutMs: APPLY_TIMEOUT_MS, what: `${name} to read ${wasOn ? 'off' : 'on'}` })
    await engineTook(explorer, before, name, wasOn ? 0 : 1)
  })
}

/** Choose another option of a Graphics select; the engine's value is the option's index. */
async function chooseGraphics(explorer: Explorer, restore: (undo: () => Promise<void>) => void, name: string, wanted: (options: string[], shown: string) => string): Promise<void> {
  const control: Selector = { name, role: 'button' }
  restore(() => closeMenuPage(explorer, SETTINGS_HEADING))
  await openGraphics(explorer)
  const before = await engineValues(explorer, 'Graphics')
  restore(() => restoreValues(explorer, 'Graphics', before))
  const shown = (await explorer.hud.text(control)).trim()
  await explorer.hud.click(control)
  const options = (await explorer.hud.list('option')).map((entry) => entry.name)
  const option = wanted(options, shown)
  assert.ok(options.includes(option), `${name} should offer ${option}: ${options.join(', ')}`)
  await explorer.hud.click({ name: option, role: 'option' })
  await until(() => explorer.hud.text(control), (text) => text.includes(option), { timeoutMs: APPLY_TIMEOUT_MS, what: `${name} to read ${option}` })
  await engineTook(explorer, before, `${name} ${option}`, options.indexOf(option))
}

scenario('Distant Scene Rendering (imposters) goes to Off and the engine takes it', async ({ explorer, restore }) => {
  await chooseGraphics(explorer, restore, 'Distant Scene Rendering', (options, shown) => (shown.includes('Off') ? options.find((o) => o !== 'Off')! : 'Off'))
})

scenario('Empty Parcel Props (grass) changes density and the engine takes it', async ({ explorer, restore }) => {
  await chooseGraphics(explorer, restore, 'Empty Parcel Props', (_options, shown) => (shown.includes('High') ? 'Off' : 'High'))
})

scenario('Ctrl+Shift+F shows the FPS meter and hides it again', async () => {}, {
  todo: 'the FPS meter (features/debug/FpsMeter.tsx) is aria-hidden: the HUD matcher cannot see it, and the engine frame the suite captures has no HUD'
})

scenario("/world to a world that does not exist is refused, and the player stays where they were", async ({ explorer, restore }) => {
  const dialog: Selector = { name: "Can't travel there", role: 'dialog' }
  const before = await explorer.worldState()
  assert.ok(before.position, 'the player should have a position')
  const missing = `e2e-no-such-world-${Date.now().toString(36)}.dcl.eth`
  restore(async () => {
    if (await explorer.hud.isVisible(dialog)) await explorer.hud.click({ name: 'OK', role: 'button' })
    if (await explorer.hud.isVisible(CLOSE_CHAT)) await closeChat(explorer)
    await explorer.returnToRealm(before)
  })
  await openChat(explorer)
  await submitLine(explorer, `/world ${missing}`)
  const refused = await explorer.hud.wait(dialog, { timeoutMs: TRAVEL_CHECK_TIMEOUT_MS })
  assert.ok(refused.found, "the HUD should answer \"Can't travel there\"")
  await explorer.hud.click({ name: 'OK', role: 'button' })
  await explorer.hud.waitHidden(dialog, CONTROL_TIMEOUT_MS)
  const after = await explorer.worldState()
  assert.equal(after.realmName, before.realmName, 'the player should stay in the realm')
  assert.equal(after.arrivals, before.arrivals, 'no travel should have happened')
  assert.equal(after.inWorld, true, 'the player should still be in the world')
  const parcel = (p: [number, number, number]): string => `${Math.floor(p[0] / PARCEL_METERS)},${Math.floor(p[2] / PARCEL_METERS)}`
  assert.equal(parcel(after.position!), parcel(before.position), 'the player should stay in the parcel')
})
