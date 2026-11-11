import { beforeEach, describe, expect, it, vi } from "vitest";

const server = vi.hoisted(() => ({
  loadJumpPlace: vi.fn(),
  loadSceneCreator: vi.fn(),
  worldExists: vi.fn(),
  loadJumpEvent: vi.fn(),
  loadCreator: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/jump/jump.server", () => server);

import { loader as jumpEventLoader } from "./jump.event";
import { loader as eventsLoader } from "./jump.events._index";
import { loader as placesLoader } from "./jump.places._index";

const WALLET = "0x" + "ab".repeat(20);

const args = (path: string) => ({ request: new Request(`https://site.example${path}`) }) as never;

async function redirectOf(load: Promise<unknown>) {
  const thrown = await load.then(
    () => null,
    (error: unknown) => error,
  );
  expect(thrown).toBeInstanceOf(Response);
  const response = thrown as Response;
  return { status: response.status, location: response.headers.get("location") };
}

const placeRow = {
  title: "Plaza",
  description: null,
  image: null,
  base_position: "10,20",
  owner: null,
  contact_name: "Studio",
  user_count: 3,
  world: false,
  world_name: null,
};

const eventRow = {
  id: "e1",
  name: "Party",
  image: null,
  description: "Come",
  next_start_at: "2099-01-01T10:00:00.000Z",
  next_finish_at: "2099-01-01T12:00:00.000Z",
  coordinates: [5, -3] as [number, number],
  user: WALLET,
  user_name: "host",
  total_attendees: 7,
  live: false,
  world: false,
  server: null,
  deleted_by_user: false,
  deleted_by_admin: false,
};

beforeEach(() => {
  for (const fn of Object.values(server)) fn.mockReset();
  server.loadSceneCreator.mockResolvedValue(null);
  server.loadCreator.mockResolvedValue(null);
  server.worldExists.mockResolvedValue(true);
});

describe("/jump/places loader", () => {
  it("should send a malformed position to the invalid page", async () => {
    await expect(redirectOf(placesLoader(args("/jump/places?position=nope")))).resolves.toEqual({
      status: 302,
      location: "/jump/places/invalid",
    });
    expect(server.loadJumpPlace).not.toHaveBeenCalled();
  });

  it("should send a World the worlds server does not know to the invalid page", async () => {
    server.worldExists.mockResolvedValue(false);
    server.loadJumpPlace.mockResolvedValue(placeRow);
    await expect(redirectOf(placesLoader(args("/jump?realm=gone.dcl.eth")))).resolves.toMatchObject({
      location: "/jump/places/invalid",
    });
  });

  it("should still render a World card when the worlds server is down", async () => {
    server.worldExists.mockRejectedValue(new Error("502"));
    server.loadJumpPlace.mockResolvedValue({ ...placeRow, world: true, world_name: "foo.dcl.eth" });
    const data = await placesLoader(args("/jump?realm=foo.dcl.eth"));
    expect(data.card.location).toEqual({ world: "foo.dcl.eth", parcel: "10, 20" });
  });

  it("should send a failed catalogue read to the invalid page", async () => {
    server.loadJumpPlace.mockRejectedValue(new Error("500"));
    await expect(redirectOf(placesLoader(args("/jump/places?position=1,2")))).resolves.toMatchObject({
      location: "/jump/places/invalid",
    });
  });

  it("should render a generic card for an uncatalogued parcel", async () => {
    server.loadJumpPlace.mockResolvedValue(null);
    const data = await placesLoader(args("/jump/places?position=7,8"));
    expect(data.card).toMatchObject({ title: "", creator: { name: "Unknown" }, location: { parcel: "7, 8" } });
    expect(data.card.jumpInHref).toContain("position=7%2C8");
  });

  it("should prefer the scene deployer over the catalogue contact", async () => {
    server.loadJumpPlace.mockResolvedValue(placeRow);
    server.loadSceneCreator.mockResolvedValue({ address: WALLET, name: "builder" });
    const data = await placesLoader(args("/jump/places?position=10,20"));
    expect(data.card.creator).toMatchObject({ name: "builder", href: `/profile/${WALLET}` });
    expect(data.card.badge).toEqual({ kind: "players", count: 3 });
  });

  it("should keep the catalogue contact when the deployer lookup fails", async () => {
    server.loadJumpPlace.mockResolvedValue(placeRow);
    server.loadSceneCreator.mockRejectedValue(new Error("down"));
    const data = await placesLoader(args("/jump/places?position=10,20"));
    expect(data.card.creator.name).toBe("Studio");
  });
});

describe("/jump/events loader", () => {
  it("should send a malformed position without an id to the invalid page", async () => {
    await expect(redirectOf(eventsLoader(args("/jump/events?position=x")))).resolves.toMatchObject({
      location: "/jump/events/invalid",
    });
  });

  it("should send an unknown event to the invalid page", async () => {
    server.loadJumpEvent.mockResolvedValue(null);
    await expect(redirectOf(eventsLoader(args("/jump/events?id=nope")))).resolves.toMatchObject({
      location: "/jump/events/invalid",
    });
  });

  it("should send a failed events read to the invalid page", async () => {
    server.loadJumpEvent.mockRejectedValue(new Error("500"));
    await expect(redirectOf(eventsLoader(args("/jump/events?id=e1")))).resolves.toMatchObject({
      location: "/jump/events/invalid",
    });
  });

  it("should offer the upcoming actions with a calendar link", async () => {
    server.loadJumpEvent.mockResolvedValue(eventRow);
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.state).toBe("upcoming");
    expect(data.calendarHref).toContain("calendar.google.com");
    expect(data.card.jumpInHref).toBeUndefined();
    expect(data.card.badge).toEqual({ kind: "attendees", count: 7 });
  });

  it("should show a live event's scene occupancy and Jump In", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, live: true });
    server.loadJumpPlace.mockResolvedValue({ ...placeRow, user_count: 12 });
    const data = await eventsLoader(args("/jump/events?position=5,-3"));
    expect(data.state).toBe("live");
    expect(data.card.badge).toEqual({ kind: "live", count: 12 });
    expect(data.card.jumpInHref).toContain("position=5%2C-3");
  });

  it("should mark a deleted event and drop its actions", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, live: true, deleted_by_user: true });
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.state).toBe("deleted");
    expect(data.card.jumpInHref).toBeUndefined();
  });

  it("should offer only Explore events once the event has ended", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_finish_at: "2000-01-01T00:00:00.000Z" });
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.state).toBe("ended");
    expect(data.card.when).toEqual({ label: "Event has ended", ended: true });
  });
});

describe("/jump/event", () => {
  it("should permanently redirect to /jump/events keeping the query", () => {
    const response = jumpEventLoader(args("/jump/event?position=0,5&realm=foo.dcl.eth"));
    expect(response.status).toBe(308);
    expect(response.headers.get("location")).toBe("/jump/events?position=0,5&realm=foo.dcl.eth");
  });
});
