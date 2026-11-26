import { afterEach, describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { dispatchCancelLayer, registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { CHAT_REACTIONS_SETTING } from '@explorer-core/features/chat/reactions'
import { PREF, getPref, setPref } from '@explorer-core/lib/prefs'
import type { Setting } from '@explorer-core/engine/protocol'
import { Chat } from '../../features/chat/Chat'
import { EMOJI_GROUPS, QUICK_REACTIONS } from '../../features/chat/emojiData'
import { SessionProvider } from '../../features/session/SessionContext'
import type { ChatState, EngineSession } from '../../features/session/useEngineSession'
import { fakeSession } from '../harness'

const HEART = QUICK_REACTIONS[0]
const FIRE = String.fromCodePoint(0x1f525)
const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

function chatState(over: Partial<ChatState> = {}): ChatState {
  return { ...fakeSession().chat, open: true, sendReaction: vi.fn(), react: vi.fn(), ...over }
}

function renderChat(over: Partial<ChatState> = {}, session?: EngineSession): { chat: ChatState; rerender: (over: Partial<ChatState>, hidden?: boolean) => void } {
  const chat = chatState(over)
  const tree = (state: ChatState, hidden = false): React.JSX.Element => {
    const ui = <Chat chat={state} hidden={hidden} />
    return session != null ? <SessionProvider value={session}>{ui}</SessionProvider> : ui
  }
  const { container, rerender } = render(tree(chat))
  fireEvent.mouseEnter(container.firstChild as Element)
  return { chat, rerender: (next, hidden) => rerender(tree({ ...chat, ...next }, hidden)) }
}

const reactions = (): HTMLElement => screen.getByRole('button', { name: 'Reactions' })
const bar = (): HTMLElement | null => screen.queryByRole('dialog', { name: 'Reactions' })
const sent = (chat: ChatState): string[] => vi.mocked(chat.sendReaction).mock.calls.map((c) => c[0])

function reactionsSetting(value: number): Setting {
  return { name: CHAT_REACTIONS_SETTING, category: 'Chat', description: '', minValue: 0, maxValue: 1, namedVariants: [{ name: 'Off', description: '' }, { name: 'On', description: '' }], value, default: 0, stepSize: 1, disabled: false }
}

function sessionWith(list: Setting[]): EngineSession {
  const session = fakeSession()
  return { ...session, settings: { ...session.settings, list } }
}

afterEach(() => {
  setPref(PREF.emojiRecents, '[]')
})

describe('the reactions button', () => {
  it('opens its bar and reads pressed while the bar is open; a second press closes it', async () => {
    renderChat()
    expect(reactions()).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(reactions())
    expect(bar()).toBeInTheDocument()
    expect(reactions()).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(reactions())
    expect(bar()).not.toBeInTheDocument()
    expect(reactions()).toHaveAttribute('aria-pressed', 'false')
  })

  it('takes the keyboard from the message being written', async () => {
    renderChat()
    const input = screen.getByRole('textbox')
    await userEvent.click(input)
    expect(document.activeElement).toBe(input)
    await userEvent.click(reactions())
    expect(document.activeElement).not.toBe(input)
  })

  it('is not offered while the chat is collapsed', () => {
    renderChat({ open: false })
    expect(screen.queryByRole('button', { name: 'Reactions' })).not.toBeInTheDocument()
  })

  it('types nothing into the message', async () => {
    renderChat()
    await userEvent.click(reactions())
    await userEvent.click(within(bar()!).getByRole('button', { name: HEART }))
    expect(screen.getByRole('textbox')).toHaveValue('')
  })
})

describe('the reactions bar', () => {
  it('sends each pick and stays open for the next', async () => {
    const { chat } = renderChat()
    await userEvent.click(reactions())
    await userEvent.click(within(bar()!).getByRole('button', { name: HEART }))
    await userEvent.click(within(bar()!).getByRole('button', { name: FIRE }))
    await userEvent.click(within(bar()!).getByRole('button', { name: FIRE }))
    expect(sent(chat)).toEqual([HEART, FIRE, FIRE])
    expect(bar()).toBeInTheDocument()
    expect(vi.mocked(chat.react)).not.toHaveBeenCalled()
  })

  it('leaves the recent emoji as they were', async () => {
    renderChat()
    const before = getPref(PREF.emojiRecents)
    await userEvent.click(reactions())
    await userEvent.click(within(bar()!).getByRole('button', { name: FIRE }))
    expect(getPref(PREF.emojiRecents)).toBe(before)
  })

  it('gives way to the full picker, whose picks send too, stay open and become recent', async () => {
    const { chat } = renderChat()
    const emoji = EMOJI_GROUPS[1].emojis[0]
    expect(QUICK_REACTIONS).not.toContain(emoji.emoji)
    await userEvent.click(reactions())
    await userEvent.click(within(bar()!).getByRole('button', { name: 'More emoji' }))
    expect(bar()).not.toBeInTheDocument()
    const picker = screen.getByRole('dialog', { name: 'Emoji picker' })
    await userEvent.click(within(picker).getAllByTitle(emoji.expression)[0])
    await userEvent.click(within(picker).getAllByTitle(emoji.expression)[0])
    expect(sent(chat)).toEqual([emoji.emoji, emoji.emoji])
    expect(screen.getByRole('dialog', { name: 'Emoji picker' })).toBeInTheDocument()

    // Cancel takes the picker first, then the bar, which now offers the emoji picked
    expect(dispatchCancelLayer()).toBe(true)
    expect(await screen.findByRole('dialog', { name: 'Reactions' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog', { name: 'Emoji picker' })).not.toBeInTheDocument()
    expect(within(bar()!).getByRole('button', { name: emoji.emoji })).toBeInTheDocument()
    expect(dispatchCancelLayer()).toBe(true)
    await vi.waitFor(() => expect(bar()).not.toBeInTheDocument())
    expect(dispatchCancelLayer()).toBe(false)
  }, 20_000)

  it('the cancel key closes the full picker with a picked emoji focused, and that press leaves the bar', async () => {
    renderChat()
    const emoji = EMOJI_GROUPS[1].emojis[0]
    await userEvent.click(reactions())
    await userEvent.click(within(bar()!).getByRole('button', { name: 'More emoji' }))
    const picked = within(screen.getByRole('dialog', { name: 'Emoji picker' })).getAllByTitle(emoji.expression)[0]
    await userEvent.click(picked)
    expect(picked).toHaveFocus()

    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Emoji picker' })).not.toBeInTheDocument()
    expect(await screen.findByRole('dialog', { name: 'Reactions' })).toBeInTheDocument()
    // an engine that saw the press too sends its Cancel: the picker took it already
    expect(dispatchCancelLayer()).toBe(true)
    expect(bar()).toBeInTheDocument()

    // the next press is the bar's
    await new Promise((resolve) => setTimeout(resolve, 350))
    expect(dispatchCancelLayer()).toBe(true)
    await vi.waitFor(() => expect(bar()).not.toBeInTheDocument())
  }, 20_000)

  it('keeps its place under a layer opened after it while its picker opens and closes', async () => {
    renderChat()
    await userEvent.click(reactions())
    const later = vi.fn()
    const unregister = registerCancelLayer(later)
    try {
      await userEvent.click(within(bar()!).getByRole('button', { name: 'More emoji' }))
      expect(screen.getByRole('dialog', { name: 'Emoji picker' })).toBeInTheDocument()
      expect(dispatchCancelLayer()).toBe(true)
      expect(later).toHaveBeenCalledTimes(1)
      expect(screen.getByRole('dialog', { name: 'Emoji picker' })).toBeInTheDocument()
    } finally {
      unregister()
    }
  }, 20_000)

  it('closes on a press outside it', async () => {
    renderChat()
    await userEvent.click(reactions())
    fireEvent.pointerDown(document.body)
    expect(bar()).not.toBeInTheDocument()
  })

  it.each([
    ['the conversation shown changes', { channel: MOJITO }, false],
    ['the chat collapses', { open: false }, false],
    ['the chat makes way for another panel', {}, true]
  ] as const)('closes when %s', async (_name, change, hidden) => {
    const { rerender } = renderChat()
    await userEvent.click(reactions())
    expect(bar()).toBeInTheDocument()
    rerender(change, hidden)
    rerender({}, false)
    expect(bar()).not.toBeInTheDocument()
  })

  it('closes when the nearby list is shown', async () => {
    renderChat({ members: [{ address: '0x1', name: 'Alice' }] })
    await userEvent.click(reactions())
    await userEvent.click(screen.getByRole('button', { name: '1 nearby' }))
    expect(bar()).not.toBeInTheDocument()
  })
})

describe('the reactions bar and a message\'s', () => {
  const line = { sender: MOJITO, message: 'hello', channel: 'Nearby', messageId: `${MOJITO}:1`, id: 1, ts: Date.now() }

  it('are never both open: a message\'s takes its place', async () => {
    const { chat } = renderChat({ messages: [line] })
    await userEvent.click(reactions())
    await userEvent.click(screen.getByRole('button', { name: 'React' }))
    expect(bar()).not.toBeInTheDocument()
    const messageBar = screen.getByRole('dialog', { name: 'React' })
    await userEvent.click(within(messageBar).getByRole('button', { name: HEART }))
    expect(vi.mocked(chat.react)).toHaveBeenCalledWith(line.messageId, HEART, false)
    expect(vi.mocked(chat.sendReaction)).not.toHaveBeenCalled()
  })

  it('a press of the reactions button over a message\'s bar only closes that', async () => {
    renderChat({ messages: [line] })
    await userEvent.click(screen.getByRole('button', { name: 'React' }))
    expect(screen.getByRole('dialog', { name: 'React' })).toBeInTheDocument()
    await userEvent.click(reactions())
    expect(screen.queryByRole('dialog', { name: 'React' })).not.toBeInTheDocument()
    expect(bar()).not.toBeInTheDocument()
  })
})

describe('the Chat Reactions switch in the bar', () => {
  it('is not there when the chat renders on its own', async () => {
    renderChat()
    await userEvent.click(reactions())
    expect(screen.queryByRole('switch', { name: 'Chat Reactions' })).not.toBeInTheDocument()
  })

  it('is not there where the engine lists no such setting', async () => {
    renderChat({}, sessionWith([]))
    await userEvent.click(reactions())
    expect(screen.queryByRole('switch', { name: 'Chat Reactions' })).not.toBeInTheDocument()
  })

  it.each([
    [0, false, 1],
    [1, true, 0]
  ])('shows the setting at %i and asks the engine for the other value', async (value, checked, asked) => {
    const session = sessionWith([reactionsSetting(value)])
    renderChat({}, session)
    await userEvent.click(reactions())
    expect(vi.mocked(session.settings.load)).toHaveBeenCalled()
    const toggle = screen.getByRole('switch', { name: 'Chat Reactions' })
    if (checked) expect(toggle).toBeChecked()
    else expect(toggle).not.toBeChecked()
    await userEvent.click(toggle)
    expect(vi.mocked(session.settings.set)).toHaveBeenCalledWith(CHAT_REACTIONS_SETTING, asked)
    // the engine's answer moves it, not the press
    if (checked) expect(toggle).toBeChecked()
    else expect(toggle).not.toBeChecked()
    expect(bar()).toBeInTheDocument()
  })

  it('waits for a value the engine does not know yet', async () => {
    renderChat({}, sessionWith([reactionsSetting(-1)]))
    await userEvent.click(reactions())
    expect(screen.getByRole('switch', { name: 'Chat Reactions' })).toBeDisabled()
  })
})
