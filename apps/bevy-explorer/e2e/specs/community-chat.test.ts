// Community text chat, beyond explorer-automation:
// a community the player is a member of opens its chat from the community card, the chat shows
// the community's tab and title bar, and a message typed there goes to the engine as a community
// message (`/community_state` counts it; the router's fan-out to other members is not observable
// from one client). The scenario uses a community the automation account is a member of, found
// under My Communities (VIEW ALL) wherever the browse order puts it. An account in none joins a
// public one from the browse grid (one named as a test's when there is one), and leaves it again
// once the scenario is done, so the account ends as it started.
import assert from 'node:assert/strict'
import { closeToRest, spec, until, type Explorer } from '../driver/index.ts'
import { cardSelector, closePage, openWithKey, withoutInitial } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000
/** The most a sent message is given to reach the engine's community counters: a ceiling, the wait
 *  ends when it is counted. */
const SEND_TIMEOUT_MS = 10_000

const DETAIL = { name: 'Community sections', role: 'tablist' } as const
const OPEN_CHAT = { name: 'Open chat', role: 'button' } as const
const CONVERSATIONS = { name: 'Conversations', role: 'navigation' } as const
const OPTIONS = { name: 'Conversation options', role: 'button' } as const
const INPUT_FOCUSED = { name: 'Write a message', role: 'textbox' } as const
const INPUT_IDLE = { name: 'Press Enter to chat', role: 'textbox' } as const
/** The left column's toggle between the browse grid and My Communities, shown once the user is a
 *  member of one: My Communities lists only the user's own, wherever the browse order puts them. */
const VIEW_ALL = { name: 'VIEW ALL ›', role: 'button' } as const
const BACK_TO_BROWSE = { name: '‹ BACK', role: 'button' } as const
/** A member's card ends in View; a card the player could join ends in Join. */
const MEMBER_CARD = /\s*Public\s*·\s*\d+\s*Members\s+View$/
const JOIN_CARD = /\s*Public\s*·\s*\d+\s*Members\s+Join$/
/** What names a card whatever its member count and button: its name and owner. */
const keyOf = (card: string): string => withoutInitial(card).replace(/\s*Public\s*·\s*\d+\s*Members\s+(?:Join|View)$/, '')
const MORE = { name: 'More', role: 'button' } as const
const LEAVE = { name: 'Leave community', role: 'button' } as const

/** The member cards' names, without the initial a cover shows for an image that did not load. */
async function memberCards(explorer: Explorer): Promise<string[]> {
  const buttons = await explorer.hud.list('button')
  return buttons.map((button) => button.name).filter((name) => MEMBER_CARD.test(name)).map(withoutInitial)
}

/** The community cards once the communities list has arrived: the engine sends it whole, in one
 *  message. */
async function waitForCards(explorer: Explorer): Promise<void> {
  await until(() => explorer.hud.list('button'), (buttons) => buttons.some((b) => /Members\s+(?:Join|View)$/.test(b.name)), { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the community cards to load' })
}

/** The member cards, from My Communities; none when the account is a member of no community. */
async function myCommunities(explorer: Explorer): Promise<string[]> {
  if (await explorer.hud.isVisible(VIEW_ALL)) {
    await explorer.hud.click(VIEW_ALL)
    await explorer.hud.wait(BACK_TO_BROWSE, { timeoutMs: TEXT_TIMEOUT_MS })
  }
  return memberCards(explorer)
}

/** A public community of the browse grid to join: one named as a test's when there is one, else
 *  the first. Its card's Join is the button of that name at the card's place among the Join
 *  cards, each holding one. */
async function joinable(explorer: Explorer): Promise<{ card: string; index: number } | undefined> {
  const joins = (await explorer.hud.list('button')).map((button) => button.name).filter((name) => JOIN_CARD.test(name))
  if (joins.length === 0) return undefined
  const index = Math.max(0, joins.findIndex((name) => /\btest\b/i.test(name)))
  return { card: keyOf(joins[index]!), index }
}

/** Open a community's card from the Communities page: its cover, near the card's top (the card's
 *  centre can be its button). */
async function openCard(explorer: Explorer, card: string): Promise<void> {
  const target = await explorer.hud.wait(cardSelector(card), { state: 'actionable', timeoutMs: TEXT_TIMEOUT_MS })
  assert.ok(target.found, `"${card}" was not found`)
  await explorer.clickAt(target.x + target.width / 2, target.y + Math.min(24, target.height / 4))
  await explorer.hud.wait(DETAIL, { timeoutMs: TEXT_TIMEOUT_MS })
}

/** Leave a community the scenario joined, through its detail's More menu, and wait for its card
 *  to stop being a member's (it offers Join again, or leaves My Communities). */
async function leave(explorer: Explorer, card: string): Promise<void> {
  await closeToRest(explorer)
  await openWithKey(explorer, 'communities')
  await waitForCards(explorer)
  await openCard(explorer, card)
  await explorer.hud.click(MORE)
  await explorer.hud.click(LEAVE)
  await until(() => explorer.hud.list('button'), (buttons) => !buttons.some((b) => MEMBER_CARD.test(b.name) && keyOf(b.name) === card), {
    timeoutMs: CONTROL_TIMEOUT_MS,
    what: `the card of "${card}" to be no member's`
  })
  await closePage(explorer, 'communities')
}

scenario("a member's community card opens its chat, whose tab and title bar name the community, and a message sent there is a community message", async ({ explorer, restore }) => {
  await openWithKey(explorer, 'communities')
  await waitForCards(explorer)
  let cards = await myCommunities(explorer)
  if (cards.length === 0) {
    const join = await joinable(explorer)
    if (join === undefined) {
      await closePage(explorer, 'communities')
      assert.fail('the automation account is a member of no community and the browse grid offers none to join, so community chat cannot be exercised')
    }
    restore(() => leave(explorer, join.card))
    await explorer.hud.click({ name: 'Join', role: 'button', index: join.index })
    // the HUD reads the list again once the join is written: My Communities appears with it
    await explorer.hud.wait(VIEW_ALL, { timeoutMs: CONTROL_TIMEOUT_MS })
    cards = (await myCommunities(explorer)).filter((name) => keyOf(name) === join.card)
    assert.ok(cards.length > 0, `the joined community "${join.card}" should be under My Communities`)
  }
  const card = cards[0]!
  await openCard(explorer, card)
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
  assert.ok(state.routerInRoom, 'the message router should be in the private chat room, or nothing sent can reach anyone')

  // a message typed in the community's chat is counted as a community message sent
  const text = `community hello ${Date.now()}`
  const typed = await explorer.hud.fill((await explorer.hud.isVisible(INPUT_FOCUSED)) ? INPUT_FOCUSED : INPUT_IDLE, text)
  assert.equal(typed.value, text, 'the chat input should hold the typed message')
  const before = state.sent ?? 0
  // the fill returned once the input had the keyboard, so the Enter is the input's: it empties as
  // the HUD hands the message to the engine
  await explorer.key('Enter')
  const value = async (): Promise<string | undefined> => ((await explorer.hud.find(INPUT_FOCUSED)) ?? (await explorer.hud.find(INPUT_IDLE)))?.value
  await until(value, (held) => held === '', { timeoutMs: TEXT_TIMEOUT_MS, what: 'the chat input to empty as it sends the community message' })
  await until(() => explorer.communityState(), (read) => (read.sent ?? 0) > before, { timeoutMs: SEND_TIMEOUT_MS, what: 'the community message to be sent' })
  // and the engine stored it under the community's channel
  const channel = state.channels.map((id) => `community:${id}`)
  const stored = await Promise.all(channel.map((c) => explorer.dmHistory(c, 5)))
  assert.ok(stored.some((lines) => lines.some((line) => line.endsWith(text))), 'the sent message should be in the community\'s stored history')
})
