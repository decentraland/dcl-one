import { beforeEach, describe, expect, it, vi } from "vitest";

const server = vi.hoisted(() => ({
  loadJumpPlace: vi.fn(),
  loadParcelCreator: vi.fn(),
  loadWorldEntity: vi.fn(),
  loadJumpEvent: vi.fn(),
  loadCreator: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/jump/jump.server", () => server);

import { loader as jumpEventLoader } from "./jump.event";
import { loader as eventsLoader } from "./jump.events._index";
import { loader as placesLoader } from "./jump.places._index";

const WALLET = "0x" + "ab".repeat(20);

const args = (path: string, userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)") =>
  ({ request: new Request(`https://site.example${path}`, { headers: { "user-agent": userAgent } }) }) as never;

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
  server.loadParcelCreator.mockResolvedValue(null);
  server.loadCreator.mockResolvedValue(null);
  server.loadWorldEntity.mockResolvedValue({ id: "bafy" });
  server.loadJumpPlace.mockResolvedValue(null);
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
    server.loadWorldEntity.mockResolvedValue(null);
    server.loadJumpPlace.mockResolvedValue(placeRow);
    await expect(redirectOf(placesLoader(args("/jump?realm=gone.dcl.eth")))).resolves.toMatchObject({
      location: "/jump/places/invalid",
    });
  });

  it("should still render a World card when the worlds server is down", async () => {
    server.loadWorldEntity.mockRejectedValue(new Error("502"));
    server.loadJumpPlace.mockResolvedValue({ ...placeRow, world: true, world_name: "foo.dcl.eth" });
    const data = await placesLoader(args("/jump?realm=foo.dcl.eth"));
    expect(data.card.location).toEqual({ world: "foo.dcl.eth", parcel: "10, 20" });
  });

  it("should name a World's owner from its active scene, looked up once", async () => {
    server.loadWorldEntity.mockResolvedValue({ id: "bafy", metadata: { owner: WALLET } });
    server.loadCreator.mockResolvedValue({ address: WALLET, name: "owner" });
    server.loadJumpPlace.mockResolvedValue({ ...placeRow, world: true, world_name: "foo.dcl.eth" });
    const data = await placesLoader(args("/jump?realm=foo.dcl.eth"));
    expect(data.card.creator.name).toBe("owner");
    expect(server.loadWorldEntity).toHaveBeenCalledOnce();
    expect(server.loadParcelCreator).not.toHaveBeenCalled();
  });

  it("should answer a failed catalogue read with 503, not the invalid page", async () => {
    server.loadJumpPlace.mockRejectedValue(new Error("500"));
    await expect(redirectOf(placesLoader(args("/jump/places?position=1,2")))).resolves.toEqual({ status: 503, location: null });
  });

  it("should treat a realm that is not a World name as Genesis City", async () => {
    server.loadJumpPlace.mockResolvedValue(null);
    const data = await placesLoader(args("/jump?realm=https://evil.example&position=1,2"));
    expect(server.loadWorldEntity).not.toHaveBeenCalled();
    expect(data.card.location).toEqual({ world: undefined, parcel: "1, 2" });
    expect(data.card.jumpInHref).not.toContain("evil");
  });

  it("should render a generic card for an uncatalogued parcel", async () => {
    server.loadJumpPlace.mockResolvedValue(null);
    const data = await placesLoader(args("/jump/places?position=7,8"));
    expect(data.card).toMatchObject({ title: "", creator: { name: "Unknown" }, location: { parcel: "7, 8" } });
    expect(data.card.jumpInHref).toContain("position=7%2C8");
  });

  it("should prefer the scene deployer over the catalogue contact", async () => {
    server.loadJumpPlace.mockResolvedValue(placeRow);
    server.loadParcelCreator.mockResolvedValue({ address: WALLET, name: "builder" });
    const data = await placesLoader(args("/jump/places?position=10,20"));
    expect(data.card.creator).toMatchObject({ name: "builder", href: `/profile/${WALLET}` });
    expect(data.card.badge).toEqual({ kind: "players", count: 3 });
  });

  it("should open the web explorer on desktop and the app on a phone", async () => {
    server.loadJumpPlace.mockResolvedValue(placeRow);
    await expect(placesLoader(args("/jump/places?position=10,20"))).resolves.toMatchObject({ launch: null });
    const android = await placesLoader(args("/jump/places?position=10,20", "Mozilla/5.0 (Linux; Android 15; Pixel 9)"));
    expect(android.launch).toMatchObject({ android: true, href: expect.stringMatching(/^intent:\/\/open\?position=10%2C20#Intent;/) });
    const ios = await placesLoader(args("/jump/places?position=10,20", "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)"));
    expect(ios.launch).toMatchObject({ android: false, href: "decentraland://open?position=10%2C20" });
  });

  it("should keep the catalogue contact when the deployer lookup fails", async () => {
    server.loadJumpPlace.mockResolvedValue(placeRow);
    server.loadParcelCreator.mockRejectedValue(new Error("down"));
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

  it("should answer a failed events read with 503, not the invalid page", async () => {
    server.loadJumpEvent.mockRejectedValue(new Error("500"));
    await expect(redirectOf(eventsLoader(args("/jump/events?id=e1")))).resolves.toEqual({ status: 503, location: null });
  });

  it("should share and calendar the event by its id", async () => {
    server.loadJumpEvent.mockResolvedValue(eventRow);
    const data = await eventsLoader(args("/jump/events?position=5,-3"));
    expect(new URL(data.shareUrl).searchParams.get("id")).toBe("e1");
    expect(data.calendarHref).toContain(encodeURIComponent("id=e1"));
  });

  it("should still show a deleted event's notice when it has no start", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null, deleted_by_admin: true });
    await expect(eventsLoader(args("/jump/events?id=e1"))).resolves.toMatchObject({ state: "deleted" });
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

  it("should fall back to no occupancy when a live event's scene lookup fails", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, live: true });
    server.loadJumpPlace.mockRejectedValue(new Error("500"));
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.card.badge).toEqual({ kind: "live", count: 0 });
  });

  it("should keep the event's own host name when the profile lookup fails", async () => {
    server.loadJumpEvent.mockResolvedValue(eventRow);
    server.loadCreator.mockRejectedValue(new Error("down"));
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.card.creator).toMatchObject({ name: "host", href: `/profile/${WALLET}` });
  });

  it("should render the start in UTC on the server", async () => {
    server.loadJumpEvent.mockResolvedValue(eventRow);
    const data = await eventsLoader(args("/jump/events?id=e1"));
    expect(data.card.when).toEqual({ label: expect.stringContaining("10:00 AM UTC"), ended: false, dateTime: eventRow.next_start_at });
  });

  it("should offer nothing for an event without a start unless it is live", async () => {
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null });
    await expect(eventsLoader(args("/jump/events?id=e1"))).resolves.toMatchObject({ state: "none" });
    server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null, live: true });
    await expect(eventsLoader(args("/jump/events?id=e1"))).resolves.toMatchObject({ state: "live" });
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
