// explorer-automation `ChatTests` (6): opening and closing the chat, sending a message, a link, the
// channel chrome and a reaction. The Bevy chat is one panel with an input that is always mounted:
// `Press Enter to chat` while it is unfocused and `Write a message` while it is focused (the
// placeholder is its only name), a `Close chat` button when the panel is open, and the rail's
// `Chat` button, whose `checked` is the panel's open state. A message's text and the Nearby title
// are text without a role, so a sent message is verified in `/chat_history` (the engine's record,
// with the player's own address from `/world_state`); a link is a `link` control. The message goes
// through the input, which takes several words where `/chat <channel> <message>` takes one.
//
// | Unity test                                  | Here                                                                                                 | Class   |
// | TestOpenAndCloseChatFromSidebar             | the rail's Chat button opens the chat with the input focused and a second click closes it             | Port    |
// | TestOpenChatWithEnterAndCloseWithEscape     | Enter opens it with the input focused; Escape leaves the input, it does not close the panel          | Partial |
// | TestSendMessageAppearsInHistory             | the message is found in /chat_history as the player's own, not as a rendered entry                    | Partial |
// | TestSendHyperlinkRendersAsLink              | the URL is a `link` control with the URL as its text (a run-unique URL, so the link is one control)   | Port    |
// | TestNearbyChannelAndConversationsToolbarPresence | the "<n> nearby" button stands for the Nearby channel; the input and Emoji button; no conversations toolbar, which exists only with a direct message open | Partial |
// | TestReactToOwnMessage                       | the reaction pill is a button named "you reacted with ..."; the row has no control to hover, so a click just above the input reveals React | Partial |
import assert from 'node:assert/strict'
import { spec, until, type ChatHistory, type ChatMessage, type Explorer } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000
/** The send is retried with another Enter, as explorer-automation does. */
const SEND_ATTEMPTS = 3
const SEND_ATTEMPT_MS = 3000

const RAIL_CHAT = { name: 'Chat', role: 'button' } as const
const CLOSE_CHAT = { name: 'Close chat', role: 'button' } as const
const INPUT_FOCUSED = { name: 'Write a message', role: 'textbox' } as const
const INPUT_IDLE = { name: 'Press Enter to chat', role: 'textbox' } as const
const EMOJI = { name: 'Emoji', role: 'button' } as const
const REACT = { name: 'React', role: 'button' } as const
const REACTION_BAR = { name: 'React', role: 'dialog' } as const
/** The first of the reaction bar's fixed emoji: the red heart, U+2665 with the variation selector U+FE0F. */
const FIRST_REACTION = { name: String.fromCodePoint(0x2665, 0xfe0f), role: 'button' } as const
const REACTION_PILL = /^you reacted(?: with :\S+:)?$/
/** The input's height in the HUD's design pixels, to read the HUD's scale from its box. */
const INPUT_HEIGHT = 40

async function ownAddress(explorer: Explorer): Promise<string> {
  const { address } = await explorer.worldState()
  assert.ok(address, 'the player should be signed in')
  return address.toLowerCase()
}

/** The chat open and its input focused: from the rail when closed, from the input when open and idle. */
async function openChat(explorer: Explorer): Promise<void> {
  if ((await explorer.hud.get(RAIL_CHAT)).checked) {
    const idle = !(await explorer.hud.isVisible(INPUT_FOCUSED))
    if (idle) await explorer.hud.click(INPUT_IDLE)
  } else {
    await explorer.hud.click(RAIL_CHAT)
  }
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.wait(INPUT_FOCUSED, { timeoutMs: TEXT_TIMEOUT_MS })
}

/** The chat closed, with the rail's button. */
async function closeChat(explorer: Explorer): Promise<void> {
  if (!(await explorer.hud.get(RAIL_CHAT)).checked) return
  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
}

/** The chat closed and no control focused: Enter on a focused button clicks it, which would toggle
 *  the chat again. Closing from the open chat's own button leaves the focus on the page. */
async function resetChat(explorer: Explorer): Promise<void> {
  await explorer.hud.click((await explorer.hud.isVisible(INPUT_FOCUSED)) ? INPUT_FOCUSED : INPUT_IDLE)
  await explorer.hud.wait(CLOSE_CHAT, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(CLOSE_CHAT)
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
}

/** Type `text` in the chat input and send it with Enter; the message as /chat_history records it. */
async function sendMessage(explorer: Explorer, text: string): Promise<ChatMessage> {
  const own = await ownAddress(explorer)
  const before = (await explorer.chatHistory(1)).total
  const recorded = (history: ChatHistory): ChatMessage | undefined =>
    history.total > before ? history.messages.find((message) => message.address?.toLowerCase() === own && message.message === text) : undefined

  const typed = await explorer.hud.fill((await explorer.hud.isVisible(INPUT_FOCUSED)) ? INPUT_FOCUSED : INPUT_IDLE, text)
  assert.equal(typed.value, text, 'the chat input should hold the typed message')
  let history: ChatHistory | undefined
  for (let attempt = 0; attempt < SEND_ATTEMPTS && !history; attempt++) {
    await explorer.key('Enter')
    history = await until(() => explorer.chatHistory(), (read) => recorded(read) !== undefined, { timeoutMs: SEND_ATTEMPT_MS, what: `'${text}' in the chat history` }).catch(() => undefined)
  }
  const message = history && recorded(history)
  assert.ok(message, `Chat message '${text}' did not appear in the history after ${SEND_ATTEMPTS} send attempts`)
  return message
}

/** The newest message's row has no control, so put the pointer on it with a click just above the
 *  input, where the list ends, until its React button shows. */
async function hoverNewestMessage(explorer: Explorer): Promise<void> {
  const input = await explorer.hud.get(INPUT_FOCUSED)
  const unit = input.height / INPUT_HEIGHT
  for (const above of [22, 30, 14, 40, 52]) {
    await explorer.clickAt(input.x + input.width / 2, input.y - above * unit)
    if (await explorer.hud.isVisible(REACT)) return
  }
  throw new Error('no chat message row revealed its React button')
}

async function reactionPills(explorer: Explorer): Promise<number> {
  const buttons = await explorer.hud.list('button')
  return buttons.filter((button) => REACTION_PILL.test(button.name)).length
}

scenario("the rail's Chat button opens the chat with its input focused and closes it", async ({ explorer }) => {
  await closeChat(explorer)
  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.wait(INPUT_FOCUSED, { timeoutMs: TEXT_TIMEOUT_MS })
  assert.equal((await explorer.hud.get(RAIL_CHAT)).checked, true, 'the rail button should show the chat as open')

  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
  await explorer.hud.wait(INPUT_IDLE, { timeoutMs: TEXT_TIMEOUT_MS })
})

scenario('Enter opens the chat with its input focused and Escape leaves the input', async ({ explorer }) => {
  await resetChat(explorer)
  await explorer.key('Enter')
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.wait(INPUT_FOCUSED, { timeoutMs: TEXT_TIMEOUT_MS })

  await explorer.escape()
  await explorer.hud.wait(INPUT_IDLE, { timeoutMs: TEXT_TIMEOUT_MS })
  await closeChat(explorer)
})

scenario('a message sent from the chat input is recorded in the chat history', async ({ explorer }) => {
  await openChat(explorer)
  const sent = await sendMessage(explorer, 'automation ping')
  assert.equal(sent.channel, 'Nearby', 'the message should go to the Nearby channel')
  assert.equal(await explorer.hud.value(INPUT_FOCUSED), '', 'the input should be cleared once the message is sent')
  await closeChat(explorer)
})

scenario('a URL in a message renders as a link', async ({ explorer }) => {
  await openChat(explorer)
  const url = `https://decentraland.org/?e2e=${Date.now().toString(36)}`
  const sent = await sendMessage(explorer, url)
  assert.equal(sent.message, url)

  const selector = { name: url, role: 'link' } as const
  await explorer.hud.wait(selector, { timeoutMs: TEXT_TIMEOUT_MS })
  const link = await explorer.hud.get(selector)
  assert.equal(link.role, 'link', `Chat message '${url}' should render as a link`)
  assert.equal(link.text, url, 'the link should show the URL')
  await closeChat(explorer)
})

scenario('the chat shows the Nearby channel, the input and the Emoji button', async ({ explorer }) => {
  await openChat(explorer)
  const buttons = await explorer.hud.list('button')
  assert.ok(
    buttons.some((button) => /^\d+ nearby$/.test(button.name)),
    'Titlebar should show the Nearby channel with its member count'
  )
  assert.ok(await explorer.hud.isVisible(INPUT_FOCUSED), 'Chat input field should be present')
  await explorer.hud.wait(EMOJI, { timeoutMs: TEXT_TIMEOUT_MS })
  await closeChat(explorer)
})

scenario('the first quick reaction can be added to an own message', async ({ explorer }) => {
  await openChat(explorer)
  const stamp = new Date().toTimeString().slice(0, 8).replaceAll(':', '')
  const sent = await sendMessage(explorer, `automation ping ${stamp}`)
  assert.ok(sent.messageId, 'the sent message should have an id to react to')
  const before = await reactionPills(explorer)

  await hoverNewestMessage(explorer)
  await explorer.hud.click(REACT)
  await explorer.hud.wait(REACTION_BAR, { timeoutMs: TEXT_TIMEOUT_MS })
  await explorer.hud.click(FIRST_REACTION)
  await explorer.hud.waitHidden(REACTION_BAR, TEXT_TIMEOUT_MS)
  await until(() => reactionPills(explorer), (pills) => pills > before, { timeoutMs: TEXT_TIMEOUT_MS, what: 'the reaction pill of the message' })

  const newest = (await explorer.chatHistory(1)).messages.at(-1)
  assert.equal(newest?.messageId, sent.messageId, 'another message arrived while reacting, so the reaction may be on it')
  await closeChat(explorer)
})
