import { describe, expect, it } from 'vitest'
import type { EventData } from '../engine/protocol'
import { splitEvents, startsInText } from '../features/lobby/lobbyEvents'

const NOW = Date.parse('2026-11-13T12:00:00Z')
const event = (id: string, minutes: number, live = false): EventData => ({
  id,
  name: `Event ${id}`,
  description: null,
  image: null,
  image_vertical: null,
  start_at: new Date(NOW + minutes * 60_000).toISOString(),
  finish_at: null,
  next_start_at: new Date(NOW + minutes * 60_000).toISOString(),
  next_finish_at: new Date(NOW + (minutes + 60) * 60_000).toISOString(),
  duration: 3_600_000,
  x: 1,
  y: 2,
  server: null,
  world: false,
  user_name: null,
  scene_name: null,
  highlighted: false,
  live,
  attending: false,
  total_attendees: 0,
  recurrent: false,
  recurrent_dates: [],
  connected_addresses: [],
  community_id: null,
  place_id: null
})

describe('lobby events', () => {
  it('splits live from upcoming, the soonest first, at most ten', () => {
    const list = [event('a', 300), event('l', -10, true), ...Array.from({ length: 12 }, (_, i) => event(`u${i}`, 10 + i))]
    const { live, upcoming } = splitEvents(list)
    expect(live.map((e) => e.id)).toEqual(['l'])
    expect(upcoming).toHaveLength(10)
    expect(upcoming[0].id).toBe('u0')
    expect(upcoming.map((e) => e.id)).not.toContain('a')
  })

  it('says when an event starts in the coarsest unit', () => {
    expect(startsInText(event('a', -1), NOW)).toBe('Starting now')
    expect(startsInText(event('a', 20), NOW)).toBe('In 20 min')
    expect(startsInText(event('a', 59.6), NOW)).toBe('In 1 hour')
    expect(startsInText(event('a', 150), NOW)).toBe('In 3 hours')
    expect(startsInText(event('a', 60 * 24 * 3), NOW)).toBe('In 3 days')
  })
})
