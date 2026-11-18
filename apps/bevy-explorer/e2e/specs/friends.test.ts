// QA Automation Suite, High End Functionality / FRIENDS: the Friends panel, opened by L or the
// sidebar rail's `Friends` button, beside the rail (library/ui/apps/hud/features/friends/
// FriendsPanel.tsx). Its header is the `Friends sections` tablist (Friends, Requests, Blocked)
// and the `Close friends` button. The Friends tab groups friends under `Online (n)` and
// `Offline (n)`, collapsible buttons (named `Online ( n )` as the page joins their text nodes), and shows a placeholder with no role when the account has
// no friend; the Requests tab groups requests under `Received (n)` and `Sent (n)`; the Blocked tab
// lists blocked accounts or shows a placeholder. A tab's selection (`aria-selected`) is not
// readable through `/hud_read`, so a switch is proven by the controls only the new tab shows.
//
// | QA case                 | Here                                                                 | Class   |
// | Open Friends Panel      | L opens the panel and closes it; the rail's Friends button opens it and Close friends closes it | Port    |
// | Friends Panel Content   | the three tabs; Requests shows Received and Sent with their counts; Friends shows Online and Offline when the account has friends (the empty placeholder has no role); the rows' Jump In and menu need a friend online | Partial |
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const PANEL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000

const SECTIONS: Selector = { name: 'Friends sections', role: 'tablist' }
const CLOSE: Selector = { name: 'Close friends', role: 'button' }
const RAIL: Selector = { name: 'Friends', role: 'button' }
const FRIENDS_TAB: Selector = { name: 'Friends', role: 'tab' }
/** The Requests tab carries the received requests' count badge in its name when there are any. */
const REQUESTS_TAB: Selector = { name: 'Requests', role: 'tab', exact: false }
const BLOCKED_TAB: Selector = { name: 'Blocked', role: 'tab' }
/** A group's toggle, named by its title and count as the page joins their text nodes: `Received ( 0 )`. */
const RECEIVED = /^Received \(\s*(\d+)\s*\)$/
const SENT = /^Sent \(\s*(\d+)\s*\)$/
const ONLINE = /^Online \(\s*(\d+)\s*\)$/
const OFFLINE = /^Offline \(\s*(\d+)\s*\)$/

async function buttonNames(explorer: Explorer): Promise<string[]> {
  return (await explorer.hud.list('button')).map((button) => button.name)
}

async function waitClosed(explorer: Explorer): Promise<void> {
  await explorer.hud.waitHidden(SECTIONS, PANEL_TIMEOUT_MS)
}

scenario('L opens the Friends panel and L again closes it', async ({ explorer, restore }) => {
  restore(async () => {
    if (await explorer.hud.isVisible(CLOSE)) await explorer.hud.click(CLOSE)
  })
  await explorer.key('KeyL')
  await explorer.hud.wait(SECTIONS, { timeoutMs: PANEL_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(CLOSE), 'the panel should offer Close friends')
  await explorer.key('KeyL')
  await waitClosed(explorer)
})

scenario("the rail's Friends button opens the Friends panel and Close friends closes it", async ({ explorer, restore }) => {
  restore(async () => {
    if (await explorer.hud.isVisible(CLOSE)) await explorer.hud.click(CLOSE)
  })
  await explorer.hud.click(RAIL)
  await explorer.hud.wait(SECTIONS, { timeoutMs: PANEL_TIMEOUT_MS })
  await until(() => explorer.hud.get(RAIL), (rail) => rail.checked === true, { timeoutMs: TEXT_TIMEOUT_MS, what: 'the rail button to read pressed while the panel is open' })
  await explorer.hud.click(CLOSE)
  await waitClosed(explorer)
})

scenario('the Friends panel has Friends, Requests and Blocked tabs, and Requests shows Received and Sent', async ({ explorer, t, restore }) => {
  restore(async () => {
    if (await explorer.hud.isVisible(CLOSE)) await explorer.hud.click(CLOSE)
  })
  await explorer.hud.click(RAIL)
  await explorer.hud.wait(SECTIONS, { timeoutMs: PANEL_TIMEOUT_MS })
  for (const tab of [FRIENDS_TAB, REQUESTS_TAB, BLOCKED_TAB]) {
    await explorer.hud.wait(tab, { timeoutMs: TEXT_TIMEOUT_MS })
  }

  // the panel opens on Friends: its groups show when the account has friends
  const friends = await buttonNames(explorer)
  const online = friends.find((name) => ONLINE.test(name))
  const offline = friends.find((name) => OFFLINE.test(name))
  if (online !== undefined || offline !== undefined) {
    assert.ok(online !== undefined && offline !== undefined, `the Friends tab should group friends as Online and Offline: ${JSON.stringify(friends)}`)
  } else {
    t.diagnostic('the account has no friends: the Friends tab shows its placeholder, which has no role')
  }

  await explorer.hud.click(REQUESTS_TAB)
  const requests = await until(() => buttonNames(explorer), (names) => names.some((name) => RECEIVED.test(name)) && names.some((name) => SENT.test(name)), {
    timeoutMs: TEXT_TIMEOUT_MS,
    what: 'the Requests tab to show Received (n) and Sent (n)'
  })
  assert.ok(!requests.some((name) => ONLINE.test(name)), 'the Friends tab groups should not show on Requests')
  const received = Number(requests.find((name) => RECEIVED.test(name))!.match(RECEIVED)![1])
  const tab = await explorer.hud.get(REQUESTS_TAB)
  if (received > 0) assert.ok(/\d/.test(tab.name), `the Requests tab should carry the received count ${received}: "${tab.name}"`)
  t.diagnostic(`requests: ${requests.filter((name) => RECEIVED.test(name) || SENT.test(name)).join(', ')}`)

  await explorer.hud.click(BLOCKED_TAB)
  await until(() => buttonNames(explorer), (names) => !names.some((name) => RECEIVED.test(name) || SENT.test(name)), {
    timeoutMs: TEXT_TIMEOUT_MS,
    what: 'the Blocked tab to replace the request groups'
  })

  await explorer.hud.click(FRIENDS_TAB)
  await until(() => buttonNames(explorer), (names) => !names.some((name) => RECEIVED.test(name)), { timeoutMs: TEXT_TIMEOUT_MS, what: 'the Friends tab back' })
  await explorer.hud.click(CLOSE)
  await waitClosed(explorer)
})
