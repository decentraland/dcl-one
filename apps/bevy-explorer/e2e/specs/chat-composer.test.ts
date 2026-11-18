// The QA Automation Suite's CHAT cases beyond explorer-automation's ChatTests (chat.test.ts): the T
// key, the composer's emoji picker, and the cases one account cannot run. The T key is the engine's
// ChatPanel action (crates/common/src/inputs.rs), which toggles the panel as the rail's Chat button
// does; the panel then focuses its input, which holds the keys, so the second T comes after an
// Escape that leaves the input (the panel stays open). The composer's Emoji button opens the
// `Emoji picker` dialog; its `Search emoji` field filters the grid, whose buttons are named by the
// emoji, and a pick is inserted into the input at the caret (Chat.tsx `insertAtCaret`).
//
// | Suite case                | Here                                                                       | Class   |
// | Open/Close chat           | T opens the panel; after Escape leaves the input, T closes it (the rail and Enter are chat.test.ts) | Port    |
// | Chat Functionalities      | the Emoji button inserts a searched emoji into the input (links: chat.test.ts; coordinates: teleport.test.ts) | Partial |
// | Chat Functionalities      | an @mention of a nearby user renders as a button                           | Gap     |
// | Chat Functionalities      | bold and italic text                                                       | Gap     |
// | In-chat navbar            | a DM partner's tab beside Nearby                                           | Gap     |
// | Direct Message            | a DM opened from a user's profile card                                     | Gap     |
// | Chat Reactions on Input   | the situational reaction bar beside the input                              | Gap     |
//
// Partial, Chat Functionalities: the suite sends an emote, a link, coordinates, a mention and bold or
// italic text in one go. The link and the coordinates have their own scenarios; the emoji is checked
// here through the input's value, as the message's text is not addressable (chat.test.ts).
// Gaps: a mention resolves only against the nearby members (chatText.tsx), and a DM tab and a profile
// card need a second user in the world: the suite drives one client with one account, so these wait
// for a second client. The HUD applies no bold or italic markup (Notion U3). The suite's reaction bar
// beside the input sends a reaction to the scene; the Bevy composer has no such bar, only the Emoji
// button (reactions on a message are chat.test.ts).
import assert from 'node:assert/strict'
import { spec, until } from '../driver/index.ts'
import { CLOSE_CHAT, CONTROL_TIMEOUT_MS, EMOJI, INPUT_FOCUSED, TEXT_TIMEOUT_MS, closeChat, openChat } from './chat.ts'

const scenario = spec(import.meta.url)

const PICKER = { name: 'Emoji picker', role: 'dialog' } as const
const PICKER_SEARCH = { name: 'Search emoji', role: 'textbox' } as const
/** The rocket: one code point, no variation selector, so the input holds exactly this text. */
const ROCKET = String.fromCodePoint(0x1f680)

scenario('T opens the chat panel and, once Escape leaves the input, T closes it', async ({ explorer }) => {
  await closeChat(explorer)
  await explorer.key('KeyT')
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  // An input holding the keys would type the next T instead of closing the panel.
  if ((await explorer.worldState()).keyboardClaimed === true) {
    await explorer.escape()
    await explorer.keyboardReleased()
  }
  assert.ok(await explorer.hud.isVisible(CLOSE_CHAT), 'leaving the input should keep the chat panel open')
  await explorer.key('KeyT')
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
})

scenario("the composer's Emoji button inserts a searched emoji into the input", async ({ explorer }) => {
  await openChat(explorer)
  try {
    await explorer.hud.fill(INPUT_FOCUSED, '')
    await explorer.hud.click(EMOJI)
    await explorer.hud.wait(PICKER, { timeoutMs: TEXT_TIMEOUT_MS })
    await explorer.hud.fill(PICKER_SEARCH, 'rocket')
    await explorer.hud.click({ name: ROCKET, role: 'button', index: 0 })
    await until(() => explorer.hud.value(INPUT_FOCUSED), (value) => value.includes(ROCKET), { timeoutMs: TEXT_TIMEOUT_MS, what: 'the rocket emoji in the chat input' })
  } finally {
    // The picker closes on Escape (its own handler stops the press there); then the input is emptied.
    if (await explorer.hud.isVisible(PICKER)) {
      await explorer.escape()
      await explorer.hud.waitHidden(PICKER, TEXT_TIMEOUT_MS)
    }
    await openChat(explorer)
    await explorer.hud.fill(INPUT_FOCUSED, '')
    await closeChat(explorer)
  }
})

const ONE_ACCOUNT = 'needs a second user in the world: the suite drives one client with one account'

scenario('an @mention of a nearby user renders as a button that opens their profile', async () => {}, { todo: `a mention resolves only against the nearby members; ${ONE_ACCOUNT}` })

scenario('bold and italic markup is applied to a sent message', async () => {}, { todo: 'the HUD applies no bold or italic markup to chat text (Notion U3)' })

scenario("a direct message opened from a user's profile card adds their tab beside Nearby", async () => {}, { todo: ONE_ACCOUNT })

scenario('a reaction picked beside the input is sent to the scene', async () => {}, { todo: 'the Bevy composer has no situational reaction bar, only the Emoji button' })
