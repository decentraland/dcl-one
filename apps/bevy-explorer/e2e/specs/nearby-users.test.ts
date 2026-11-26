// QA Automation Suite, High End Functionality / CHAT: the people around the player, as the chat's
// Nearby title bar counts them (library/ui/apps/hud/features/chat/Chat.tsx). The bar's members
// button is named `<n> nearby`; it opens the members overlay in place of the messages: `Back`,
// the `<n> Online` count (text with no role) and one row per person, a button named `View <name>`
// that opens their profile card. The engine's own list of the players it is connected to is
// `/connected_players`. Who is around changes while a scenario runs (other clients come and go),
// so the counts are compared once they agree, within a window.
//
// | QA case                 | Here                                                                 | Class   |
// | Connected users around  | the `<n> nearby` button's count matches the engine's connected players | Partial |
// | Nearby                  | the button opens the members overlay with one `View` row per counted person; Back returns to the messages | Partial |
//
// Partial: the overlay's `<n> Online` and the rows' names and pictures are text with no role; the
// count is read from the button's name. That the number matches the avatars one can see in the
// scene is not observable from one client beyond `/connected_players`.
import assert from 'node:assert/strict'
import { spec, until, type Explorer } from '../driver/index.ts'
import { closeChat, openChat } from './chat.ts'

const scenario = spec(import.meta.url)

const AGREE_TIMEOUT_MS = 30_000
const TEXT_TIMEOUT_MS = 10_000
const NEARBY = /^(\d+) nearby$/
const BACK = { name: 'Back', role: 'button' } as const

async function nearbyCount(explorer: Explorer): Promise<{ name: string; count: number } | undefined> {
  const button = (await explorer.hud.list('button')).find((entry) => NEARBY.test(entry.name))
  return button === undefined ? undefined : { name: button.name, count: Number(button.name.match(NEARBY)![1]) }
}

/** The `View <name>` buttons: the overlay's rows (a message's sender picture is one too, so the
 *  rows are counted against those shown before the overlay opened). */
async function viewButtons(explorer: Explorer): Promise<number> {
  return (await explorer.hud.list('button')).filter((entry) => entry.name.startsWith('View ')).length
}

scenario("the chat's nearby count matches the engine's connected players", async ({ explorer, t, restore }) => {
  restore(() => closeChat(explorer))
  await openChat(explorer)
  const agreed = await until(
    async () => ({ hud: await nearbyCount(explorer), engine: await explorer.connectedPlayers() }),
    (read) => read.hud !== undefined && read.hud.count === read.engine.length,
    { timeoutMs: AGREE_TIMEOUT_MS, what: "the chat's `<n> nearby` to match /connected_players" }
  )
  t.diagnostic(`${agreed.hud!.count} nearby; engine: ${agreed.engine.length} connected`)
  await closeChat(explorer)
})

scenario('the nearby button opens the members overlay with a row per person, and Back returns', async ({ explorer, t, restore }) => {
  restore(() => closeChat(explorer))
  await openChat(explorer)
  const count = await until(() => nearbyCount(explorer), (read) => read !== undefined, { timeoutMs: TEXT_TIMEOUT_MS, what: 'the `<n> nearby` button' })
  const before = await viewButtons(explorer)
  await explorer.hud.click({ name: count!.name, role: 'button' })
  await explorer.hud.wait(BACK, { timeoutMs: TEXT_TIMEOUT_MS })
  // the overlay replaces the title bar and its button: the rows follow the count read before
  const views = await until(() => viewButtons(explorer), (n) => n >= count!.count, {
    timeoutMs: AGREE_TIMEOUT_MS,
    what: `a View row for each of the ${count!.count} people counted`
  })
  t.diagnostic(`${count!.count} nearby, ${views} View buttons with the overlay open (${before} before)`)
  await explorer.hud.click(BACK)
  await explorer.hud.waitHidden(BACK, TEXT_TIMEOUT_MS)
  assert.ok((await nearbyCount(explorer)) !== undefined, 'Back should return to the Nearby messages and their title bar')
  await closeChat(explorer)
})
