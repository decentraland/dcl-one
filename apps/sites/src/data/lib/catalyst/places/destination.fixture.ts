import type { Destination } from "./destinations";

/** A destinations row for tests: a visible, empty Genesis City scene unless overridden. */
export function destination(overrides: Partial<Destination> = {}): Destination {
  return {
    id: "plaza",
    title: "Plaza",
    description: null,
    image: "https://example.com/cover.png",
    owner: "0x00000000000000000000000000000000000000aa",
    positions: ["1,2"],
    base_position: "1,2",
    updated_at: null,
    created_at: null,
    contact_name: "Studio",
    categories: ["social"],
    highlighted: false,
    highlighted_image: null,
    user_count: 0,
    user_visits: 0,
    favorites: 0,
    likes: 0,
    like_rate: null,
    world: false,
    world_name: null,
    live: false,
    live_event_name: null,
    creator_address: null,
    user_favorite: false,
    ...overrides,
  };
}
