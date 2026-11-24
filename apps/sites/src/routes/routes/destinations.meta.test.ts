// @vitest-environment jsdom
import { QueryClient, dehydrate } from "@tanstack/react-query";
import type { MetaDescriptor } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

import type { JumpPlaceRecord } from "@/data/lib/catalyst/jump/jump";
import type { Event } from "@/data/lib/catalyst/places/events";
import { eventsPageKeys, type LiveNowData } from "@/data/lib/catalyst/places/events-page";
import type { Place } from "@/data/lib/catalyst/schema";

import { DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import { meta as eventsMeta } from "./events";
import { meta as jumpIndexMeta } from "./jump._index";
import { meta as jumpEventsMeta } from "./jump.events._index";
import { meta as jumpPlacesMeta } from "./jump.places._index";
import { meta as placesMeta } from "./places";
import { meta as placeMeta } from "./places_.$id";
import { meta as worldMeta } from "./places_.world.$name";

const BASE = "https://dcl.test";
const EVENT_ID = "6f1c2c5e-3d0b-4c7a-9a43-0d6d1e8a9b10";

const values = (tags: MetaDescriptor[], key: string) =>
  tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);
const title = (tags: MetaDescriptor[]) => (tags[0] as { title: string }).title;
const canonical = (tags: MetaDescriptor[]) => (tags.find((t) => "rel" in t && t.rel === "canonical") as { href: string } | undefined)?.href;
const jsonLd = (tags: MetaDescriptor[]) => tags.flatMap((t) => ("script:ld+json" in t ? [t["script:ld+json"] as Record<string, unknown>] : []));

const matches = (base: string | undefined) => [{ id: "root", loaderData: { publicPageBase: base } }];
const args = (loaderData: unknown, path: string, extra: Record<string, unknown> = {}) => {
  const url = new URL(path, BASE);
  return { loaderData, location: { pathname: url.pathname, search: url.search }, matches: matches(BASE), ...extra } as never;
};

function event(over: Partial<Event> = {}): Event {
  return {
    id: EVENT_ID,
    name: "Sunset Rave",
    image: "https://events.test/sunset.png",
    image_vertical: null,
    description: "Music  and\n lights.",
    start_at: "2026-11-01T20:00:00.000Z",
    finish_at: "2026-11-01T22:00:00.000Z",
    next_start_at: "2026-11-21T20:00:00.000Z",
    next_finish_at: "2026-11-21T22:00:00.000Z",
    all_day: false,
    x: -9,
    y: 12,
    position: [-9, 12],
    coordinates: [-9, 12],
    url: null,
    user_name: "DJ Ana",
    scene_name: "Club Neon",
    estate_name: null,
    live: false,
    highlighted: false,
    trending: false,
    recurrent: false,
    recurrent_frequency: null,
    total_attendees: 3,
    place_id: null,
    world: false,
    approved: true,
    rejected: false,
    ...over,
  };
}

function place(over: Partial<Place> = {}): Place {
  return {
    id: "a1b2c3d4-0000-4000-8000-000000000001",
    title: "Genesis Plaza",
    description: "The heart of the city.",
    image: "https://places.test/plaza.png",
    owner: null,
    positions: ["0,0", "0,1"],
    base_position: "0,0",
    updated_at: null,
    created_at: null,
    contact_name: null,
    categories: ["social"],
    highlighted: false,
    highlighted_image: null,
    user_count: 4,
    user_visits: 10,
    favorites: 1,
    likes: 1,
    like_rate: 0.9,
    world: false,
    world_name: null,
    ...over,
  };
}

const jumpPlace = (over: Partial<JumpPlaceRecord> = {}): JumpPlaceRecord => ({
  title: "Skychaser",
  description: "Climb the towers.",
  image: "https://places.test/sky.png",
  base_position: "0,0",
  owner: null,
  contact_name: null,
  user_count: 0,
  world: true,
  world_name: "skychaser.dcl.eth",
  ...over,
});

/** The /events loader's dehydrated reads, as the page's first document carries them. */
function eventsState(reads: [readonly unknown[], unknown][]) {
  const client = new QueryClient();
  for (const [key, data] of reads) client.setQueryData(key, data);
  return { dehydratedState: dehydrate(client) };
}

describe("events meta", () => {
  describe("when the link opens an event the loader read", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      tags = eventsMeta(args(eventsState([[eventsPageKeys.detail(EVENT_ID), event()]]), `/events?id=${EVENT_ID}&utm_source=x`));
    });

    it("should title the card with the event and its scene", () => {
      expect(title(tags)).toBe("Sunset Rave at Club Neon | Decentraland");
      expect(values(tags, "og:title")).toEqual(["Sunset Rave at Club Neon | Decentraland"]);
      expect(values(tags, "twitter:title")).toEqual(["Sunset Rave at Club Neon | Decentraland"]);
    });

    it("should describe it on one line with the event's image as a large card", () => {
      expect(values(tags, "og:description")).toEqual(["Music and lights."]);
      expect(values(tags, "og:image")).toEqual(["https://events.test/sunset.png"]);
      expect(values(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(values(tags, "og:type")).toEqual(["website"]);
    });

    it("should be canonical at the events page opened on the event", () => {
      expect(canonical(tags)).toBe(`${BASE}/events?id=${EVENT_ID}`);
      expect(values(tags, "og:url")).toEqual([`${BASE}/events?id=${EVENT_ID}`]);
    });

    it("should carry its schema.org Event at the next occurrence", () => {
      expect(jsonLd(tags)).toEqual([
        expect.objectContaining({
          "@type": "Event",
          name: "Sunset Rave",
          url: `${BASE}/events?id=${EVENT_ID}`,
          startDate: "2026-11-21T20:00:00.000Z",
          endDate: "2026-11-21T22:00:00.000Z",
          location: { "@type": "VirtualLocation", url: `${BASE}/events?id=${EVENT_ID}`, name: "Club Neon, Genesis City parcel -9,12" },
          organizer: { "@type": "Person", name: "DJ Ana" },
        }),
      ]);
    });
  });

  describe("when the event has no scene and an image that is not http(s)", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      const read = event({ scene_name: null, image: "ipfs://bafy", description: null });
      tags = eventsMeta(args(eventsState([[eventsPageKeys.detail(EVENT_ID), read]]), `/events?id=${EVENT_ID}`));
    });

    it("should title it with the event's name alone", () => {
      expect(title(tags)).toBe("Sunset Rave | Decentraland");
    });

    it("should fall back to the section description and the shared image", () => {
      expect(values(tags, "og:description")).toEqual(["Live events and places happening right now in Decentraland."]);
      expect(values(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    });
  });

  describe("when the event has no image", () => {
    it("should use the shared image", () => {
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.detail(EVENT_ID), event({ image: null })]]), `/events?id=${EVENT_ID}`));
      expect(values(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    });
  });

  describe("when the event record does not say it was approved", () => {
    it("should keep the card but carry no structured data", () => {
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.detail(EVENT_ID), event({ approved: undefined })]]), `/events?id=${EVENT_ID}`));
      expect(title(tags)).toBe("Sunset Rave at Club Neon | Decentraland");
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the events service does not know the id", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      tags = eventsMeta(args(eventsState([[eventsPageKeys.detail(EVENT_ID), null]]), `/events?id=${EVENT_ID}`));
    });

    it("should keep the section card, canonical at the section, with no structured data", () => {
      expect(title(tags)).toBe("Events in Decentraland | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/events`);
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the link names a World the loader read", () => {
    it("should describe the World and be canonical at its world link", () => {
      const target = { world: "skychaser.dcl.eth" };
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.place(target), jumpPlace()]]), "/events?world=SkyChaser"));
      expect(title(tags)).toBe("Skychaser | Decentraland");
      expect(values(tags, "og:image")).toEqual(["https://places.test/sky.png"]);
      expect(canonical(tags)).toBe(`${BASE}/events?world=skychaser.dcl.eth`);
    });
  });

  describe("when the link names a World the catalogue has no row for", () => {
    it("should fall back to the World's own wording", () => {
      const target = { world: "nowhere.dcl.eth" };
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.place(target), null]]), "/events?world=nowhere"));
      expect(title(tags)).toBe("Visit nowhere.dcl.eth in Decentraland | Decentraland");
      expect(values(tags, "og:description")).toEqual(["Discover nowhere.dcl.eth, a Decentraland world."]);
    });
  });

  describe("when the link names a parcel the catalogue has no place for", () => {
    it("should fall back to the coordinates", () => {
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.place({ position: [12, -3] }), null]]), "/events?position=12,-3"));
      expect(title(tags)).toBe("Explore (12,-3) in Decentraland | Decentraland");
      expect(values(tags, "og:description")).toEqual(["Discover what's happening at coordinates 12,-3 in Decentraland."]);
      expect(canonical(tags)).toBe(`${BASE}/events?position=12,-3`);
    });
  });

  describe("when the link names no target and the loader read the live and upcoming events", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      const live: LiveNowData = { events: [event()], places: [] };
      const upcoming = [event({ id: "7f1c2c5e-3d0b-4c7a-9a43-0d6d1e8a9b11", name: "Trivia Night" })];
      tags = eventsMeta(args(eventsState([[eventsPageKeys.live(), live], [eventsPageKeys.upcoming(), upcoming]]), "/events"));
    });

    it("should carry the section card at the section's canonical", () => {
      expect(title(tags)).toBe("Events in Decentraland | Decentraland");
      expect(values(tags, "og:description")).toEqual(["Live events and places happening right now in Decentraland."]);
      expect(canonical(tags)).toBe(`${BASE}/events`);
    });

    it("should list both reads as ItemLists", () => {
      const [graph] = jsonLd(tags);
      expect(graph["@graph"]).toEqual([
        expect.objectContaining({ name: "Live now", numberOfItems: 1, itemListElement: [{ "@type": "ListItem", position: 1, url: `${BASE}/events?id=${EVENT_ID}`, name: "Sunset Rave" }] }),
        expect.objectContaining({ name: "Upcoming in the next 24 hours", numberOfItems: 1 }),
      ]);
    });
  });

  describe("when one of the listing reads failed", () => {
    it("should carry no ItemList rather than a partial one", () => {
      const tags = eventsMeta(args(eventsState([[eventsPageKeys.upcoming(), [event()]]]), "/events"));
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the root has no public base", () => {
    it("should keep the card without a canonical or structured data", () => {
      const tags = eventsMeta({ ...(args(eventsState([[eventsPageKeys.detail(EVENT_ID), event()]]), `/events?id=${EVENT_ID}`) as object), matches: matches(undefined) } as never);
      expect(title(tags)).toBe("Sunset Rave at Club Neon | Decentraland");
      expect(canonical(tags)).toBeUndefined();
      expect(values(tags, "og:url")).toEqual([]);
      expect(jsonLd(tags)).toEqual([]);
    });
  });
});

describe("jump meta", () => {
  describe("when the event jump opens an event", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      const shareEvent = { name: "Sunset Rave", scene_name: "Club Neon", description: "Music.", image: "data:image/png;base64,AA" };
      tags = jumpEventsMeta(args({ shareEvent }, `/jump/events?id=${EVENT_ID}&position=1,1`));
    });

    it("should title the card with the event and its scene, on the shared image", () => {
      expect(title(tags)).toBe("Sunset Rave at Club Neon | Decentraland");
      expect(values(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    });

    it("should be canonical at the event id alone, with no structured data", () => {
      expect(canonical(tags)).toBe(`${BASE}/jump/events?id=${EVENT_ID}`);
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the place jump names a World", () => {
    it("should describe the World from the loader's row and be canonical at its world link", () => {
      const tags = jumpPlacesMeta(args({ place: jumpPlace() }, "/jump/places?realm=skychaser.dcl.eth&position=25,4"));
      expect(title(tags)).toBe("Skychaser | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/jump/places?world=skychaser.dcl.eth`);
    });
  });

  describe("when the place jump names a World the catalogue has no row for", () => {
    it("should fall back to the World's own wording", () => {
      const tags = jumpPlacesMeta(args({ place: null }, "/jump/places?realm=nowhere.dcl.eth"));
      expect(title(tags)).toBe("Visit nowhere.dcl.eth in Decentraland | Decentraland");
    });
  });

  describe("when the place jump names a parcel", () => {
    it("should describe the place that serves it", () => {
      const row = jumpPlace({ title: "Genesis Plaza", world: false, world_name: null, image: null });
      const tags = jumpIndexMeta(args({ place: row }, "/jump?position=0,1"));
      expect(title(tags)).toBe("Genesis Plaza | Decentraland");
      expect(values(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(canonical(tags)).toBe(`${BASE}/jump?position=0,1`);
    });
  });

  describe("when the place jump names no target", () => {
    it("should carry the section card", () => {
      const tags = jumpPlacesMeta(args({ place: jumpPlace({ title: "Genesis Plaza" }) }, "/jump/places/"));
      expect(title(tags)).toBe("Events in Decentraland | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/jump/places`);
    });
  });

  describe("when the jump link carries a malformed position and an id the place jump does not read", () => {
    it("should keep the section card at the bare path", () => {
      const tags = jumpPlacesMeta(args({ place: null }, `/jump/places?position=abc&id=${EVENT_ID}`));
      expect(title(tags)).toBe("Events in Decentraland | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/jump/places`);
    });
  });

  describe("when the event jump has no loader data", () => {
    it("should carry the section card", () => {
      const tags = jumpEventsMeta(args(undefined, `/jump/events?id=${EVENT_ID}`));
      expect(title(tags)).toBe("Events in Decentraland | Decentraland");
    });
  });
});

describe("places meta", () => {
  describe("when the page is the listing", () => {
    it("should carry the places section card at its canonical", () => {
      const tags = placesMeta({ matches: matches(BASE) } as never);
      expect(title(tags)).toBe("Places in Decentraland | Decentraland");
      expect(values(tags, "og:description")).toEqual(["Explore the worlds, games and scenes people are in right now in Decentraland."]);
      expect(values(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(canonical(tags)).toBe(`${BASE}/places`);
    });
  });

  describe("when the place page found its place", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      tags = placeMeta(args({ place: place(), unavailable: false }, "/places/a1b2c3d4-0000-4000-8000-000000000001", { params: { id: "a1b2c3d4-0000-4000-8000-000000000001" } }));
    });

    it("should describe the place", () => {
      expect(title(tags)).toBe("Genesis Plaza | Decentraland");
      expect(values(tags, "og:description")).toEqual(["The heart of the city."]);
      expect(values(tags, "og:image")).toEqual(["https://places.test/plaza.png"]);
    });

    it("should be canonical at its row's id and carry its schema.org Place", () => {
      const url = `${BASE}/places/a1b2c3d4-0000-4000-8000-000000000001`;
      expect(canonical(tags)).toBe(url);
      expect(jsonLd(tags)).toEqual([
        expect.objectContaining({
          "@type": "Place",
          name: "Genesis Plaza",
          url,
          keywords: "social",
          containedInPlace: { "@type": "Place", name: "Genesis City" },
          additionalProperty: [{ "@type": "PropertyValue", name: "parcel", value: "0,0" }],
        }),
      ]);
    });
  });

  describe("when the place has no title", () => {
    it("should fall back to its coordinates and carry no structured data", () => {
      const tags = placeMeta(args({ place: place({ title: null, description: null }), unavailable: false }, "/places/x", { params: { id: "x" } }));
      expect(title(tags)).toBe("Explore (0,0) in Decentraland | Decentraland");
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the place page does not know the place", () => {
    it("should answer the not-found head", () => {
      const tags = placeMeta(args({ place: null, unavailable: false }, "/places/missing", { params: { id: "missing" } }));
      expect(title(tags)).toBe("Page not found | Decentraland");
      expect(values(tags, "robots")).toEqual(["noindex"]);
      expect(values(tags, "og:title")).toEqual([]);
    });
  });

  describe("when the catalogue could not answer", () => {
    it("should keep the section card at the page's own path", () => {
      const tags = placeMeta(args({ place: null, unavailable: true }, "/places/abc", { params: { id: "abc" } }));
      expect(title(tags)).toBe("Places in Decentraland | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/places/abc`);
    });
  });

  describe("when the World page found its World", () => {
    let tags: MetaDescriptor[];

    beforeEach(() => {
      const row = place({ title: "Skychaser", world: true, world_name: "SkyChaser.dcl.eth", base_position: "25,4", categories: [] });
      tags = worldMeta(args({ place: row, unavailable: false }, "/places/world/skychaser.dcl.eth", { params: { name: "SkyChaser.dcl.eth" } }));
    });

    it("should be canonical at the World's lowercased name", () => {
      expect(title(tags)).toBe("Skychaser | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/places/world/skychaser.dcl.eth`);
    });

    it("should name the World, never a Genesis City parcel, in its schema.org Place", () => {
      const [data] = jsonLd(tags);
      expect(data.additionalProperty).toEqual([{ "@type": "PropertyValue", name: "world", value: "skychaser.dcl.eth" }]);
      expect(data.containedInPlace).toBeUndefined();
      expect(data.keywords).toBeUndefined();
    });
  });

  describe("when the root has no public base", () => {
    it("should keep the place's card without a canonical or structured data", () => {
      const tags = placeMeta({ ...(args({ place: place(), unavailable: false }, "/places/x", { params: { id: "x" } }) as object), matches: matches(undefined) } as never);
      expect(title(tags)).toBe("Genesis Plaza | Decentraland");
      expect(canonical(tags)).toBeUndefined();
      expect(jsonLd(tags)).toEqual([]);
    });
  });

  describe("when the World page does not know the World", () => {
    it("should answer the not-found head", () => {
      const tags = worldMeta(args({ place: null, unavailable: false }, "/places/world/nowhere.dcl.eth", { params: { name: "nowhere.dcl.eth" } }));
      expect(title(tags)).toBe("Page not found | Decentraland");
    });
  });

  describe("when the catalogue could not answer for a World", () => {
    it("should fall back to the World's own wording at its path", () => {
      const tags = worldMeta(args({ place: null, unavailable: true }, "/places/world/Nowhere.dcl.eth", { params: { name: "Nowhere.dcl.eth" } }));
      expect(title(tags)).toBe("Visit nowhere.dcl.eth in Decentraland | Decentraland");
      expect(canonical(tags)).toBe(`${BASE}/places/world/nowhere.dcl.eth`);
    });
  });
});
