import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import type { SituationalReaction } from '@explorer-core/engine/protocol'
import { ReactionLane } from '../../features/chat/ReactionLane'
import { MAX_FLYING } from '../../features/chat/reactionMotion'

const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const FIRE = String.fromCodePoint(0x1f525)
const THUMBS = String.fromCodePoint(0x1f44d)
/** Longer than any reaction lasts. */
const A_LIFE = 3000

type Listener = (reaction: SituationalReaction) => void

/** The lane with a stream the test plays reactions on, rising from a button that is there. */
function renderLane(): { play: (emoji: string) => void; listening: () => number; unmount: () => void } {
  const listeners = new Set<Listener>()
  const subscribe = (listener: Listener): (() => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  }
  const button = document.createElement('span')
  document.body.appendChild(button)
  const { unmount } = render(<ReactionLane subscribe={subscribe} anchor={{ current: button }} />)
  return {
    play: (emoji) => act(() => listeners.forEach((listener) => listener({ from: MOJITO, emoji }))),
    listening: () => listeners.size,
    unmount: () => {
      unmount()
      button.remove()
    }
  }
}

const flying = (emoji: string): number => screen.queryAllByText(emoji).length

describe('the on-screen lane of reactions', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('shows each reaction played and lets it go when its flight is over', () => {
    const lane = renderLane()
    lane.play(FIRE)
    lane.play(THUMBS)
    lane.play(FIRE)
    expect(flying(FIRE)).toBe(2)
    expect(flying(THUMBS)).toBe(1)
    act(() => vi.advanceTimersByTime(A_LIFE))
    expect(flying(FIRE)).toBe(0)
    expect(flying(THUMBS)).toBe(0)
    lane.unmount()
  })

  it('is no part of what a screen reader reads, and takes its place from the button', () => {
    const lane = renderLane()
    lane.play(FIRE)
    const glyph = screen.getByText(FIRE)
    expect(glyph.closest('[aria-hidden="true"]')).not.toBeNull()
    lane.unmount()
  })

  it('shows no more than its fill at once, and room made is room for the next', () => {
    const lane = renderLane()
    for (let i = 0; i < MAX_FLYING + 5; i++) lane.play(FIRE)
    expect(flying(FIRE)).toBe(MAX_FLYING)
    // the ones not shown are not kept for later
    act(() => vi.advanceTimersByTime(A_LIFE))
    expect(flying(FIRE)).toBe(0)
    lane.play(THUMBS)
    expect(flying(THUMBS)).toBe(1)
    lane.unmount()
  })

  it('shows nothing played while the page is not on screen', () => {
    const lane = renderLane()
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    lane.play(FIRE)
    expect(flying(FIRE)).toBe(0)
    hidden.mockReturnValue(false)
    lane.play(FIRE)
    expect(flying(FIRE)).toBe(1)
    lane.unmount()
  })

  it('stops listening, and waiting on its flights, when it goes', () => {
    const lane = renderLane()
    expect(lane.listening()).toBe(1)
    lane.play(FIRE)
    lane.unmount()
    expect(lane.listening()).toBe(0)
    expect(vi.getTimerCount()).toBe(0)
  })
})
