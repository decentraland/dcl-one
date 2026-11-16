import type { EventData } from '@explorer-core/engine/protocol'

const HOUR = 60 * 60 * 1000

export function eventAt(id: string, start: Date, extra: Partial<EventData> = {}): EventData {
  const duration = extra.duration ?? 2 * HOUR
  return {
    id,
    name: `Event ${id}`,
    description: 'A description.',
    image: null,
    image_vertical: null,
    start_at: start.toISOString(),
    finish_at: new Date(start.getTime() + duration).toISOString(),
    next_start_at: start.toISOString(),
    next_finish_at: new Date(start.getTime() + duration).toISOString(),
    duration,
    x: 10,
    y: -20,
    server: null,
    world: false,
    user_name: 'Host',
    scene_name: 'Scene',
    highlighted: false,
    live: false,
    attending: false,
    total_attendees: 0,
    recurrent: false,
    recurrent_dates: [],
    connected_addresses: [],
    community_id: null,
    place_id: null,
    ...extra
  }
}
