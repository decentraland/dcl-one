// Whole places and events for the map's tests, from the few fields each test cares about.

import type { EventData } from '../engine/protocol'
import type { DiscoverPlace } from '../features/places/placesApi'

export const placeOf = (over: Partial<DiscoverPlace> & Pick<DiscoverPlace, 'id' | 'title' | 'positions'>): DiscoverPlace => ({
  description: '',
  image: '',
  owner: null,
  base_position: over.positions[0],
  ...over
})

export const eventOf = (over: Partial<EventData> & Pick<EventData, 'id' | 'name' | 'x' | 'y'>): EventData => ({
  description: null,
  image: null,
  image_vertical: null,
  start_at: null,
  finish_at: null,
  next_start_at: null,
  next_finish_at: null,
  duration: null,
  server: null,
  world: false,
  user_name: null,
  scene_name: null,
  highlighted: false,
  live: false,
  attending: false,
  total_attendees: 0,
  recurrent: false,
  recurrent_dates: [],
  connected_addresses: [],
  community_id: null,
  place_id: null,
  ...over
})
