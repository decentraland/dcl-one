// QA Automation Suite, High End Functionality / COMMUNITIES: creating, joining and browsing a
// community from the Communities page (library/ui/apps/hud/features/communities). A card is a button
// named by its whole text, ending `(Public|Private) · <n> Members (Join|View|Request to Join|Cancel
// join request|…)` (see specs/communities.test.ts); its detail is a popup with the community's name
// as a heading, Join (`Joined` once a member), `Request to Join` (`Cancel join request` once sent)
// for a private one, `Open chat` for a member, the `More` menu (Copy link, Leave community) and the
// `Community sections` tabs (ANNOUNCEMENTS, MEMBERS, PLACES, PHOTOS). A member row is `View <name>'s
// profile` with `More options for <name>`, which opens the member's profile card (`Profile` dialog:
// View Profile, Chat, Call, Block, Report). `+ CREATE A COMMUNITY` opens the `Create a Community`
// form when the account owns a NAME, and the `Get a NAME` gate otherwise.
//
// | QA case                    | Here                                                             | Class   |
// | Create a Community         | the form's CREATE stays disabled until name and description are filled; CANCEL asks to discard and closes it, nothing is created. An account with no NAME gets the Get a NAME gate, whose Maybe later closes it | Partial |
// | Join a community           | Join on a public card makes the account a member (the card says View, My Communities lists it) and the chat's conversations gain the community's tab; leaving restores | Port    |
// | Private community          | a private card's detail shows its name, Request to Join and no Open chat; the members it lists are reported | Partial |
// | Join a Private Community   | Request to Join turns into Cancel join request; cancelling restores Request to Join | Partial |
// | Community interaction      | a public community's MEMBERS rows: a member's More options opens their card with View Profile, Chat and Call; View <name>'s profile opens the passport | Partial |
//
// Partial: Create a Community never submits (it would publish a community from the shared account
// each run), and the image, privacy and places fields are not exercised. Private community: the
// description and privacy line are text with no role. Join a Private Community: the "request sent"
// toast has no role; approval needs the community's owner. Community interaction: adding a friend
// from a row and the friends-in-community list need a second account.
//
// Every scenario that changes membership registers the undo first: leaving a joined community,
// cancelling a sent request.
import assert from 'node:assert/strict'
import { closeToRest, spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { cardSelector, closePage, openFromRail, withoutInitial } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000

const CREATE: Selector = { name: '+ CREATE A COMMUNITY', role: 'button' }
const FORM: Selector = { name: 'Create a Community', role: 'dialog' }
const GATE: Selector = { name: 'Get a NAME', role: 'dialog' }
const MAYBE_LATER: Selector = { name: 'Maybe later', role: 'button' }
const NAME_FIELD: Selector = { name: 'COMMUNITY NAME', role: 'textbox', exact: false }
const DESCRIPTION_FIELD: Selector = { name: 'DESCRIPTION', role: 'textbox', exact: false }
const CREATE_BUTTON: Selector = { name: 'CREATE', role: 'button' }
const CANCEL_BUTTON: Selector = { name: 'CANCEL', role: 'button' }
const DISCARD: Selector = { name: 'Discard', role: 'button' }

const DETAIL: Selector = { name: 'Community sections', role: 'tablist' }
const MEMBERS_TAB: Selector = { name: 'MEMBERS', role: 'tab' }
const OPEN_CHAT: Selector = { name: 'Open chat', role: 'button' }
/** The member's disabled Joined button: an aria-hidden check icon and `Joined` (CommunityModal.tsx). */
const JOINED: Selector = { name: 'Joined', role: 'button' }
const MORE: Selector = { name: 'More', role: 'button' }
const LEAVE: Selector = { name: 'Leave community', role: 'button' }
const PROFILE_CARD: Selector = { name: 'Profile', role: 'dialog' }
const PASSPORT: Selector = { name: 'Passport sections', role: 'tablist' }
const CONVERSATIONS: Selector = { name: 'Conversations', role: 'navigation' }
const RAIL_CHAT: Selector = { name: 'Chat', role: 'button' }
const CLOSE_CHAT: Selector = { name: 'Close chat', role: 'button' }

const VIEW_ALL: Selector = { name: 'VIEW ALL ›', role: 'button' }
const BACK_TO_BROWSE: Selector = { name: '‹ BACK', role: 'button' }
const ANY_CARD = /\s*(?:Public|Private)\s*·\s*\d+\s*Members\s+(?:Join|View|Request to Join|Cancel join request|Accept invite\s+Reject invite)$/
const JOIN_CARD = /\s*Public\s*·\s*\d+\s*Members\s+Join$/
const PRIVATE_CARD = /\s*Private\s*·\s*\d+\s*Members\s+Request to Join$/
const MEMBER_CARD = /\s*(?:Public|Private)\s*·\s*\d+\s*Members\s+View$/
/** What names a card whatever its member count and button: its name and owner. */
const keyOf = (card: string): string => withoutInitial(card).replace(ANY_CARD, '')
const MEMBER_ROW = /^View (.+)'s profile$/

async function buttons(explorer: Explorer): Promise<string[]> {
  return (await explorer.hud.list('button')).map((button) => button.name)
}

async function waitForCards(explorer: Explorer): Promise<string[]> {
  return until(() => buttons(explorer), (names) => names.some((name) => ANY_CARD.test(name)), { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the community cards to load' })
}

/** Open a card's detail by its cover, near the card's top (the card's centre can be its button). */
async function openCard(explorer: Explorer, card: string): Promise<void> {
  const target = await explorer.hud.wait(cardSelector(card), { state: 'actionable', timeoutMs: TEXT_TIMEOUT_MS })
  assert.ok(target.found, `"${card}" was not found`)
  await explorer.clickAt(target.x + target.width / 2, target.y + Math.min(24, target.height / 4))
  await explorer.hud.wait(DETAIL, { timeoutMs: TEXT_TIMEOUT_MS })
}

/** The detail's own Close: the page's comes first in the document, the popup's last. */
async function closeDetail(explorer: Explorer): Promise<void> {
  const closes = (await buttons(explorer)).filter((name) => name === 'Close').length
  await explorer.hud.click({ name: 'Close', role: 'button', index: closes - 1 })
  await explorer.hud.waitHidden(DETAIL, CONTROL_TIMEOUT_MS)
}

/** The detail's button of that name: the grid's cards behind the popup carry the same Join,
 *  Request to Join and Cancel join request buttons, and the popup comes last in the document. */
async function detailButton(explorer: Explorer, name: string): Promise<Selector> {
  const count = await until(async () => (await buttons(explorer)).filter((n) => n === name).length, (n) => n > 0, { timeoutMs: CONTROL_TIMEOUT_MS, what: `the detail's ${name}` })
  return { name, role: 'button', index: count - 1 }
}

/** A card of the browse grid matching `pattern`: one named as a test's when there is one, else the first. */
async function cardMatching(explorer: Explorer, pattern: RegExp): Promise<string | undefined> {
  const cards = (await waitForCards(explorer)).filter((name) => pattern.test(name)).map(withoutInitial)
  if (cards.length === 0) return undefined
  return cards.find((name) => /\btest\b/i.test(name)) ?? cards[0]
}

/** Leave a community through its detail's More menu, from a fresh Communities page. */
async function leave(explorer: Explorer, key: string): Promise<void> {
  await closeToRest(explorer)
  await openFromRail(explorer, 'communities')
  await waitForCards(explorer)
  if (await explorer.hud.isVisible(VIEW_ALL)) {
    await explorer.hud.click(VIEW_ALL)
    await explorer.hud.wait(BACK_TO_BROWSE, { timeoutMs: TEXT_TIMEOUT_MS })
  }
  const card = (await buttons(explorer)).find((name) => MEMBER_CARD.test(name) && keyOf(name) === key)
  if (card === undefined) {
    await closePage(explorer, 'communities')
    return
  }
  await openCard(explorer, withoutInitial(card))
  await explorer.hud.click(MORE)
  await explorer.hud.click(LEAVE)
  await until(() => buttons(explorer), (names) => !names.some((name) => MEMBER_CARD.test(name) && keyOf(name) === key), {
    timeoutMs: CONTROL_TIMEOUT_MS,
    what: `"${key}" to stop being a member's card`
  })
  await closePage(explorer, 'communities')
}

scenario("the create form keeps CREATE disabled until it is filled, and CANCEL discards it (or the NAME gate shows)", async ({ explorer, t }) => {
  await openFromRail(explorer, 'communities')
  await explorer.hud.click(CREATE)
  const opened = await until(
    async () => ((await explorer.hud.isVisible(FORM)) ? 'form' : (await explorer.hud.isVisible(GATE)) ? 'gate' : undefined),
    (which) => which !== undefined,
    { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the create form or the Get a NAME gate' }
  )
  if (opened === 'gate') {
    t.diagnostic('the account owns no NAME: the gate shows instead of the form, which is not exercised')
    assert.ok(await explorer.hud.isVisible({ name: 'Get a NAME', role: 'button' }), 'the gate should offer Get a NAME')
    await explorer.hud.click(MAYBE_LATER)
    await explorer.hud.waitHidden(GATE, TEXT_TIMEOUT_MS)
    await closePage(explorer, 'communities')
    return
  }
  await explorer.hud.wait(NAME_FIELD, { state: 'enabled', timeoutMs: CONTROL_TIMEOUT_MS })
  assert.equal((await explorer.hud.get(CREATE_BUTTON)).disabled, true, 'CREATE should be disabled while the form is empty')
  await explorer.hud.fill(NAME_FIELD, `automation ${Date.now()}`)
  assert.equal((await explorer.hud.get(CREATE_BUTTON)).disabled, true, 'CREATE should stay disabled without a description')
  await explorer.hud.fill(DESCRIPTION_FIELD, 'An automated check of the create form; never submitted.')
  await until(() => explorer.hud.get(CREATE_BUTTON), (button) => !button.disabled, { timeoutMs: TEXT_TIMEOUT_MS, what: 'CREATE to enable once the mandatory fields are filled' })
  await explorer.hud.click(CANCEL_BUTTON)
  await explorer.hud.click(DISCARD)
  await explorer.hud.waitHidden(FORM, TEXT_TIMEOUT_MS)
  await closePage(explorer, 'communities')
})

scenario("joining a public community makes the account a member and adds the community's chat", async ({ explorer, restore }) => {
  await openFromRail(explorer, 'communities')
  const card = await cardMatching(explorer, JOIN_CARD)
  assert.ok(card !== undefined, 'the browse grid should offer a public community to join')
  const key = keyOf(card)
  restore(() => leave(explorer, key))
  await openCard(explorer, card)
  await explorer.hud.click(await detailButton(explorer, 'Join'))
  await explorer.hud.wait(JOINED, { timeoutMs: CONTROL_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(OPEN_CHAT), 'a member should be offered Open chat')
  await closeDetail(explorer)
  await until(() => buttons(explorer), (names) => names.some((name) => MEMBER_CARD.test(name) && keyOf(name) === key), {
    timeoutMs: CONTROL_TIMEOUT_MS,
    what: `the card of "${key}" to say View`
  })
  await closePage(explorer, 'communities')

  // the chat's conversations list the community without opening it from the card
  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.wait(CONVERSATIONS, { timeoutMs: CONTROL_TIMEOUT_MS })
  const tabs = await until(
    async () => (await buttons(explorer)).filter((name) => name.startsWith('Chat in ')),
    (list) => list.some((tab) => key.startsWith(tab.replace(/^Chat in /, ''))),
    { timeoutMs: CONTROL_TIMEOUT_MS, what: `a "Chat in" tab for "${key}"` }
  )
  assert.ok(tabs.length > 0)
  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
  await explorer.keyboardReleased()
})

scenario("a private community's card shows its name and Request to Join, and no chat", async ({ explorer, t }) => {
  await openFromRail(explorer, 'communities')
  const card = await cardMatching(explorer, PRIVATE_CARD)
  assert.ok(card !== undefined, 'the browse grid should list a private community the account can request to join')
  await openCard(explorer, card)
  const headings = await until(() => explorer.hud.list('heading'), (list) => list.length > 1, { timeoutMs: TEXT_TIMEOUT_MS, what: "the private community's name" })
  const name = headings.at(-1)!.name
  assert.ok(card.includes(name), `the detail should name the community: card "${card}", detail "${name}"`)
  assert.ok((await buttons(explorer)).includes('Request to Join'), 'a private community should offer Request to Join')
  assert.equal(await explorer.hud.isVisible(OPEN_CHAT), false, 'a non-member should not be offered the community chat')
  await explorer.hud.click(MEMBERS_TAB)
  await new Promise((resolve) => setTimeout(resolve, 2000))
  const rows = (await buttons(explorer)).filter((n) => MEMBER_ROW.test(n)).length
  t.diagnostic(`the private community lists ${rows} member row(s) to a non-member`)
  await closeDetail(explorer)
  await closePage(explorer, 'communities')
})

scenario('Request to Join on a private community sends a request, and cancelling it restores the button', async ({ explorer, restore }) => {
  await openFromRail(explorer, 'communities')
  const card = await cardMatching(explorer, PRIVATE_CARD)
  assert.ok(card !== undefined, 'the browse grid should list a private community the account can request to join')
  await openCard(explorer, card)
  const requests = async (): Promise<number> => (await buttons(explorer)).filter((n) => n === 'Request to Join').length
  const cancels = async (): Promise<number> => (await buttons(explorer)).filter((n) => n === 'Cancel join request').length
  const before = { requests: await requests(), cancels: await cancels() }
  restore(async () => {
    if ((await cancels()) > before.cancels && (await explorer.hud.isVisible(DETAIL))) {
      await explorer.hud.click(await detailButton(explorer, 'Cancel join request'))
      await until(cancels, (n) => n === before.cancels, { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the request to be cancelled' })
    }
  })
  await explorer.hud.click(await detailButton(explorer, 'Request to Join'))
  await until(cancels, (n) => n > before.cancels, { timeoutMs: CONTROL_TIMEOUT_MS, what: 'Request to Join to turn into Cancel join request' })
  await explorer.hud.click(await detailButton(explorer, 'Cancel join request'))
  await until(requests, (n) => n === before.requests, { timeoutMs: CONTROL_TIMEOUT_MS, what: 'Cancel join request to turn back into Request to Join' })
  await closeDetail(explorer)
  await closePage(explorer, 'communities')
})

scenario("a community member's menu offers View Profile, Chat and Call, and their row opens the passport", async ({ explorer, t }) => {
  await openFromRail(explorer, 'communities')
  const cards = (await waitForCards(explorer)).filter((name) => /\s*Public\s*·\s*\d+\s*Members\s+(?:Join|View)$/.test(name)).map(withoutInitial)
  assert.ok(cards.length > 0, 'the browse grid should list a public community')
  let member: string | undefined
  for (const card of cards.slice(0, 5)) {
    await openCard(explorer, card)
    await explorer.hud.click(MEMBERS_TAB)
    member = await until(
      async () => (await buttons(explorer)).find((name) => name.startsWith('More options for ')),
      (found) => found !== undefined,
      { timeoutMs: TEXT_TIMEOUT_MS, what: 'a member row with More options' }
    ).catch(() => undefined)
    if (member !== undefined) {
      t.diagnostic(`members of "${card}"`)
      break
    }
    await closeDetail(explorer)
  }
  assert.ok(member !== undefined, 'one of the first public communities should list another member')
  const name = member.replace(/^More options for /, '')
  await explorer.hud.click({ name: member, role: 'button' })
  await explorer.hud.wait(PROFILE_CARD, { timeoutMs: TEXT_TIMEOUT_MS })
  for (const action of ['View Profile', 'Chat', 'Call']) {
    assert.ok(await explorer.hud.isVisible({ name: action, role: 'button' }), `the member menu should offer ${action}`)
  }
  await explorer.escape()
  await explorer.hud.waitHidden(PROFILE_CARD, TEXT_TIMEOUT_MS)

  await explorer.hud.click({ name: `View ${name}'s profile`, role: 'button' })
  await explorer.hud.wait(PASSPORT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await closeToRest(explorer)
})
