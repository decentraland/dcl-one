import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Chat } from '../../features/chat/Chat'
import { EmojiPicker } from '../../features/chat/EmojiPicker'
import { EMOJI_GROUPS } from '../../features/chat/emojiData'
import type { ChatState } from '../../features/session/useEngineSession'
import { fakeSession } from '../harness'

function chatState(over: Partial<ChatState> = {}): ChatState {
  return { ...fakeSession().chat, open: true, send: vi.fn(), toggle: vi.fn(), members: [], ...over }
}

// Activate the chat (full chrome) by hovering its root — avoids the focus-then-blur
// race that would unmount the chrome mid-click.
function renderActiveChat(over: Partial<ChatState> = {}): { container: HTMLElement; chat: ChatState } {
  const chat = chatState(over)
  const { container } = render(<Chat chat={chat} />)
  fireEvent.mouseEnter(container.firstChild as Element)
  return { container, chat }
}

describe('chat chrome clicks', () => {
  it('the emoji button opens the picker', async () => {
    renderActiveChat()
    await userEvent.click(screen.getByRole('button', { name: 'Emoji' }))
    expect(screen.getByPlaceholderText('Search emoji')).toBeInTheDocument()
  })

  it('typing, the emoji panel, a picked emoji and the buttons each sound', async () => {
    const { chat } = renderActiveChat({ playSound: vi.fn() })
    const played = (): string[] => vi.mocked(chat.playSound).mock.calls.map((c) => c[0])
    await userEvent.type(screen.getByRole('textbox'), 'hi')
    expect(played()).toEqual(['chatInput', 'chatInput'])
    vi.mocked(chat.playSound).mockClear()
    // the emoji toggle has the close press; the panel opening has the swish
    await userEvent.click(screen.getByRole('button', { name: 'Emoji' }))
    expect(played()).toEqual(['buttonHover', 'buttonClose', 'chatEmoji'])
    vi.mocked(chat.playSound).mockClear()
    // a category tab presses without a highlight
    await userEvent.click(screen.getByRole('button', { name: 'Food and drink' }))
    expect(played()).toEqual(['buttonPress'])
    vi.mocked(chat.playSound).mockClear()
    const group = EMOJI_GROUPS[0]
    await userEvent.click(screen.getByTitle(group.emojis[0].expression))
    // the emoji's own button, the swish, and the input's text changing
    expect(played()).toEqual(['buttonHover', 'buttonPress', 'chatEmoji', 'chatInput'])
  }, 20_000)

  it('the members button reveals the nearby overlay', async () => {
    renderActiveChat({ members: [{ address: '0x1', name: 'Alice' }] })
    await userEvent.click(screen.getByRole('button', { name: '1 nearby' }))
    expect(screen.getByText('Alice')).toBeInTheDocument()
  })

  it('the nav close button toggles the chat', async () => {
    const { chat } = renderActiveChat()
    await userEvent.click(screen.getByRole('button', { name: 'Close chat' }))
    expect(vi.mocked(chat.toggle)).toHaveBeenCalledTimes(1)
  })
})

describe('chat focus', () => {
  it('a focused control that unmounts lets go of the chat: Back in the members list, then the pointer leaves', async () => {
    const { container } = renderActiveChat({ members: [{ address: '0x1', name: 'Alice' }] })
    const root = container.firstChild as HTMLElement
    const idle = (): boolean => !root.className.split(' ').some((c) => /(^|_)active(_|$)/.test(c))
    await userEvent.click(screen.getByRole('button', { name: '1 nearby' }))
    const back = screen.getByRole('button', { name: /back/i })
    back.focus()
    expect(document.activeElement).toBe(back)
    await userEvent.click(back)
    expect(back.isConnected).toBe(false)
    expect(idle()).toBe(false) // the pointer is still over the panel
    fireEvent.mouseLeave(root)
    expect(idle()).toBe(true)
  })


  it('mounting idle-open leaves the keyboard to the engine; opening or a focus request takes it', () => {
    const { rerender } = render(<Chat chat={chatState({ open: true, focusTick: 0 })} />)
    const input = screen.getByRole('textbox')
    expect(document.activeElement).not.toBe(input)
    rerender(<Chat chat={chatState({ open: true, focusTick: 1 })} />)
    expect(document.activeElement).toBe(input)
    input.blur()
    rerender(<Chat chat={chatState({ open: false, focusTick: 1 })} />)
    rerender(<Chat chat={chatState({ open: true, focusTick: 1 })} />)
    expect(document.activeElement).toBe(screen.getByRole('textbox'))
  })
})

describe('emoji picker', () => {
  it('picking an emoji calls onPick with the glyph', async () => {
    const onPick = vi.fn()
    render(<EmojiPicker onPick={onPick} />)
    const first = EMOJI_GROUPS[0].emojis[0]
    await userEvent.click(screen.getAllByTitle(first.expression)[0])
    expect(onPick).toHaveBeenCalledWith(first.emoji)
  })
})
