// What ?mock=1 answers the full-screen map with: a small city of places, events, photos, points of
// interest and connected players, and writes that stick for the session.

import { serviceUrl } from '@hud-host'
import type { Peer } from '../features/map/mapApi'
import type { DiscoverPlace } from '../features/places/placesApi'
import type { Vec2 } from '../features/map/mapCoords'
import type { MapPinInfo } from './generated/index'
import type { EventData, MapPlayer, PageToScene, SceneToPage, SearchPlacesRequest } from './protocol'

const image = (x: number, y: number): string => `${serviceUrl('mapApi')}/v2/parcels/${x}/${y}/map.png?width=512&height=256&size=16`

function block(x0: number, y0: number, w: number, h: number): string[] {
  const out: string[] = []
  for (let x = x0; x < x0 + w; x++) for (let y = y0; y < y0 + h; y++) out.push(`${x},${y}`)
  return out
}

function place(id: string, title: string, positions: string[], categories: string[], users: number, likes: number, dislikes: number): DiscoverPlace {
  const [x, y] = positions[0].split(',').map(Number)
  return {
    id,
    title,
    description: `${title} is a mock place.\nMore at https://decentraland.org`,
    image: image(x, y),
    owner: null,
    contact_name: 'Mock Creator',
    creator_address: '0x5854cce95d5e25817b41f4c41f06b695a83bc495',
    positions,
    base_position: positions[0],
    categories,
    world_name: undefined,
    likes,
    dislikes,
    favorites: 3,
    like_rate: likes + dislikes > 0 ? likes / (likes + dislikes) : null,
    user_count: users,
    user_like: false,
    user_dislike: false,
    user_favorite: false
  }
}

const PLACES: DiscoverPlace[] = [
  place('mock-plaza', 'Genesis Plaza', block(-9, -9, 19, 19), ['poi', 'social'], 42, 900, 40),
  place('mock-casino', 'Tominoya', block(-120, 130, 4, 4), ['poi', 'game'], 12, 300, 60),
  place('mock-museum', 'Museum District', block(16, 80, 6, 4), ['poi', 'art'], 7, 210, 9),
  place('mock-club', 'The Sugar Club', block(-20, 60, 2, 2), ['music', 'social'], 18, 120, 12),
  place('mock-gallery', 'Voxel Gallery', ['52,2'], ['art'], 3, 40, 2),
  place('mock-arcade', 'Pixel Arcade', block(60, -30, 3, 3), ['game'], 9, 77, 11),
  place('mock-boutique', 'Runway 77', ['-40,-12'], ['fashion', 'shop'], 2, 15, 1),
  place('mock-campus', 'DCL University', block(-70, 20, 5, 5), ['education'], 5, 60, 4),
  place('mock-market', 'Wearables Market', block(24, -24, 2, 3), ['shop'], 6, 33, 3),
  place('mock-stadium', 'Vegas Stadium', block(100, 100, 8, 8), ['sports'], 11, 88, 20),
  place('mock-hq', 'Meta HQ', ['-5,40'], ['business'], 1, 9, 0),
  place('mock-garden', 'Zen Garden', ['-12,-40'], ['social'], 0, 4, 0),
  place('mock-tower', 'Crystal Tower', ['8,45'], ['art', 'social'], 4, 21, 2),
  place('mock-dock', 'Harbour Dock', block(-100, -100, 3, 2), ['social'], 2, 12, 5),
  { ...place('mock-world', 'Mock Mansion', ['0,0'], ['social'], 3, 10, 1), world_name: 'mock.dcl.eth' }
]

const HOUR = 3_600_000

function event(id: string, name: string, at: DiscoverPlace, live: boolean, startOffsetMs: number, recurring = 0): EventData {
  const [x, y] = (at.base_position ?? '0,0').split(',').map(Number)
  const start = Date.now() + startOffsetMs
  return {
    id,
    name,
    image: at.image,
    image_vertical: null,
    description: `${name}, hosted at ${at.title}.\nhttps://decentraland.org/events`,
    x,
    y,
    coordinates: [x, y],
    server: null,
    world: false,
    live,
    attending: false,
    total_attendees: 37,
    user_name: 'MockHost',
    scene_name: at.title,
    highlighted: false,
    start_at: new Date(start).toISOString(),
    finish_at: new Date(start + 2 * HOUR).toISOString(),
    next_start_at: new Date(start).toISOString(),
    next_finish_at: new Date(start + 2 * HOUR).toISOString(),
    duration: 2 * HOUR,
    recurrent: recurring > 0,
    recurrent_dates: Array.from({ length: recurring }, (_, i) => new Date(start + i * 24 * HOUR).toISOString()),
    connected_addresses: [],
    community_id: null,
    place_id: at.id
  }
}

const EVENTS: EventData[] = [
  event('mock-event-1', 'Friday Night Party', PLACES[3], true, -HOUR),
  event('mock-event-2', 'Plaza Meetup', PLACES[0], true, -20 * 60_000),
  event('mock-event-3', 'Gallery Opening', PLACES[4], false, 26 * HOUR, 3),
  event('mock-event-4', 'Arcade Tournament', PLACES[5], true, -3 * HOUR),
  event('mock-event-5', 'Plaza Trivia', PLACES[0], false, 50 * HOUR, 2)
]

export const MOCK_POIS: Vec2[] = [
  { x: 0, y: 0 },
  { x: -119, y: 131 },
  { x: 18, y: 81 }
]

export const MOCK_PEERS: Peer[] = [
  { address: '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4', x: 80, z: 64 },
  { address: '0x1e105bb213754519903788022b962fe2b9c4b263', x: -160, z: 320 },
  { address: '0x9f8c2a1b4d6e7f0a3b5c8d9e1f2a4b6c8d0e1f23', x: 960, z: -480 },
  { address: '0x3a1b2c4d5e6f7081920a3b4c5d6e7f8091a2b3c4', x: -1100, z: 300 },
  { address: '0x77a0b1c2d3e4f5061728394a5b6c7d8e9f001122', x: 1600, z: 1600 }
]

/** One pin without an icon, one whose icon fails to load. */
export const MOCK_MAP_PINS: MapPinInfo[] = [
  { id: 'bafymockpx:512', x: 3, y: 2, title: 'Memory Grid', description: 'Win a round', icon: null },
  { id: 'bafymockpx:513', x: -4, y: 5, title: 'Tile Tally', description: 'Win a round', icon: 'https://content.invalid/images/icons/tile_tally.png' }
]

/** Players the mock engine holds near the player, in world metres. */
export const MOCK_NEARBY_PLAYERS: MapPlayer[] = [
  { address: '0x5854cce95d5e25817b41f4c41f06b695a83bc495', x: 24, z: -30 },
  { address: '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4', x: 80, z: 64 }
]

const covers = (p: DiscoverPlace, positions: readonly string[]): boolean => positions.some((pos) => p.positions.includes(pos))

function search(msg: SearchPlacesRequest): { places: DiscoverPlace[]; total: number } {
  const text = msg.search.toLowerCase()
  const category = msg.favorites ? '' : msg.category
  let hits = PLACES.filter((p) => p.title.toLowerCase().includes(text))
  if (category !== '') hits = hits.filter((p) => p.categories?.includes(category))
  if (msg.favorites) hits = hits.filter((p) => p.user_favorite)
  hits = [...hits].sort((a, b) =>
    msg.orderBy === 'like_score' ? Number(b.like_rate ?? 0) - Number(a.like_rate ?? 0) : msg.orderBy === 'created_at' ? 0 : (b.user_count ?? 0) - (a.user_count ?? 0)
  )
  return { places: hits.slice(msg.offset, msg.offset + msg.limit), total: hits.length }
}

function eventsAt(positions: string[], live: boolean): EventData[] {
  return EVENTS.filter((e) => {
    if (live && !e.live) return false
    const at = PLACES.find((p) => p.positions.includes(`${e.x},${e.y}`))
    return positions.includes(`${e.x},${e.y}`) || (at != null && covers(at, positions))
  })
}

const placeById = (id: string): DiscoverPlace | undefined => PLACES.find((x) => x.id === id)

/** The mock's answer to one of the map's requests: the places ones, the events held at parcels and
 *  a reminder on one of the map's events. Null for anything else, which the mock bridge answers. */
export function mockMapReply(msg: PageToScene): SceneToPage | null {
  switch (msg.kind) {
    case 'getPlacesAt':
      if (msg.world != null) return { kind: 'placesAt', request: msg.request, places: PLACES.filter((p) => p.world_name === msg.world) }
      return { kind: 'placesAt', request: msg.request, places: PLACES.filter((p) => (p.world_name ?? '') === '' && covers(p, msg.positions)) }
    case 'searchPlaces':
      return { kind: 'placeSearch', request: msg.request, ...search(msg) }
    case 'getEvents':
      if (msg.positions == null) return null
      return { kind: 'events', request: msg.request, events: eventsAt(msg.positions.map((p) => `${p.x},${p.y}`), msg.list === 'live') }
    case 'setPlaceLike': {
      const p = placeById(msg.placeId)
      if (p == null) return { kind: 'placeLike', request: msg.request, likes: 0, dislikes: 0, user_like: false, user_dislike: false, failed: true }
      p.likes = (p.likes ?? 0) - (p.user_like === true ? 1 : 0) + (msg.like === true ? 1 : 0)
      p.dislikes = (p.dislikes ?? 0) - (p.user_dislike === true ? 1 : 0) + (msg.like === false ? 1 : 0)
      p.user_like = msg.like === true
      p.user_dislike = msg.like === false
      p.like_rate = p.likes + p.dislikes > 0 ? p.likes / (p.likes + p.dislikes) : null
      return { kind: 'placeLike', request: msg.request, likes: p.likes, dislikes: p.dislikes, user_like: p.user_like, user_dislike: p.user_dislike }
    }
    case 'setPlaceFavorite': {
      const p = placeById(msg.placeId)
      if (p == null) return { kind: 'placeFavorite', request: msg.request, favorites: 0, user_favorite: false, failed: true }
      p.favorites = (p.favorites ?? 0) + (msg.favorite ? 1 : 0) - (p.user_favorite === true ? 1 : 0)
      p.user_favorite = msg.favorite
      return { kind: 'placeFavorite', request: msg.request, favorites: p.favorites, user_favorite: p.user_favorite }
    }
    case 'setEventAttending': {
      const e = EVENTS.find((x) => x.id === msg.eventId)
      if (e == null) return null
      e.total_attendees = e.total_attendees + (msg.attending ? 1 : 0) - (e.attending ? 1 : 0)
      e.attending = msg.attending
      return { kind: 'eventAttending', eventId: msg.eventId, attending: msg.attending }
    }
    case 'getPlacePhotos': {
      const p = placeById(msg.placeId)
      const count = p?.id === 'mock-plaza' ? 5 : 0
      const [x, y] = (p?.base_position ?? '0,0').split(',').map(Number)
      const images = Array.from({ length: count }, (_, i) => ({
        id: `mock-photo-${i}`,
        url: image(x + i, y),
        thumbnailUrl: image(x + i, y),
        dateTime: String(Date.now() - i * 24 * HOUR)
      }))
      return { kind: 'placePhotos', request: msg.request, images: images.slice(msg.offset, msg.offset + msg.limit), total: count }
    }
    default:
      return null
  }
}
