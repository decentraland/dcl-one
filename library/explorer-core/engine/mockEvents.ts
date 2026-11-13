// The events the mock bridge answers with under `?mock=1`: a week of them around now, covering
// each card state (live, reminded, highlighted, a community's, recurrent, in a world).

import type { EventData, EventPlace, GetEventsRequest } from './protocol'

const HOUR = 60 * 60 * 1000
const DAY = 24 * HOUR

const NAMES = ['Sunset Rooftop Session', 'Voxel Builders Meetup', 'Trivia Night', 'Open Mic', 'Wearables Runway', 'Parkour Race', 'Pixel Art Workshop', 'Late Night Lounge']
const HOSTS = ['DJTraxNFT', 'Canessa', 'ToxicWaifu', 'Lastraum', 'Chiri', 'BillyTeacoin']

function event(id: string, startAt: number, extra: Partial<EventData> = {}): EventData {
  const duration = extra.duration ?? 2 * HOUR
  const n = Number(id.replace(/\D/g, '')) || 0
  return {
    id,
    name: NAMES[n % NAMES.length],
    description: 'Come by, bring a friend and stay for the after party.\n\nDoors open ten minutes before the start.',
    image: `https://picsum.photos/seed/event${id}/680/368`,
    image_vertical: null,
    start_at: new Date(startAt).toISOString(),
    finish_at: new Date(startAt + duration).toISOString(),
    next_start_at: new Date(startAt).toISOString(),
    next_finish_at: new Date(startAt + duration).toISOString(),
    duration,
    x: (n * 7) % 100,
    y: -((n * 13) % 100),
    server: null,
    world: false,
    user_name: HOSTS[n % HOSTS.length],
    scene_name: 'Genesis Plaza',
    highlighted: false,
    live: false,
    attending: false,
    total_attendees: 3 + n,
    recurrent: false,
    recurrent_dates: [new Date(startAt).toISOString()],
    connected_addresses: [],
    community_id: null,
    place_id: `place-${n % 3}`,
    ...extra
  }
}

// Reminders set in this mock session, kept over the fixtures (which are rebuilt on each read).
const reminders = new Map<string, boolean>()

function all(now: number): EventData[] {
  const midnight = new Date(now)
  midnight.setHours(0, 0, 0, 0)
  const today = midnight.getTime()
  const friends = ['0x5854cce95d5e25817b41f4c41f06b695a83bc495', '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4', '0x1e105bb213754519903788022b962fe2b9c4b263']
  const list: EventData[] = [
    // live since yesterday: shows in today's column after today's live events
    event('e1', now - 26 * HOUR, { live: true, duration: 30 * HOUR, connected_addresses: [friends[0], '0xaa'], name: 'Marathon Stream' }),
    event('e2', now - 40 * 60 * 1000, { live: true, connected_addresses: [...friends, '0xab', '0xac', '0xad'], community_id: 'c1' }),
    event('e3', now - 5 * 60 * 1000, { live: true, world: true, server: 'festival.dcl.eth', name: 'Festival Main Stage' }),
    event('e4', now + 2 * HOUR, { attending: true }),
    event('e5', now + 3 * HOUR),
    event('e6', now + 4 * HOUR, { community_id: 'c5' }),
    // a community the mock user is not in: no community thumbnail
    event('e7', now + 5 * HOUR, { community_id: 'c3' }),
    event('e8', today + DAY + 18 * HOUR, {
      recurrent: true,
      recurrent_dates: [0, 7, 14, 21].map((d) => new Date(today + (d + 1) * DAY + 18 * HOUR).toISOString()),
      name: 'Weekly Poker Night'
    }),
    event('e9', today + DAY + 20 * HOUR),
    event('e10', today + 2 * DAY + 10 * HOUR, { highlighted: true, image_vertical: 'https://picsum.photos/seed/eventv10/716/1640', name: 'Metaverse Music Festival' }),
    event('e11', today + 2 * DAY + 15 * HOUR),
    event('e12', today + 2 * DAY + 16 * HOUR),
    event('e13', today + 2 * DAY + 17 * HOUR),
    event('e14', today + 2 * DAY + 19 * HOUR),
    event('e15', today + 4 * DAY + 12 * HOUR, { highlighted: true, name: 'Art Week Opening' }),
    event('e16', today + 6 * DAY + 21 * HOUR, { world: true, server: 'lounge.dcl.eth' })
    // day 3 and day 5 have none: the empty column and the empty day
  ]
  return list.map((e) => (reminders.has(e.id) ? { ...e, attending: reminders.get(e.id) === true } : e))
}

export function mockEventList(msg: GetEventsRequest, now: number = Date.now()): EventData[] {
  const events = all(now)
  if (msg.list === 'live') return events.filter((e) => e.live)
  if (msg.list === 'highlight') return events.filter((e) => e.highlighted).slice(msg.offset ?? 0, (msg.offset ?? 0) + (msg.limit ?? 10))
  const from = msg.from != null ? Date.parse(msg.from) : -Infinity
  const to = msg.to != null ? Date.parse(msg.to) : Infinity
  return events.filter((e) => {
    const start = Date.parse(e.next_start_at ?? '')
    return start >= from && start <= to
  })
}

export function setMockReminder(eventId: string, attending: boolean): void {
  reminders.set(eventId, attending)
}

export function mockEventPlaces(ids: string[]): EventPlace[] {
  const known: Record<string, EventPlace> = {
    'place-0': { id: 'place-0', title: 'Genesis Plaza', world_name: null, base_position: '0,0' },
    'place-1': { id: 'place-1', title: 'Festival Grounds', world_name: 'festival.dcl.eth', base_position: null }
    // place-2 is unknown to the places service: the detail falls back to the event's scene name
  }
  return ids.flatMap((id) => (known[id] != null ? [known[id]] : []))
}
