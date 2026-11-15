import { describe, it, expect } from 'vitest'
import { createSceneLookup, SCENE_LOOKUP_ATTEMPTS, type SceneLookup } from '../../bridge-scene/src/sceneLookup'

const HERE = '10,20'
const THERE = '11,20'

/** Polls an empty parcel until the lookup publishes it as empty, which it does only at the last attempt. */
function publishEmpty(lookup: SceneLookup, key = HERE): void {
  for (let poll = 1; poll <= SCENE_LOOKUP_ATTEMPTS; poll++) {
    expect(lookup.poll(key)).toBe(true)
    expect(lookup.settle(key, false)).toBe(poll === SCENE_LOOKUP_ATTEMPTS)
  }
}

/** How many polls pass until the lookup asks the engine again. */
function pollsUntilAsked(lookup: SceneLookup, key = HERE, limit = 1000): number {
  for (let poll = 1; poll <= limit; poll++) if (lookup.poll(key)) return poll
  return Infinity
}

describe('scene title lookup', () => {
  it('publishes a scene at once and does not ask again on that parcel', () => {
    const lookup = createSceneLookup()
    expect(lookup.poll(HERE)).toBe(true)
    expect(lookup.settle(HERE, true)).toBe(true)
    expect(pollsUntilAsked(lookup)).toBe(Infinity)
  })

  it('gives a scene that loads late its title, then asks no more', () => {
    const lookup = createSceneLookup()
    publishEmpty(lookup)
    expect(pollsUntilAsked(lookup)).toBe(6)
    expect(lookup.settle(HERE, true)).toBe(true)
    expect(pollsUntilAsked(lookup)).toBe(Infinity)
  })

  it('asks less and less often while the parcel stays empty, and publishes nothing', () => {
    const lookup = createSceneLookup()
    publishEmpty(lookup)
    const gaps: number[] = []
    for (let recheck = 0; recheck < 7; recheck++) {
      gaps.push(pollsUntilAsked(lookup))
      expect(lookup.settle(HERE, false)).toBe(false)
    }
    expect(gaps).toEqual([6, 12, 24, 48, 96, 96, 96])
  })

  it('starts from the first gap again on the next empty parcel', () => {
    const lookup = createSceneLookup()
    publishEmpty(lookup)
    pollsUntilAsked(lookup)
    pollsUntilAsked(lookup)
    publishEmpty(lookup, THERE)
    expect(pollsUntilAsked(lookup, THERE)).toBe(6)
  })

  it('asks again after a reset, with the first budget for a miss', () => {
    const lookup = createSceneLookup()
    lookup.poll(HERE)
    lookup.settle(HERE, true)
    lookup.reset()
    expect(lookup.poll(HERE)).toBe(true)
    expect(lookup.settle(HERE, false)).toBe(false)
  })
})
