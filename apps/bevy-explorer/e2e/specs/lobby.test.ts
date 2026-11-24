// QA's release checklist, High End Functionality → LOBBY: the Lobby reopened in-world from the
// menu's top bar, and the startup Lobby a sign-in with no destination lands in.
//
// | Checklist case                    | Here                                                                         | Class   |
// | Open Lobby in world               | the top bar's Lobby opens it over the world; its Close and Escape close it    | Port    |
// | Travel from in-world Lobby        | the landing card's Jump In goes home; a Recommended Places card's travels     | Partial |
// | Jump in to Home                   | the in-world landing card shows the home place and its Jump In goes there     | Partial |
// | Customize avatar from Lobby       | Customize opens the Backpack over the Lobby, and Escape returns to it          | Partial |
// | Notifications from Lobby          | the Lobby's bell opens the notifications panel and closes it again           | Port    |
// | Profile menu in Lobby             | the profile chip's Profile row opens the passport  (Sign Out: `auth`)         | Partial |
// | Recommended Places                | the section shows cards, and one's Jump in travels                            | Partial |
// | Jump Back In                      | a place visited from the Lobby is listed under Jump Back In                   | Gap     |
// | Live Now                          | the Live Events widget's cards offer a Jump in, when anything is live         | Partial |
// | Lobby on startup                  | a sign-in with no destination lands in the Lobby: welcome, bell, chip, card  | Port    |
// | Failed travel from Lobby          | no Lobby control takes a destination that cannot be reached                  | Gap     |
//
// Partial:
// - The Lobby is a board of widgets (#996): Welcome and the landing card, Friends, Live Events,
//   Upcoming Events, Jump Back In and Recommended Places, each a section, in that page order. The
//   landing card's button is `Jump In`; a place's or a live event's is `Jump in to <title>`. A Jump
//   in is checked by the travel it makes: the Lobby closes and the player arrives somewhere else
//   (`/world_state` arrivals and position).
// - The landing card is the first Jump in on the page (its widget comes first); the Recommended
//   Places rail is the last widget, so its cards' buttons are the last ones. A rail scrolls its
//   pages, so only the cards on its first page are visible.
// - Travel to a World from the Lobby needs a World card on a rail, which the backend's data decides:
//   not staged (on zone a Recommended Places card is often one, and the scenario reports where it
//   went). Customize's change of look on the Lobby's avatar is the engine's drawing, not checked.
// - The in-world Lobby's profile menu offers Sign Out on the web only (`signOutOfWorld`; the
//   standalone keeps its account while a realm is loaded). The scenario does not press it, since
//   the session is shared; `auth` signs out from the startup Lobby. The chip is named after the
//   profile, `<name>#<tag>`; a guest's name is the HUD's own pick (`Bevy_User`), whatever name the
//   sign-in asked for.
// - The checklist's Live Now is the Live Events widget: live events, from the events service, which
//   refuses this loopback origin by CORS when the suite serves the app itself. An empty widget is
//   hidden, and the scenario then only reports it.
//
// Gap, Jump Back In: the Lobby reads the visited places back with a POST to the places service's
// /destinations, whose CORS preflight refuses the loopback origin the suite serves the app from, so
// the section never loads (Recommended Places' GET passes). It runs against an app served from an
// allowed origin (`DCL_E2E_WEB_URL`).
//
// Gap, Failed travel: the Lobby's destinations are its cards, which the backend lists: a World that
// is not deployed is never one of them. The Go to widget, which takes a typed World, is not on the
// default board (a user adds it from Customize lobby layout).
//
// The in-world scenarios share the default session and put the player back at the target's parcel.
// The startup Lobby needs a sign-in that picks no destination, which the default session's boot
// always does: it runs on a client of its own (the engine's own guest, whose HUD stays on its start
// page), signs a fresh guest account in through `/hud_login` with no realm or position, and closes
// the client afterwards.
import assert from 'node:assert/strict'
import { buildAuthIdentity, newPrivateKey, readTarget, spec, until, type Explorer, type Selector, type WorldState } from '../driver/index.ts'
import { PAGES, pressRailRow } from './pages.ts'

const scenario = spec(import.meta.url)
const target = readTarget()

const CONTROL_TIMEOUT_MS = 20_000
const LOBBY_TIMEOUT_MS = 30_000
const TRAVEL_TIMEOUT_MS = 120_000
const PARCEL_METERS = 16

/** The page the top bar's Lobby button sits on: any menu page has it; Settings opens from the rail. */
const SETTINGS_PAGE: Selector = { name: 'Settings', role: 'heading' }
const LOBBY_BUTTON: Selector = { name: 'Lobby', role: 'button' }
/** The Lobby's welcome line, `Welcome <name>!` (or `Welcome!` before the profile is known). */
const WELCOME: Selector = { name: 'Welcome', role: 'heading', exact: false }
const LOBBY_CLOSE: Selector = { name: 'Close', role: 'button' }
const CUSTOMIZE: Selector = { name: 'Customize', role: 'button' }
/** The bell: `Notifications`, or `Notifications, <n> unread` with unread ones. */
const BELL_NAME = /^Notifications(, \d+ unread)?$/
const NOTIFICATIONS_CLOSE: Selector = { name: 'Close notifications', role: 'button' }
const BACKPACK_MODAL: Selector = { name: 'Backpack', role: 'dialog' }
const VIEW_PROFILE: Selector = { name: 'Profile', role: 'menuitem' }
/** The profile chip's menu (LobbyProfile.tsx): its rows are menu items. */
const SIGN_OUT: Selector = { name: 'Sign Out', role: 'menuitem' }
const PASSPORT: Selector = { name: 'Passport sections', role: 'tablist' }
/** Part of the landing card's `Jump In` and of the cards' `Jump in to <title>`. */
const JUMP_IN = 'Jump in'
const RECOMMENDED: Selector = { name: 'Recommended Places', role: 'heading' }
const JUMP_BACK_IN: Selector = { name: 'Jump Back In', role: 'heading' }
const LIVE_EVENTS: Selector = { name: 'Live Events', role: 'heading' }
/** The in-world Lobby offers Sign Out on the web only. */
const NO_IN_WORLD_SIGN_OUT = target.transport === 'native'

function parcelOf(state: WorldState): [number, number] | undefined {
  const p = state.position
  return p ? [Math.floor(p[0] / PARCEL_METERS), Math.floor(p[2] / PARCEL_METERS)] : undefined
}

async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

/** Open the in-world Lobby: Settings from the rail, then the top bar's Lobby. */
async function openLobby(explorer: Explorer): Promise<void> {
  // the rail's System panel holds the Settings row, whose page's top bar has the Lobby (#929)
  await pressRailRow(explorer, PAGES.settings.rail!)
  await explorer.hud.wait(SETTINGS_PAGE, { timeoutMs: CONTROL_TIMEOUT_MS })
  await press(explorer, LOBBY_BUTTON)
  await explorer.hud.wait(WELCOME, { timeoutMs: LOBBY_TIMEOUT_MS })
}

/** Close the in-world Lobby: Escape, or its Close (the first `Close`: a popup over it adds its own). */
async function closeLobby(explorer: Explorer): Promise<void> {
  await explorer.escape()
  try {
    await explorer.hud.waitHidden(WELCOME, 5_000)
    return
  } catch {
    // the press went to something else first
  }
  await press(explorer, { name: 'Close', role: 'button', index: 0 })
  await explorer.hud.waitHidden(WELCOME, CONTROL_TIMEOUT_MS)
}

/** The Lobby's bell, by its name of the moment (it carries the unread count). */
async function bell(explorer: Explorer): Promise<Selector> {
  const found = (await explorer.hud.list('button')).find((button) => BELL_NAME.test(button.name))
  assert.ok(found, 'the Lobby should show its Notifications bell')
  return { name: found.name, role: 'button' }
}

/** The visible Jump in buttons (`Jump In`, `Jump in to <title>`), in page order. */
async function jumpIns(explorer: Explorer): Promise<Selector[]> {
  const found: Selector[] = []
  for (let index = 0; index < 40; index++) {
    const read = await explorer.hud.read({ name: JUMP_IN, role: 'button', exact: false, index })
    if (!read.found) break
    if (read.visible) found.push({ name: JUMP_IN, role: 'button', exact: false, index })
  }
  return found
}

/** Click a Jump in and wait for the Lobby to close and the player to arrive. */
async function jumpAndArrive(explorer: Explorer, button: Selector): Promise<WorldState> {
  const before = await explorer.worldState()
  await explorer.hud.wait(button, { state: 'actionable', timeoutMs: LOBBY_TIMEOUT_MS })
  await explorer.hud.click(button)
  await explorer.hud.waitHidden(WELCOME, LOBBY_TIMEOUT_MS)
  return until(() => explorer.worldState(), (now) => now.arrivals > before.arrivals && now.inWorld, {
    timeoutMs: TRAVEL_TIMEOUT_MS,
    what: 'the arrival after the Lobby\'s Jump in'
  })
}

/** Put the player back where the run starts. */
async function backToStart(explorer: Explorer): Promise<void> {
  const [x, y] = target.position.split(',').map(Number) as [number, number]
  // a Jump in can land in a World: its `realm` stays the catalyst's, its `realmName` is the World's
  if (((await explorer.worldState()).realmName ?? '').toLowerCase().endsWith('.eth')) await explorer.changeRealm(target.realm, { timeoutMs: TRAVEL_TIMEOUT_MS })
  const here = parcelOf(await explorer.worldState())
  if (here && here[0] === x && here[1] === y) return
  await explorer.teleport(x, y, { timeoutMs: TRAVEL_TIMEOUT_MS })
}

scenario('the top bar opens the Lobby over the world, and Close and Escape close it', async ({ explorer }) => {
  await openLobby(explorer)
  assert.ok(await explorer.hud.isVisible(CUSTOMIZE), 'the Lobby should show its Customize control')
  await bell(explorer)
  assert.ok((await jumpIns(explorer)).length >= 1, 'the Lobby should show the landing card\'s Jump In')
  await press(explorer, LOBBY_CLOSE)
  await explorer.hud.waitHidden(WELCOME, CONTROL_TIMEOUT_MS)
  assert.equal((await explorer.worldState()).inWorld, true, 'closing the Lobby should leave the player in the world')

  await openLobby(explorer)
  await explorer.escape()
  await explorer.hud.waitHidden(WELCOME, CONTROL_TIMEOUT_MS)
})

scenario('the in-world landing card\'s Jump in takes the player home', async ({ explorer, t, restore }) => {
  restore(() => backToStart(explorer))
  await openLobby(explorer)
  const [landing] = await jumpIns(explorer)
  assert.ok(landing, 'the landing card should offer a Jump in')
  const before = parcelOf(await explorer.worldState())
  const arrived = await jumpAndArrive(explorer, landing)
  const parcel = parcelOf(arrived)
  t.diagnostic(`from ${before?.join(',')} to ${parcel?.join(',')} in ${arrived.realmName || arrived.realm}`)
  assert.notDeepEqual(parcel, before, 'the landing card\'s Jump in should take the player away from where they stood')
})

scenario('a Recommended Places card\'s Jump in travels there', async ({ explorer, t, restore }) => {
  restore(() => backToStart(explorer))
  await openLobby(explorer)
  await explorer.hud.wait(RECOMMENDED, { timeoutMs: LOBBY_TIMEOUT_MS })
  const buttons = await jumpIns(explorer)
  assert.ok(buttons.length >= 2, `the Lobby should show the landing card and Recommended Places cards; found ${buttons.length} Jump in`)
  const before = await explorer.worldState()
  const arrived = await jumpAndArrive(explorer, buttons[buttons.length - 1]!)
  t.diagnostic(`a recommended place took the player from ${parcelOf(before)?.join(',')} in ${before.realmName} to ${parcelOf(arrived)?.join(',')} in ${arrived.realmName}`)
  assert.ok(
    arrived.realmName !== before.realmName || parcelOf(arrived)?.join(',') !== parcelOf(before)?.join(','),
    'a recommended place\'s Jump in should take the player somewhere else'
  )
})

scenario('a place visited from the Lobby shows under Jump Back In', async ({ explorer, restore }) => {
  restore(() => backToStart(explorer))
  await openLobby(explorer)
  await explorer.hud.wait(RECOMMENDED, { timeoutMs: LOBBY_TIMEOUT_MS })
  const buttons = await jumpIns(explorer)
  assert.ok(buttons.length >= 2, `the Lobby should show Recommended Places cards; found ${buttons.length} Jump in`)
  await jumpAndArrive(explorer, buttons[buttons.length - 1]!)
  // the visit is recorded once the HUD hears of the arrival (`recordVisit`, a places lookup)
  await until(
    async () => {
      await openLobby(explorer)
      const listed = await explorer.hud.isVisible(JUMP_BACK_IN)
      await closeLobby(explorer)
      return listed
    },
    (listed) => listed,
    { timeoutMs: 60_000, intervalMs: 3_000, what: 'the visited place under Jump Back In' }
  )
}, {
  // runs against an app served from an origin the places service allows (DCL_E2E_WEB_URL)
  todo: readTarget().webUrl
    ? undefined
    : 'the Lobby reads the visited places back with a POST to the places service\'s /destinations, whose CORS preflight refuses the loopback origin the suite serves the app from, so the section never loads (Recommended Places\' GET passes)'
})

scenario('the Lobby\'s Live Events cards each offer a way in, when anything is live', async ({ explorer, t }) => {
  await openLobby(explorer)
  // the sections load on their own; Recommended Places is the last to settle the layout
  await explorer.hud.wait(RECOMMENDED, { timeoutMs: LOBBY_TIMEOUT_MS }).catch(() => undefined)
  if (!(await explorer.hud.isVisible(LIVE_EVENTS))) {
    t.diagnostic('nothing is live on the backend (or the events service refused this origin): the Live Events widget is hidden')
  } else {
    // the landing card's, the live events' and the rails': more than the landing card's alone
    const buttons = await jumpIns(explorer)
    t.diagnostic(`Live Events is shown; ${buttons.length} Jump in on the page`)
    assert.ok(buttons.length >= 2, 'a Lobby with Live Events should offer a Jump in beside the landing card\'s')
  }
  await closeLobby(explorer)
})

scenario('Customize opens the Backpack over the Lobby, and Escape returns to the Lobby', async ({ explorer }) => {
  await openLobby(explorer)
  await press(explorer, CUSTOMIZE)
  await explorer.hud.wait(BACKPACK_MODAL, { timeoutMs: LOBBY_TIMEOUT_MS })
  await explorer.escape()
  await explorer.hud.waitHidden(BACKPACK_MODAL, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(WELCOME), 'closing the Backpack should leave the Lobby open')
  await closeLobby(explorer)
})

scenario('the Lobby\'s bell opens the notifications panel and closes it', async ({ explorer }) => {
  await openLobby(explorer)
  await press(explorer, await bell(explorer))
  await explorer.hud.wait(NOTIFICATIONS_CLOSE, { timeoutMs: CONTROL_TIMEOUT_MS })
  // opening the panel can mark the notifications read, which renames the bell
  await press(explorer, await bell(explorer))
  await explorer.hud.waitHidden(NOTIFICATIONS_CLOSE, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(WELCOME), 'the bell should leave the Lobby open')
  await closeLobby(explorer)
})

scenario('the Lobby\'s profile chip opens the own passport, and offers Sign Out in-world on the web only', async ({ explorer }) => {
  const { name } = await explorer.userData()
  await openLobby(explorer)
  await press(explorer, { name: name.split('#')[0]!, role: 'button', exact: false })
  await explorer.hud.wait(VIEW_PROFILE, { timeoutMs: CONTROL_TIMEOUT_MS })
  // not pressed: the in-world sign-out would end the session every scenario shares
  assert.equal(
    await explorer.hud.isVisible(SIGN_OUT),
    !NO_IN_WORLD_SIGN_OUT,
    NO_IN_WORLD_SIGN_OUT ? 'the standalone\'s in-world Lobby should not offer Sign Out' : 'the web\'s in-world Lobby should offer Sign Out'
  )
  await press(explorer, VIEW_PROFILE)
  await explorer.hud.wait(PASSPORT, { timeoutMs: LOBBY_TIMEOUT_MS })
  // the passport's Close comes after the Lobby's in the page
  await press(explorer, { name: 'Close', role: 'button', index: 1 })
  await explorer.hud.waitHidden(PASSPORT, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(WELCOME), 'closing the passport should leave the Lobby open')
  await closeLobby(explorer)
})

scenario('a Jump in to an unreachable World returns the player to the Lobby', async () => {}, {
  todo: 'the Lobby travels only to the cards the backend lists, and has no field for a destination: a World that is not deployed cannot be picked from it'
})

// The startup Lobby, on a client of its own: see the header.
const STARTUP = { signIn: 'unsigned-guest' } as const

/** The Lobby's profile chip: the one button named after a profile, `<initial> <name>#<tag>` (the
 *  avatar's initial, then the name with its tag). The startup guest's name is the HUD's own pick. */
async function profileChip(explorer: Explorer): Promise<Selector | undefined> {
  const chip = (await explorer.hud.list('button')).find((button) => /#[0-9a-f]{4}$/i.test(button.name))
  return chip ? { name: chip.name, role: 'button' } : undefined
}

/** Sign a fresh guest account in through the HUD with no destination: the HUD stays in its Lobby. */
async function signInToLobby(explorer: Explorer): Promise<void> {
  const reply = await explorer.hudLogin(buildAuthIdentity(newPrivateKey()), { name: 'e2elobby', look: 1, guest: true })
  assert.equal(reply.phase, 'lobby', `a sign-in with no destination should stay in the Lobby; the HUD answered in phase ${reply.phase}`)
  await explorer.hud.wait(WELCOME, { timeoutMs: LOBBY_TIMEOUT_MS })
}

scenario('a sign-in with no destination lands in the Lobby, and its landing card enters the world', async ({ explorer, t }) => {
  await signInToLobby(explorer)
  assert.equal((await explorer.worldState()).inWorld, false, 'the startup Lobby holds the world')
  assert.ok(await explorer.hud.isVisible(CUSTOMIZE), 'the startup Lobby should show its Customize control')
  await bell(explorer)
  assert.ok(await profileChip(explorer), 'the startup Lobby should show the profile chip')
  assert.equal(await explorer.hud.isVisible(LOBBY_CLOSE), false, 'the startup Lobby has no Close: Jump In is the way out')
  const [landing] = await jumpIns(explorer)
  assert.ok(landing, 'the landing card should offer a Jump In')
  // the Jump ins are disabled until the sign-in has finished
  await explorer.hud.wait(landing, { state: 'actionable', timeoutMs: LOBBY_TIMEOUT_MS })
  await explorer.hud.click(landing)
  const arrived = await explorer.waitInWorld(TRAVEL_TIMEOUT_MS)
  t.diagnostic(`entered ${arrived.realmName || arrived.realm} at ${parcelOf(arrived)?.join(',')}`)
  await explorer.hud.waitHidden(WELCOME, CONTROL_TIMEOUT_MS)
}, { session: STARTUP, noEscape: true, closeSession: true, timeoutMs: 300_000 })
