import { describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import { renderSession, enterAsGuest } from '../harness'

// DOMAIN: situational reactions — the emoji the player picks goes to the engine, which shows it
// over their avatar.
const FIRE = String.fromCodePoint(0x1f525)

describe('situational reactions domain', () => {
  it("sends the player's pick to the engine as it is", async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    act(() => h.session().chat.sendReaction(FIRE))
    expect(h.driver.last('sendReaction')).toEqual({ kind: 'sendReaction', emoji: FIRE })
  })
})
