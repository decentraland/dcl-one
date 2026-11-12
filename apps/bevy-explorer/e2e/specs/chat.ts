// Shared by the chat specs (chat, chat-composer, teleport): the chat panel's controls by their
// accessible names, and opening, closing and typing into it. Names are the HUD's
// (library/ui/apps/hud/features/chat/Chat.tsx). The input is always mounted: `Press Enter to chat`
// while it is unfocused and `Write a message` while it is focused (the placeholder is its only name).
import assert from 'node:assert/strict'
import { until, type ChatHistory, type ChatMessage, type Explorer } from '../driver/index.ts'

export const CONTROL_TIMEOUT_MS = 20_000
export const TEXT_TIMEOUT_MS = 10_000
/** The most a sent message is given to reach the engine's chat history: a ceiling, the wait ends
 *  when it is there. */
export const SEND_TIMEOUT_MS = 10_000

export const RAIL_CHAT = { name: 'Chat', role: 'button' } as const
export const CLOSE_CHAT = { name: 'Close chat', role: 'button' } as const
export const INPUT_FOCUSED = { name: 'Write a message', role: 'textbox' } as const
export const INPUT_IDLE = { name: 'Press Enter to chat', role: 'textbox' } as const
export const EMOJI = { name: 'Emoji', role: 'button' } as const

export async function ownAddress(explorer: Explorer): Promise<string> {
  const { address } = await explorer.worldState()
  assert.ok(address, 'the player should be signed in')
  return address.toLowerCase()
}

/** The chat open and its input focused: from the rail when closed, from the input when open and idle. */
export async function openChat(explorer: Explorer): Promise<void> {
  if ((await explorer.hud.get(RAIL_CHAT)).checked) {
    const idle = !(await explorer.hud.isVisible(INPUT_FOCUSED))
    if (idle) await explorer.hud.click(INPUT_IDLE)
  } else {
    await explorer.hud.click(RAIL_CHAT)
  }
  await explorer.hud.wait(CLOSE_CHAT, { timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.wait(INPUT_FOCUSED, { timeoutMs: TEXT_TIMEOUT_MS })
}

/** The chat closed, with the rail's button; the input it had focused gives the engine the keys back. */
export async function closeChat(explorer: Explorer): Promise<void> {
  if (!(await explorer.hud.get(RAIL_CHAT)).checked) return
  await explorer.hud.click(RAIL_CHAT)
  await explorer.hud.waitHidden(CLOSE_CHAT, CONTROL_TIMEOUT_MS)
  await explorer.keyboardReleased()
}

/** Type `text` in the chat input and press Enter, then wait for the HUD to empty the input as it
 *  takes the line. The fill returns once the input has the keyboard, so the Enter is the input's.
 *  A line starting with `/` is a chat command (`/goto`, `/world`): the HUD runs it and sends nothing. */
export async function submitLine(explorer: Explorer, text: string): Promise<void> {
  const typed = await explorer.hud.fill((await explorer.hud.isVisible(INPUT_FOCUSED)) ? INPUT_FOCUSED : INPUT_IDLE, text)
  assert.equal(typed.value, text, 'the chat input should hold the typed line')
  await explorer.key('Enter')
  await waitInputEmptied(explorer, `the chat input to empty as it takes '${text}'`)
}

/** Type `text` in the chat input and send it with Enter; the message as /chat_history records it. */
export async function sendMessage(explorer: Explorer, text: string): Promise<ChatMessage> {
  const own = await ownAddress(explorer)
  const before = (await explorer.chatHistory(1)).total
  const recorded = (history: ChatHistory): ChatMessage | undefined =>
    history.total > before ? history.messages.find((message) => message.address?.toLowerCase() === own && message.message === text) : undefined

  await submitLine(explorer, text)
  const history = await until(() => explorer.chatHistory(), (read) => recorded(read) !== undefined, { timeoutMs: SEND_TIMEOUT_MS, what: `'${text}' in the chat history` })
  return recorded(history)!
}

/** Wait for the chat input, focused or not, to be empty. */
export function waitInputEmptied(explorer: Explorer, what: string): Promise<string | undefined> {
  const value = async (): Promise<string | undefined> => ((await explorer.hud.find(INPUT_FOCUSED)) ?? (await explorer.hud.find(INPUT_IDLE)))?.value
  return until(value, (held) => held === '', { timeoutMs: TEXT_TIMEOUT_MS, what })
}
