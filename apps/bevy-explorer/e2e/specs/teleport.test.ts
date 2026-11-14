// The QA Automation Suite's TELEPORT cases (beyond explorer-automation, which has none): a
// coordinate in a chat message, `/goto x,y`, `/world <name>`, `/goto x,y` inside a world, and the
// Map's JUMP IN. The HUD runs `/goto` and `/world` itself (explorer-core chatCommands.ts) and sends
// nothing to the chat; a coordinate in a message is a `button` named by the coordinate
// (chatText.tsx) that teleports at once, with no confirmation. Each case is read through the
// engine: `/world_state`'s `arrivals` grows and the player stands in the parcel asked for
// (floor(x / 16), floor(z / 16)), or the realm is the world's.
//
// | Suite case            | Here                                                                         | Class   |
// | Coordinates on chat   | a message carrying "x,y" renders the coordinate as a button; clicking it teleports there | Port    |
// | Coordinates via command | `/goto x,y` typed in the chat teleports there                              | Port    |
// | Worlds                | `/world <name>` changes the realm to the world                               | Partial |
// | Coordinates in world  | in the world, `/goto x,y` teleports to a parcel of the world                 | Port    |
// | Via Map               | the Map's search finds a place read from a category list; its card's JUMP IN teleports | Partial |
//
// Partial, Worlds: the suite's `olavra` is not a world on decentraland.zone, so the world is
// `DCL_E2E_WORLD` (`33.dcl.eth` by default: a zone world open to anyone, parcels 0,0 to 4,5, spawn
// 0,0); the in-world `/goto` targets `DCL_E2E_WORLD_PARCEL` (2,3), away from the spawn.
// The world's access must let the test accounts in (unrestricted, or them on its allow-list), or the
// HUD answers "Can't travel there". The arrival is read from `realmName`: a world's `realm` and
// content origin stay the catalyst's (`https://peer.decentraland.zone`), as its about names that one. `/goto <world>` is the same parse as `/world` (chatCommands.ts)
// and is not run twice.
// Partial, Via Map: the suite clicks a scene on the map canvas, which has no addressable parcels.
// The place comes from a category list instead (nothing asserts zone's live listings as literals),
// is found again through the search, whose result names its base parcel, and its JUMP IN is
// pressed. A scene's spawn point may put the player in another of its parcels, so the arrival is
// asserted and the parcel is reported, not compared.
//
// Every scenario puts the player back on the realm and the parcel it started on.
import assert from 'node:assert/strict'
import { closeToRest, spec, until, type Explorer, type WorldState } from '../driver/index.ts'
import { RAIL_CHAT, closeChat, openChat, submitLine } from './chat.ts'
import { PAGES, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const PARCEL_METERS = 16
/** A teleport's arrival: the realm's scenes around the parcel load before `inWorld` returns. */
const ARRIVAL_TIMEOUT_MS = 90_000
/** A realm change: the world's about, its scenes and the comms connection. */
const REALM_TIMEOUT_MS = 120_000
const CONTROL_TIMEOUT_MS = 20_000
/** The conversation rail's Nearby button, pressed while the Nearby conversation shows (ConversationRail.tsx). */
const NEARBY_CHAT = { name: 'Nearby chat', role: 'button' } as const
const WORLD = (process.env.DCL_E2E_WORLD ?? '33.dcl.eth').toLowerCase()
const WORLD_PARCEL = (process.env.DCL_E2E_WORLD_PARCEL ?? '2,3').split(',').map(Number) as [number, number]
const SCENARIO_TIMEOUT_MS = 300_000

type Parcel = [number, number]

function parcelOf(state: WorldState): Parcel {
  assert.ok(state.position, 'the player should have a position in the world')
  return [Math.floor(state.position[0] / PARCEL_METERS), Math.floor(state.position[2] / PARCEL_METERS)]
}

const showParcel = ([x, y]: Parcel): string => `${x},${y}`

/** The HUD back after an arrival: `/world_state` reads `inWorld` while the HUD's loading screen still
 *  covers the page, rail included, until the scenes around the player are loaded. */
async function hudBack(explorer: Explorer): Promise<void> {
  await explorer.hud.wait(RAIL_CHAT, { state: 'actionable', timeoutMs: ARRIVAL_TIMEOUT_MS })
}

/** Where the scenario started, and the restore that takes the player back there. */
async function startingPoint(explorer: Explorer, restore: (undo: () => Promise<void>) => void): Promise<{ state: WorldState; parcel: Parcel }> {
  const state = await explorer.waitInWorld()
  await hudBack(explorer)
  const parcel = parcelOf(state)
  restore(async () => {
    // A failure can leave the Map or the chat open: the rail, and the chat's button on it, come back at rest.
    await closeToRest(explorer)
    await closeChat(explorer)
    const now = await explorer.waitInWorld(ARRIVAL_TIMEOUT_MS)
    if (now.realm !== state.realm) await explorer.changeRealm(state.realm, { timeoutMs: REALM_TIMEOUT_MS })
    const back = await explorer.worldState()
    if (showParcel(parcelOf(back)) !== showParcel(parcel)) await explorer.teleport(parcel[0], parcel[1], { timeoutMs: ARRIVAL_TIMEOUT_MS })
    await hudBack(explorer)
  })
  return { state, parcel }
}

/** Wait for an arrival counted after `before`, then for the player to stand in `parcel`. */
async function waitArrivalAt(explorer: Explorer, before: number, parcel: Parcel): Promise<WorldState> {
  await until(() => explorer.worldState(), (state) => state.arrivals > before && state.inWorld, { timeoutMs: ARRIVAL_TIMEOUT_MS, what: `the arrival at ${showParcel(parcel)}` })
  const there = await until(() => explorer.worldState(), (state) => state.position !== null && showParcel(parcelOf(state)) === showParcel(parcel), {
    timeoutMs: CONTROL_TIMEOUT_MS,
    what: `the player to stand in parcel ${showParcel(parcel)}`
  })
  await hudBack(explorer)
  return there
}

/** A parcel a couple away from the start: near enough to load fast, far enough to be another parcel. */
const nearby = ([x, y]: Parcel, dx: number, dy: number): Parcel => [x + dx, y + dy]

scenario(
  'a coordinate in a chat message is a button that teleports to it',
  async ({ explorer, restore }) => {
    const { parcel } = await startingPoint(explorer, restore)
    const target = nearby(parcel, 2, 0)
    const coordinate = showParcel(target)
    await openChat(explorer)
    // An earlier spec can leave a community's conversation shown, whose messages /chat_history does
    // not hold and whose members would get this one: the message goes to Nearby.
    if (!(await explorer.hud.get(NEARBY_CHAT)).checked) await explorer.hud.click(NEARBY_CHAT)
    await until(async () => (await explorer.hud.get(NEARBY_CHAT)).checked, (shown) => shown === true, { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the Nearby conversation to show' })
    await openChat(explorer)
    await submitLine(explorer, `automation teleport ${coordinate}`)

    // Earlier runs left the same coordinate in the history: the newest is the last in the page's order.
    const buttons = await until(
      async () => (await explorer.hud.list('button')).filter((entry) => entry.name === coordinate),
      (found) => found.length > 0,
      { timeoutMs: CONTROL_TIMEOUT_MS, what: `the coordinate ${coordinate} to render as a button in the chat` }
    )
    const before = (await explorer.worldState()).arrivals
    await explorer.hud.click({ name: coordinate, role: 'button', index: buttons.length - 1 })
    await waitArrivalAt(explorer, before, target)
    await closeChat(explorer)
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  '/goto x,y typed in the chat teleports to the parcel',
  async ({ explorer, restore }) => {
    const { parcel } = await startingPoint(explorer, restore)
    const target = nearby(parcel, 0, 2)
    await openChat(explorer)
    const before = (await explorer.worldState()).arrivals
    await submitLine(explorer, `/goto ${showParcel(target)}`)
    await waitArrivalAt(explorer, before, target)
    await closeChat(explorer)
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)

scenario(
  `/world ${WORLD} changes the realm to the world, and /goto there teleports inside it`,
  async ({ explorer, restore }) => {
    const { state } = await startingPoint(explorer, restore)
    await openChat(explorer)
    await submitLine(explorer, `/world ${WORLD}`)
    const inWorld = await until(() => explorer.worldState(), (now) => now.arrivals > state.arrivals && now.inWorld && now.realmName.toLowerCase() === WORLD, {
      timeoutMs: REALM_TIMEOUT_MS,
      what: `the arrival in ${WORLD}`
    })
    assert.notEqual(inWorld.realmName, state.realmName, 'the realm should have changed to the world')
    await hudBack(explorer)

    await openChat(explorer)
    await submitLine(explorer, `/goto ${showParcel(WORLD_PARCEL)}`)
    const there = await waitArrivalAt(explorer, inWorld.arrivals, WORLD_PARCEL)
    assert.equal(there.realm, inWorld.realm, 'a /goto inside a world should keep the player in the world')
    await closeChat(explorer)
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)

// The Map's category chips (MapPage.tsx CATEGORIES), tried in order until one lists a place.
const CATEGORY_CHIPS = ['Art', 'Social', 'Game', 'Music', 'Fashion'] as const
/** A sidebar card is a button named by its whole text, its text nodes joined by spaces: the title,
 *  "created by <owner>" when it has one, then the stats ("👍 85 %", "👤 3") and the chevron. The
 *  title is what precedes the first of those. */
const CARD = /^(.+?)\s+(?:created by |👍|👤)/u
/** A search result is a button named by the place's title and its base parcel. */
const RESULT = /^(.+?)\s+(-?\d+),(-?\d+)$/

async function firstListedPlace(explorer: Explorer): Promise<string> {
  for (const chip of CATEGORY_CHIPS) {
    await explorer.hud.click({ name: chip, role: 'button', index: 0 })
    const title = await until(
      async () => (await explorer.hud.list('button')).map((entry) => CARD.exec(entry.name)?.[1]).find((found) => found !== undefined),
      (found) => found !== undefined,
      { timeoutMs: CONTROL_TIMEOUT_MS, what: `a place in the ${chip} list` }
    ).catch(() => undefined)
    if (title) return title
  }
  throw new Error(`none of the Map's ${CATEGORY_CHIPS.join(', ')} lists showed a place`)
}

scenario(
  "a place found with the Map's search opens its card, and JUMP IN teleports there",
  async ({ explorer, restore, t }) => {
    const { state } = await startingPoint(explorer, restore)
    await openWithKey(explorer, 'map')
    const title = await firstListedPlace(explorer)
    // The category's side list covers the search field: closed with its own Close, after the top bar's.
    const closes = (await explorer.hud.list('button')).filter((entry) => entry.name === 'Close').length
    await explorer.hud.click({ name: 'Close', role: 'button', index: closes - 1 })
    await explorer.hud.wait(PAGES.map.marker, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
    // The search lists places once it holds two characters; the place's longest word finds it.
    const query = title.split(/\s+/).sort((a, b) => b.length - a.length)[0]!
    await explorer.hud.fill(PAGES.map.marker, query)
    const result = await until(
      async () => (await explorer.hud.list('button')).find((entry) => RESULT.exec(entry.name)?.[1]?.toLowerCase() === title.toLowerCase()),
      (found) => found !== undefined,
      { timeoutMs: CONTROL_TIMEOUT_MS, what: `"${title}" among the results for "${query}"` }
    )
    const [, , bx, by] = RESULT.exec(result!.name)!
    await explorer.hud.click({ name: result!.name, role: 'button' })
    await explorer.hud.wait({ name: title, role: 'heading' }, { timeoutMs: CONTROL_TIMEOUT_MS })

    await explorer.hud.click({ name: 'JUMP IN', role: 'button' })
    const arrived = await until(() => explorer.worldState(), (now) => now.arrivals > state.arrivals && now.inWorld, { timeoutMs: ARRIVAL_TIMEOUT_MS, what: `the arrival at "${title}"` })
    await hudBack(explorer)
    t.diagnostic(`"${title}" has its base parcel at ${bx},${by}; the player arrived in ${showParcel(parcelOf(arrived))}`)
    if (await explorer.hud.isVisible(PAGES.map.marker)) await closePage(explorer, 'map')
  },
  { timeoutMs: SCENARIO_TIMEOUT_MS }
)

