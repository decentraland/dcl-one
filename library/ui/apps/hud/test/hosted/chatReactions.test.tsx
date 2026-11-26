import { describe, it, expect, vi } from 'vitest'
import { act } from '@testing-library/react'
import { renderSession, enterAsGuest } from '../harness'

// DOMAIN: situational reactions — the emoji the player picks goes to the engine, which shows it
// over their avatar, and each reaction the engine plays is handed to whoever shows them on screen.
const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const FIRE = String.fromCodePoint(0x1f525)

describe('situational reactions domain', () => {
  it("sends the player's pick to the engine as it is", async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    act(() => h.session().chat.sendReaction(FIRE))
    expect(h.driver.last('sendReaction')).toEqual({ kind: 'sendReaction', emoji: FIRE })
  })

  it('hands each reaction played to its listeners, until they stop listening', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const heard = vi.fn()
    const stop = h.session().chat.onReaction(heard)
    act(() => h.driver.emit({ kind: 'reaction', reaction: { from: MOJITO, emoji: FIRE } }))
    expect(heard.mock.calls).toEqual([[{ from: MOJITO, emoji: FIRE }]])
    stop()
    act(() => h.driver.emit({ kind: 'reaction', reaction: { from: MOJITO, emoji: FIRE } }))
    expect(heard).toHaveBeenCalledTimes(1)
  })

  it('a reaction played is no line of the chat and no new session to render', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const before = h.session()
    act(() => h.driver.emit({ kind: 'reaction', reaction: { from: MOJITO, emoji: FIRE } }))
    expect(h.session()).toBe(before)
    expect(h.session().chat.messages).toBe(before.chat.messages)
  })
})
