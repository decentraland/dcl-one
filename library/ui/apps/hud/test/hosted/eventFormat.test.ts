import { describe, expect, it } from 'vitest'
import {
  addDays,
  calendarRange,
  columnOccupancy,
  dayLabel,
  dayRange,
  emptySlotCount,
  eventCalendarUrl,
  eventCardSize,
  eventCopyLink,
  eventPlaceText,
  eventShareUrl,
  eventTimeText,
  rangeParam,
  scheduleDates,
  scheduleText,
  startOfDay
} from '../../features/events/eventFormat'
import { BASE_DOMAIN } from '@hud-host'
import { eventAt } from './eventFixtures'

// local dates throughout, so the suite reads the same in any time zone
const now = new Date(2026, 8, 8, 12, 0, 0)
const today = startOfDay(now)

describe('event labels', () => {
  it('names today and tomorrow, and writes other days as "Thu, Sep 10"', () => {
    expect(dayLabel(today, now)).toBe('Today')
    expect(dayLabel(addDays(today, 1), now)).toBe('Tomorrow')
    expect(dayLabel(addDays(today, 2), now)).toBe('Thu, Sep 10')
    expect(dayLabel(new Date(2026, 9, 5), now)).toBe('Mon, Oct 05')
  })

  it('writes an upcoming event as its hour on a card and its day and hour in the detail', () => {
    const event = eventAt('a', new Date(2026, 8, 9, 16, 5))
    expect(eventTimeText(event, true, now)).toBe('4:05PM')
    expect(eventTimeText(event, false, now)).toBe('WED, SEP 09 @ 4:05PM')
    expect(eventTimeText(eventAt('b', new Date(2026, 8, 9, 0, 30)), true, now)).toBe('12:30AM')
  })

  it('writes a live event as how long ago it started, in the largest whole unit', () => {
    const live = (minutesAgo: number): string => eventTimeText(eventAt('a', new Date(now.getTime() - minutesAgo * 60_000), { live: true }), true, now)
    expect(live(5)).toBe('Started 5 min ago')
    expect(live(59)).toBe('Started 59 min ago')
    expect(live(150)).toBe('Started 2 hour ago')
    expect(live(26 * 60)).toBe('Started 1 day ago')
  })

  it('writes a recurrent date with its end and the whole hours from UTC', () => {
    const start = new Date(2026, 7, 30, 16, 0)
    const hours = Math.trunc(-start.getTimezoneOffset() / 60)
    expect(scheduleText(start, 60 * 60_000)).toBe(`Sunday, Aug 30 from 04:00pm to 05:00pm (UTC${hours >= 0 ? '+' : ''}${hours})`)
  })

  it('lists the recurrent dates from the next start on', () => {
    const next = new Date(2026, 8, 9, 18)
    const event = eventAt('a', next, { recurrent_dates: [addDays(next, -7), next, addDays(next, 7)].map((d) => d.toISOString()) })
    expect(scheduleDates(event)).toHaveLength(2)
  })

  it('names the place from the places service, or the scene when it has none', () => {
    const land = eventAt('a', now, { x: 3, y: 4, scene_name: 'Scene' })
    expect(eventPlaceText(land, undefined)).toBe('Scene (3,4)')
    expect(eventPlaceText(land, { id: 'p', title: 'Plaza', world_name: null, base_position: '0,0' })).toBe('Plaza (3,4)')
    const world = eventAt('b', now, { world: true, server: 'fest.dcl.eth' })
    expect(eventPlaceText(world, { id: 'p', title: 'Fest', world_name: 'fest.dcl.eth', base_position: null })).toBe('Fest (fest.dcl.eth)')
  })
})

describe('event ranges', () => {
  it('sends dates as UTC to the second', () => {
    expect(rangeParam(new Date(Date.UTC(2026, 8, 8, 3, 4, 5, 678)))).toBe('2026-09-08T03:04:05Z')
  })

  it('reads a calendar from the day before its first to the last second of its last day', () => {
    const range = calendarRange(today, 5)
    expect(range.from).toBe(rangeParam(addDays(today, -1)))
    expect(range.to).toBe(rangeParam(new Date(addDays(today, 5).getTime() - 1000)))
  })

  it('reads a day from its midnight to its last second', () => {
    const range = dayRange(today)
    expect(range.from).toBe(rangeParam(today))
    expect(range.to).toBe(rangeParam(new Date(addDays(today, 1).getTime() - 1000)))
  })
})

describe('event cards', () => {
  it('gives the big card to a highlighted, live or reminded event, or one of a community the user is in', () => {
    const plain = eventAt('a', now)
    expect(eventCardSize(plain, false, false)).toBe('small')
    expect(eventCardSize(plain, false, true)).toBe('big')
    expect(eventCardSize(plain, true, false)).toBe('big')
    expect(eventCardSize({ ...plain, highlighted: true }, false, false)).toBe('big')
    expect(eventCardSize({ ...plain, live: true }, false, false)).toBe('big')
    expect(eventCardSize({ ...plain, attending: true }, false, false)).toBe('small')
  })

  it('fills a column with three, two or one empty slots by how full it is', () => {
    expect(emptySlotCount(columnOccupancy([]))).toBe(3)
    expect(emptySlotCount(columnOccupancy(['small']))).toBe(3)
    expect(emptySlotCount(columnOccupancy(['big']))).toBe(2)
    expect(emptySlotCount(columnOccupancy(['big', 'small']))).toBe(2)
    expect(emptySlotCount(columnOccupancy(['big', 'big']))).toBe(1)
  })
})

describe('event links', () => {
  const upcoming = eventAt('id-1', new Date(Date.UTC(2026, 8, 9, 18, 0, 0)), { name: 'Rock & Roll', description: 'Bring friends' })

  it('copies the event page for an upcoming event and the jump link, with the referrer, for a live one', () => {
    expect(eventCopyLink(upcoming, '0xme')).toBe(`https://${BASE_DOMAIN}/whats-on/?id=id-1`)
    expect(eventCopyLink({ ...upcoming, live: true }, '0xme')).toBe(`https://${BASE_DOMAIN}/jump/?position=10,-20&referrer=0xme`)
    expect(eventCopyLink({ ...upcoming, live: true, world: true, server: 'fest.dcl.eth' }, null)).toBe(`https://${BASE_DOMAIN}/jump/?realm=fest.dcl.eth`)
  })

  it('shares on X with the name, the hashtag and the link', () => {
    const url = new URL(eventShareUrl(upcoming, null))
    expect(url.origin + url.pathname).toBe('https://twitter.com/intent/tweet')
    expect(url.searchParams.get('text')).toBe('Rock & Roll')
    expect(url.searchParams.get('hashtags')).toBe('DCLPlace')
    expect(url.searchParams.get('url')).toBe(`https://${BASE_DOMAIN}/whats-on/?id=id-1`)
    expect(url.searchParams.get('utm_source')).toBe('bevy-web-client')
    expect(url.searchParams.get('utm_campaign')).toBe('discover')
  })

  it('adds the next occurrence to Google Calendar, or the one asked for', () => {
    const url = new URL(eventCalendarUrl(upcoming, null)!)
    expect(url.origin + url.pathname).toBe('https://calendar.google.com/calendar/r/eventedit')
    expect(url.searchParams.get('text')).toBe('Rock & Roll')
    expect(url.searchParams.get('details')).toBe(`Bring friends\n\njump in: https://${BASE_DOMAIN}/jump/?position=10,-20`)
    expect(url.searchParams.get('dates')).toBe('20260909T180000Z/20260909T200000Z')
    const later = new URL(eventCalendarUrl(upcoming, null, new Date(Date.UTC(2026, 8, 16, 18, 0, 0)))!)
    expect(later.searchParams.get('dates')).toBe('20260916T180000Z/20260916T200000Z')
  })
})
