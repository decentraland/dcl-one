// explorer-automation `CommunitiesTests` (4): the Communities section's content, invites view,
// search and community detail. The Bevy page is read-only here: no scenario creates, joins or
// leaves a community. Its titles ("My Communities", "Browse Communities (N)"), its Invites &
// Requests label and its empty messages are text without a role, so the HUD commands cannot read
// them. A card is a button named by its whole text,
// `[initial] <name> [owner] Public · <n> Members (Join|View)`, so a community's name is read as a
// part of the card's name. Opening a card clicks its cover, near the card's top: the card's centre
// can be its Join button.
//
// | Unity test                    | Here                                                                 | Class   |
// | TestCommunitiesPanelContent   | CREATE A COMMUNITY, the search field and a card; not the Invites & Requests entry, the two titles or the count | Partial |
// | TestOpenInvitesAndRequests    | Invites & Requests is a label, and there is no such view             | Gap     |
// | TestSearchCommunities         | the query is a word of the first card read, not "Decentraland"; a query nothing matches empties the grid; no results title, count or Back button | Partial |
// | TestOpenCommunityDetail       | the detail's name is part of the card's name, not equal to it; the ANNOUNCEMENTS tab stands for the sections and there is no private notice | Partial |
import assert from 'node:assert/strict'
import { settled, spec, until, type Explorer } from '../driver/index.ts'
import { PAGES, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000

const SEARCH = { name: 'Search', role: 'textbox' } as const
const CREATE = { name: '+ CREATE A COMMUNITY', role: 'button' } as const
const DETAIL = { name: 'Community sections', role: 'tablist' } as const
const ANNOUNCEMENTS = { name: 'ANNOUNCEMENTS', role: 'tab' } as const
const CARD_TAIL = /\s*Public\s*·\s*\d+\s*Members\s+(?:Join|View)$/
/** Unity tries up to five cards until one opens. */
const CARDS_TRIED = 5

/** The names of the community cards on the page. */
async function communityCards(explorer: Explorer): Promise<string[]> {
  const buttons = await explorer.hud.list('button')
  return buttons.map((button) => button.name).filter((name) => CARD_TAIL.test(name))
}

/** The cards once the live communities list has arrived and stopped changing. */
async function waitForCards(explorer: Explorer): Promise<string[]> {
  await until(() => communityCards(explorer), (cards) => cards.length > 0, { timeoutMs: CONTROL_TIMEOUT_MS, what: 'the community cards to load' })
  return settled(() => communityCards(explorer), { timeoutMs: TEXT_TIMEOUT_MS, intervalMs: 500, what: 'the community cards to stop changing' })
}

/** The first word of the community's name, read from a card's name. Without a cover image the
 *  card starts with the name's initial, which is dropped when the next word starts with it. */
function searchWordOf(cardName: string): string {
  const words = cardName.replace(CARD_TAIL, '').split(' ')
  if (words.length > 1 && words[0]!.length === 1 && words[1]!.toLowerCase().startsWith(words[0]!.toLowerCase())) words.shift()
  return words[0]!
}

/** Click a card's cover and say whether the detail opened. */
async function openCard(explorer: Explorer, card: string): Promise<string | undefined> {
  try {
    const target = await explorer.hud.wait({ name: card, role: 'button' }, { state: 'actionable', timeoutMs: TEXT_TIMEOUT_MS })
    if (!target.found) return `"${card}" was not found`
    await explorer.clickAt(target.x + target.width / 2, target.y + Math.min(24, target.height / 4))
    await explorer.hud.wait(DETAIL, { timeoutMs: TEXT_TIMEOUT_MS })
    return undefined
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}

scenario('the Communities page shows its create button, search and community cards', async ({ explorer }) => {
  await openWithKey(explorer, 'communities')
  assert.ok(await explorer.hud.isVisible(CREATE), 'Left column should offer the CREATE A COMMUNITY button')
  assert.ok(await explorer.hud.isVisible(SEARCH), 'The page should offer the community search')
  const cards = await waitForCards(explorer)
  assert.ok(cards[0]!.replace(CARD_TAIL, '').trim().length > 0, 'The browse grid should contain community cards with a name')
  await closePage(explorer, 'communities')
})

scenario('the Invites & Requests view opens and Back returns to the grid', async () => {}, {
  todo: 'Invites & Requests is a label in the left column of the Bevy Communities page, not a control, and the HUD has no invites or requests view'
})

scenario('searching communities filters the cards and emptying the search restores them', async ({ explorer }) => {
  await openWithKey(explorer, 'communities')
  const all = await waitForCards(explorer)
  const query = searchWordOf(all[0]!)
  assert.ok(query.length > 0, `no search word in the first card "${all[0]}"`)

  const typed = await explorer.hud.fill(SEARCH, query)
  assert.equal(typed.value, query, 'the search field should hold the typed query')
  const lowered = query.toLowerCase()
  await until(() => communityCards(explorer), (cards) => cards.length > 0 && cards.every((name) => name.toLowerCase().includes(lowered)), {
    timeoutMs: TEXT_TIMEOUT_MS,
    what: `only community cards matching "${query}"`
  })

  await explorer.hud.fill(SEARCH, `no-community-called-${Date.now()}`)
  await until(() => communityCards(explorer), (cards) => cards.length === 0, { timeoutMs: TEXT_TIMEOUT_MS, what: 'no community cards for a name nothing matches' })

  await explorer.hud.fill(SEARCH, '')
  await until(() => communityCards(explorer), (cards) => cards.length === all.length, { timeoutMs: TEXT_TIMEOUT_MS, what: 'every community card to return when the search is emptied' })
  assert.equal(await explorer.hud.value(SEARCH), '', 'the search field should be empty')
  await explorer.escape()
  await closePage(explorer, 'communities')
})

scenario('a community card opens its detail with the same name', async ({ explorer }) => {
  await openWithKey(explorer, 'communities')
  const cards = await waitForCards(explorer)

  let opened: string | undefined
  const failures: string[] = []
  for (const card of cards.slice(0, CARDS_TRIED)) {
    const failure = await openCard(explorer, card)
    if (failure === undefined) {
      opened = card
      break
    }
    failures.push(failure)
  }
  assert.ok(opened !== undefined, `Clicking a community card should open the community detail popup: ${failures.join('; ')}`)

  // The page's own heading comes first; the detail's is the name of the community.
  const headings = await until(() => explorer.hud.list('heading'), (list) => list.length > 1, { timeoutMs: TEXT_TIMEOUT_MS, what: "the community detail's name" })
  const name = headings.at(-1)!.name
  assert.ok(opened.includes(name), `Community detail should show the clicked community's name: card "${opened}", detail "${name}"`)
  assert.ok(await explorer.hud.isVisible(ANNOUNCEMENTS), 'Community detail should show its section tabs')

  // The page's Close button comes first in the document and the detail's last.
  const buttons = await explorer.hud.list('button')
  const closes = buttons.filter((button) => button.name === 'Close').length
  await explorer.hud.click({ name: 'Close', role: 'button', index: closes - 1 })
  await explorer.hud.waitHidden(DETAIL, CONTROL_TIMEOUT_MS)
  assert.ok(await explorer.hud.isVisible(PAGES.communities.marker), 'Closing the detail should leave the Communities page open')
  await closePage(explorer, 'communities')
})
