import { describe, it, expect } from 'vitest'
import { equippedSlots, lookSlots } from '../../bridge-scene/src/domains/emoteSlots'

// The bridge's emote wheel slots (bridge-scene/src/domains/emoteSlots): which 10 slots the Backpack
// shows and an equip edits, from the look's emotes and the live profile's.

const DEFAULTS = equippedSlots([])
const EMPTY = Array.from({ length: 10 }, () => '')
const WAVE = 'urn:decentraland:off-chain:base-emotes:wave'

describe('the wheel slots of a look', () => {
  it('a fresh profile, empty live and in the look, shows the defaults', () => {
    expect(lookSlots(EMPTY, [])).toEqual(DEFAULTS)
    expect(lookSlots(undefined, undefined)).toEqual(DEFAULTS)
  })

  it('a look emptied over a live wheel that still has an emote stays empty', () => {
    expect(lookSlots(EMPTY, ['', WAVE])).toEqual(EMPTY)
  })

  it('an emptied wheel already deployed takes the defaults again', () => {
    expect(lookSlots(EMPTY, EMPTY)).toEqual(DEFAULTS)
  })

  it('a look with an emote keeps its slots as they are, empties too', () => {
    const look = [WAVE, ...EMPTY.slice(1)]
    expect(lookSlots(look, [])).toEqual(look)
    expect(lookSlots(look, [WAVE])).toEqual(look)
  })
})
