import { describe, expect, it } from 'vitest'
import { eventCalendarLink, eventCopyLink, splitEvents, startsInText, type LobbyEvent } from '../features/lobby/lobbyEvents'

const NOW = Date.parse('2026-11-13T12:00:00Z')
const event = (id: string, minutes: number, live = false): LobbyEvent => ({
  id,
  name: `Event ${id}`,
  x: 1,
  y: 2,
  live,
  start_at: new Date(NOW + minutes * 60_000).toISOString(),
  next_start_at: new Date(NOW + minutes * 60_000).toISOString(),
  next_finish_at: new Date(NOW + (minutes + 60) * 60_000).toISOString()
})
const site = (path: string): string => `https://decentraland.org${path}`

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

  it('links a live event to its parcel and an upcoming one to its page', () => {
    expect(eventCopyLink(event('l', -1, true), site, '0xabc')).toBe('https://decentraland.org/jump/?position=1,2&referrer=0xabc')
    expect(eventCopyLink(event('u', 10), site)).toBe('https://decentraland.org/whats-on/?id=u')
  })

  it('drafts a calendar entry with UTC stamps', () => {
    const link = eventCalendarLink(event('u', 60), site)
    expect(link).toContain('dates=20261113T130000Z/20261113T140000Z')
    expect(link).toContain('jump in: https%3A%2F%2Fdecentraland.org%2Fjump%2F%3Fposition%3D1%2C2')
  })
})
