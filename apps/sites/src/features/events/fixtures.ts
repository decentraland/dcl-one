import type { Event } from "@/data/lib/catalyst/places/events";
import type { Place } from "@/data/lib/catalyst/schema";

/** Test records shaped as the events and places services return them. */

export function eventRecord(overrides: Partial<Event> = {}): Event {
  return {
    id: "e1",
    name: "Poker Night",
    image: "https://example.org/poster.png",
    image_vertical: null,
    description: "Cards and chips.",
    start_at: "2099-06-01T18:00:00Z",
    finish_at: "2099-06-01T20:00:00Z",
    next_start_at: "2099-06-01T18:00:00Z",
    next_finish_at: "2099-06-01T20:00:00Z",
    duration: 7_200_000,
    all_day: false,
    x: 10,
    y: 20,
    position: [10, 20],
    coordinates: [10, 20],
    url: null,
    user: "0x1111111111111111111111111111111111111111",
    user_name: "OtoLam",
    scene_name: null,
    estate_name: null,
    live: false,
    highlighted: false,
    trending: false,
    recurrent: false,
    recurrent_frequency: null,
    total_attendees: 3,
    place_id: null,
    world: false,
    server: null,
    approved: true,
    rejected: false,
    deleted_by_user: false,
    deleted_by_admin: false,
    ...overrides,
  };
}

export function placeRecord(overrides: Partial<Place> = {}): Place {
  return {
    id: "p1",
    title: "Meta GamiMall",
    description: "Shops.",
    image: "https://example.org/place.png",
    owner: "0x2222222222222222222222222222222222222222",
    positions: ["1,95"],
    base_position: "1,95",
    updated_at: null,
    created_at: null,
    contact_name: "MetaDoge",
    categories: [],
    highlighted: false,
    highlighted_image: null,
    user_count: 6,
    user_visits: 0,
    favorites: 0,
    likes: 0,
    like_rate: null,
    world: false,
    world_name: null,
    ...overrides,
  };
}
