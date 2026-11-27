import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import type { SituationalReaction } from '@explorer-core/engine/protocol'
import { ReactionLane } from '../../features/chat/ReactionLane'
import { MAX_FLYING, reactionFlight } from '../../features/chat/reactionMotion'

// the HUD's scale, set by each test: there is no viewport here to take it from
const hud = vi.hoisted(() => ({ scale: 1 }))
vi.mock('@explorer-core/lib/hudScale', () => ({ getHudScale: () => hud.scale }))

const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const FIRE = String.fromCodePoint(0x1f525)
const THUMBS = String.fromCodePoint(0x1f44d)
/** Longer than any reaction lasts. */
const A_LIFE = 3000

type Listener = (reaction: SituationalReaction) => void

/** The lane with a stream the test plays reactions on, rising from a button that is there, at
 *  `at` on the page when given. */
function renderLane(at?: { left: number; top: number; width: number; height: number }): {
  play: (emoji: string) => void
  listening: () => number
  unmount: () => void
} {
  const listeners = new Set<Listener>()
  const subscribe = (listener: Listener): (() => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  }
  const button = document.createElement('span')
  document.body.appendChild(button)
  if (at != null) vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(at as DOMRect)
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
    hud.scale = 1
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

  it('is no part of what a screen reader reads', () => {
    const lane = renderLane()
    lane.play(FIRE)
    const glyph = screen.getByText(FIRE)
    expect(glyph.closest('[aria-hidden="true"]')).not.toBeNull()
    lane.unmount()
  })

  // The lane is scaled as the HUD is, so the button's place on the page is divided by that scale.
  // The button's box is in page px: 34 px across at the HUD's reference size.
  it.each([
    ['at the reference size', 1, { left: 361, top: 1037, width: 34, height: 34 }],
    ['on a larger screen', 4 / 3, { left: 482, top: 1383, width: 34 * (4 / 3), height: 34 * (4 / 3) }],
    ['on a smaller screen', 0.5, { left: 180, top: 518, width: 17, height: 17 }],
    // a place with no size is still a place
    ['from a button with no box', 2, { left: 790, top: 2108, width: 0, height: 0 }]
  ])('starts beside the middle of the button, in its own px, %s', (_, scale, box) => {
    hud.scale = scale
    vi.spyOn(Math, 'random').mockReturnValue(0.5)
    const flight = reactionFlight(() => 0.5)
    const lane = renderLane(box)
    lane.play(FIRE)
    const particle = screen.getByText(FIRE).closest<HTMLElement>('[style]')!
    expect(parseFloat(particle.style.left)).toBeCloseTo((box.left + box.width / 2) / scale + flight.x, 6)
    expect(parseFloat(particle.style.top)).toBeCloseTo((box.top + box.height / 2) / scale - flight.y, 6)
    lane.unmount()
  })

  it('takes the scale as it is when each reaction plays', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.5)
    const flight = reactionFlight(() => 0.5)
    const lane = renderLane({ left: 400, top: 800, width: 0, height: 0 })
    lane.play(FIRE)
    hud.scale = 2
    lane.play(THUMBS)
    expect(parseFloat(screen.getByText(FIRE).closest<HTMLElement>('[style]')!.style.left)).toBeCloseTo(400 + flight.x, 6)
    expect(parseFloat(screen.getByText(THUMBS).closest<HTMLElement>('[style]')!.style.left)).toBeCloseTo(200 + flight.x, 6)
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
