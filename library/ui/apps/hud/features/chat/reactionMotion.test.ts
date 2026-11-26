import { describe, it, expect } from 'vitest'
import { MAX_FLYING, SWAY, SWAY_PERIOD, flightEnd, reactionFlight } from './reactionMotion'

/** The flight stepped through, a 240th of a second at a time: pushed by the swinging force and the
 *  lift, then slowed, then moved. */
function stepped(kick: number, up: number, phase: number, life: number): { x: number; y: number } {
  const dt = 1 / 240
  let [x, y, vx, vy] = [0, 0, kick, up]
  for (let age = 0; age < life; age += dt) {
    vx += (Math.sin(age * 2 * Math.PI + phase) * 130 + 1) * dt
    vy += 201 * dt
    const slowed = Math.exp(-1.2 * dt)
    vx *= slowed
    vy *= slowed
    x += vx * dt
    y += vy * dt
  }
  return { x, y }
}

describe('the on-screen flight of a reaction', () => {
  it.each([
    [40, 175, 0, 2.04],
    [60, 200, Math.PI / 2, 2.54],
    [50, 190, 2.5, 2.3],
    [45, 180, 5.5, 2.2]
  ])('ends where stepping through it does (kick %d, up %d, phase %d, life %d)', (kick, up, phase, life) => {
    const end = flightEnd(kick, up, phase, life)
    const reference = stepped(kick, up, phase, life)
    expect(Math.abs(end.rise - reference.y)).toBeLessThan(3)
    expect(Math.abs(end.drift + end.sway - reference.x)).toBeLessThan(1.5)
  })

  // Each draw at its least, its middle and its most, in every mix: the shortest life with the
  // slowest start, the hardest kick with the longest life, and the rest.
  it('stays within its ranges whatever the dice say', () => {
    const rolls = [0, 0.5, 0.999999]
    // a flight draws seven numbers
    const draws = 7
    for (let mix = 0; mix < rolls.length ** draws; mix++) {
      let drawn = 0
      const flight = reactionFlight(() => rolls[Math.floor(mix / rolls.length ** (drawn++ % draws)) % rolls.length])
      expect(drawn).toBe(draws)
      expect(flight.life).toBeGreaterThanOrEqual(2.04)
      expect(flight.life).toBeLessThanOrEqual(2.54)
      expect(flight.size).toBeGreaterThanOrEqual(30)
      expect(flight.size).toBeLessThanOrEqual(40)
      // beside the button, never over it
      expect(flight.x).toBeGreaterThan(40 - 8 - SWAY - 1e-9)
      expect(flight.x).toBeLessThan(40 + 8 + SWAY + 1e-9)
      expect(Math.abs(flight.y)).toBeLessThanOrEqual(4)
      // up most of half a screen, and a little to the right
      expect(flight.rise).toBeGreaterThan(340)
      expect(flight.rise).toBeLessThan(460)
      expect(flight.drift).toBeGreaterThan(10)
      expect(flight.drift).toBeLessThan(75)
      expect(flight.swayAt).toBeGreaterThanOrEqual(0)
      expect(flight.swayAt).toBeLessThan(SWAY_PERIOD)
    }
  })

  it('sways a few px either side, once a second', () => {
    expect(SWAY).toBeGreaterThan(3)
    expect(SWAY).toBeLessThan(3.5)
    expect(SWAY_PERIOD).toBe(1)
    expect(MAX_FLYING).toBe(30)
  })
})
