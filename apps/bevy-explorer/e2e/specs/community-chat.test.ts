// Community text chat, beyond explorer-automation (Unity's suite has no community chat tests):
// a community the player is a member of opens its chat from the community card, the chat shows
// the community's tab and title bar, and a message typed there goes to the engine as a community
// message (`/community_state` counts it; the router's fan-out to other members is not observable
// from one client). The automation account must be a member of a community whose card is in the
// browse grid (a card ending in `View`); otherwise the scenario reports that and stops.
import assert from 'node:assert/strict'
import { settled, spec, until, type Explorer } from '../driver/index.ts'
import { closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000
const SEND_ATTEMPTS = 3
const SEND_ATTEMPT_MS = 3000

const DETAIL = { name: 'Community sections', role: 'tablist' } as const
const OPEN_CHAT = { name: 'Open chat', role: 'button' } as const
const CONVERSATIONS = { name: 'Conversations', role: 'navigation' } as const
const OPTIONS = { name: 'Conversation options', role: 'button' } as const
const INPUT_FOCUSED = { name: 'Write a message', role: 'textbox' } as const
const INPUT_IDLE = { name: 'Press Enter to chat', role: 'textbox' } as const
/** A member's card ends in View; a card the player could join ends in Join. */
const MEMBER_CARD = /\s*Public\s*·\s*\d+\s*Members\s+View$/

async function memberCards(explorer: Explorer): Promise<string[]> {
  const buttons = await explorer.hud.list('button')
  return buttons.map((button) => button.name).filter((name) => MEMBER_CARD.test(name))
}

/** The member cards once the live list has arrived and stopped changing; none when the account
 *  is in no listed community. */
async function waitForMemberCards(explorer: Explorer): Promise<string[]> {
  await until(() => explorer.hud.list('button'), (buttons) => buttons.some((b) => /Members\s+(?:Join|View)$/.test(b.name)), { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the community cards to load' })
  return settled(() => memberCards(explorer), { timeoutMs: TEXT_TIMEOUT_MS, intervalMs: 500, what: 'the member cards to stop changing' })
}

scenario("a member's community card opens its chat, whose tab and title bar name the community, and a message sent there is a community message", async ({ explorer }) => {
  await openWithKey(explorer, 'communities')
  const cards = await waitForMemberCards(explorer)
  if (cards.length === 0) {
    await closePage(explorer, 'communities')
    assert.fail('the automation account is a member of no community in the browse grid (no card ends in View), so community chat cannot be exercised')
  }
  const card = cards[0]!
  const target = await explorer.hud.wait({ name: card, role: 'button' }, { state: 'actionable', timeoutMs: TEXT_TIMEOUT_MS })
  assert.ok(target.found, `"${card}" was not found`)
  await explorer.clickAt(target.x + target.width / 2, target.y + Math.min(24, target.height / 4))
  await explorer.hud.wait(DETAIL, { timeoutMs: TEXT_TIMEOUT_MS })
  await explorer.hud.click(OPEN_CHAT)

  // the modal and the page close, the chat opens on the community's tab
  await explorer.hud.wait(CONVERSATIONS, { timeoutMs: CONTROL_TIMEOUT_MS })
  const tabs = (await explorer.hud.list('button')).map((b) => b.name).filter((n) => /^Chat in /.test(n))
  let pressed: string | undefined
  for (const tab of tabs) {
    if ((await explorer.hud.get({ name: tab, role: 'button' })).checked === true) pressed = tab
  }
  assert.ok(pressed, `no pressed community tab among ${JSON.stringify(tabs)}`)
  const name = pressed.replace(/^Chat in /, '')
  assert.ok(card.includes(name), `the tab "${name}" should name the opened community "${card}"`)
  assert.ok(await explorer.hud.isVisible(OPTIONS), 'the community title bar should offer the conversation options')
  const state = await explorer.communityState()
  assert.ok(state.channelCount > 0, 'the engine should listen to the opened community')

  // a message typed in the community's chat is counted as a community message sent
  const text = `community hello ${Date.now()}`
  const typed = await explorer.hud.fill((await explorer.hud.isVisible(INPUT_FOCUSED)) ? INPUT_FOCUSED : INPUT_IDLE, text)
  assert.equal(typed.value, text, 'the chat input should hold the typed message')
  const before = state.sent ?? 0
  let sent: number | undefined
  for (let attempt = 0; attempt < SEND_ATTEMPTS && sent === undefined; attempt++) {
    await explorer.key('Enter')
    sent = await until(() => explorer.communityState(), (read) => (read.sent ?? 0) > before, { timeoutMs: SEND_ATTEMPT_MS, what: 'the community message to be sent' })
      .then((read) => read.sent)
      .catch(() => undefined)
  }
  assert.ok(sent !== undefined && sent > before, `the community message was not sent after ${SEND_ATTEMPTS} attempts`)
  // and the engine stored it under the community's channel
  const channel = state.channels.map((id) => `community:${id}`)
  const stored = await Promise.all(channel.map((c) => explorer.dmHistory(c, 5)))
  assert.ok(stored.some((lines) => lines.some((line) => line.endsWith(text))), 'the sent message should be in the community\'s stored history')
})
