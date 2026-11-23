import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const server = vi.hoisted(() => ({
  loadJumpPlace: vi.fn(),
  loadParcelCreator: vi.fn(),
  loadWorldEntity: vi.fn(),
  loadJumpEvent: vi.fn(),
  loadCreator: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/jump/jump.server", () => server);

import { headers as jumpHeaders } from "./jump._index";
import { loader as jumpEventLoader } from "./jump.event";
import { loader as invalidEventLoader } from "./jump.events.invalid";
import { loader as invalidPlaceLoader } from "./jump.places.invalid";
import { loader as eventsLoader } from "./jump.events._index";
import { loader as placesLoader } from "./jump.places._index";

const WALLET = "0x" + "ab".repeat(20);
const EVENT_ID = "d275788c-2e03-4163-b7aa-2d997f679936";
const UNKNOWN_ID = "00000000-0000-4000-8000-000000000000";

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

const placeRowFixture = () => ({
  title: "Plaza",
  description: null,
  image: null,
  base_position: "10,20",
  owner: null,
  contact_name: "Studio",
  user_count: 3,
  world: false,
  world_name: null,
});

const eventRowFixture = () => ({
  id: EVENT_ID,
  name: "Party",
  image: null,
  description: "Come",
  next_start_at: "2099-01-01T10:00:00.000Z" as string | null,
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
});

const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)";

let placeRow: ReturnType<typeof placeRowFixture>;
let eventRow: ReturnType<typeof eventRowFixture>;

beforeEach(() => {
  placeRow = placeRowFixture();
  eventRow = eventRowFixture();
  server.loadParcelCreator.mockResolvedValue(null);
  server.loadCreator.mockResolvedValue(null);
  server.loadWorldEntity.mockResolvedValue({ id: "bafy" });
  server.loadJumpPlace.mockResolvedValue(null);
});

afterEach(() => {
  vi.resetAllMocks();
});

describe("/jump/places loader", () => {
  type PlacesData = Awaited<ReturnType<typeof placesLoader>>;
  let data: PlacesData;

  describe("when the position is malformed", () => {
    let redirect: Promise<unknown>;

    beforeEach(() => {
      redirect = redirectOf(placesLoader(args("/jump/places?position=nope")));
    });

    it("should send it to the invalid page without reading the catalogue", async () => {
      await expect(redirect).resolves.toEqual({
        status: 302,
        location: "/jump/places/invalid",
      });
      expect(server.loadJumpPlace).not.toHaveBeenCalled();
    });
  });

  describe("when the worlds server does not know the World", () => {
    beforeEach(() => {
      server.loadWorldEntity.mockResolvedValue(null);
      server.loadJumpPlace.mockResolvedValue(placeRow);
    });

    it("should send it to the invalid page", async () => {
      await expect(redirectOf(placesLoader(args("/jump?realm=gone.dcl.eth")))).resolves.toMatchObject({
        location: "/jump/places/invalid",
      });
    });
  });

  describe("when the worlds server is down", () => {
    beforeEach(async () => {
      server.loadWorldEntity.mockRejectedValue(new Error("502"));
      server.loadJumpPlace.mockResolvedValue({ ...placeRow, world: true, world_name: "foo.dcl.eth" });
      data = await placesLoader(args("/jump?realm=foo.dcl.eth"));
    });

    it("should still render a World card", () => {
      expect(data.card.location).toEqual({ world: "foo.dcl.eth", parcel: "10, 20" });
    });
  });

  describe("when the World's active scene names an owner", () => {
    beforeEach(async () => {
      server.loadWorldEntity.mockResolvedValue({ id: "bafy", metadata: { owner: WALLET } });
      server.loadCreator.mockResolvedValue({ address: WALLET, name: "owner" });
      server.loadJumpPlace.mockResolvedValue({ ...placeRow, world: true, world_name: "foo.dcl.eth" });
      data = await placesLoader(args("/jump?realm=foo.dcl.eth"));
    });

    it("should name the owner", () => {
      expect(data.card.creator.name).toBe("owner");
    });

    it("should look the scene up once", () => {
      expect(server.loadWorldEntity).toHaveBeenCalledOnce();
    });

    it("should not look the parcel's deployer up", () => {
      expect(server.loadParcelCreator).not.toHaveBeenCalled();
    });
  });

  describe("when the catalogue read fails", () => {
    beforeEach(() => {
      server.loadJumpPlace.mockRejectedValue(new Error("500"));
    });

    it("should answer 503, not the invalid page", async () => {
      await expect(redirectOf(placesLoader(args("/jump/places?position=1,2")))).resolves.toEqual({ status: 503, location: null });
    });
  });

  describe.each(["/jump?world=brai", "/jump/places?realm=https://evil.example&position=1,2"])(
    "when the realm of %s is not a World name",
    (path) => {
      it("should send it to the invalid page without reading the catalogue or the worlds server", async () => {
        await expect(redirectOf(placesLoader(args(path)))).resolves.toEqual({ status: 302, location: "/jump/places/invalid" });
        expect(server.loadJumpPlace).not.toHaveBeenCalled();
        expect(server.loadWorldEntity).not.toHaveBeenCalled();
      });
    },
  );

  describe("when the link names a parcel other than the scene's base parcel", () => {
    beforeEach(() => {
      server.loadJumpPlace.mockResolvedValue({ ...placeRow, base_position: "-3,-2" });
    });

    describe("and the visitor is on desktop", () => {
      beforeEach(async () => {
        data = await placesLoader(args("/jump?position=0,0"));
      });

      it("should show the base parcel, as decentraland.org does", () => {
        expect(data.card.location.parcel).toBe("-3, -2");
      });

      it("should jump in at the base parcel", () => {
        expect(data.card.jumpInHref).toContain("position=-3%2C-2");
      });
    });

    describe("and the visitor is on an iPhone", () => {
      beforeEach(async () => {
        data = await placesLoader(args("/jump?position=0,0", IPHONE));
      });

      it("should launch the app at the base parcel", () => {
        expect(data.launch?.href).toBe("decentraland://open?position=-3%2C-2");
      });
    });
  });

  describe("when the position is empty", () => {
    beforeEach(async () => {
      server.loadJumpPlace.mockResolvedValue(null);
      data = await placesLoader(args("/jump?position="));
    });

    it("should read it as Genesis City origin", () => {
      expect(data.card.location.parcel).toBe("0, 0");
    });
  });

  describe("when the visitor aborts the request", () => {
    let aborted: DOMException;
    let request: Request;

    beforeEach(() => {
      const controller = new AbortController();
      controller.abort();
      aborted = new DOMException("aborted", "AbortError");
      server.loadJumpPlace.mockRejectedValue(aborted);
      request = new Request("https://site.example/jump?position=1,2", { signal: controller.signal });
    });

    it("should rethrow the abort instead of counting it as an outage", async () => {
      await expect(placesLoader({ request } as never)).rejects.toBe(aborted);
    });
  });

  describe("when the parcel is not catalogued", () => {
    beforeEach(async () => {
      server.loadJumpPlace.mockResolvedValue(null);
      data = await placesLoader(args("/jump/places?position=7,8"));
    });

    it("should render a generic card", () => {
      expect(data.card).toMatchObject({ title: "", creator: { name: "Unknown" }, location: { parcel: "7, 8" } });
    });

    it("should jump in at the parcel", () => {
      expect(data.card.jumpInHref).toContain("position=7%2C8");
    });
  });

  describe("when the place is catalogued", () => {
    beforeEach(() => {
      server.loadJumpPlace.mockResolvedValue(placeRow);
    });

    describe("and the scene deployer is known", () => {
      beforeEach(async () => {
        server.loadParcelCreator.mockResolvedValue({ address: WALLET, name: "builder" });
        data = await placesLoader(args("/jump/places?position=10,20"));
      });

      it("should prefer the deployer over the catalogue contact", () => {
        expect(data.card.creator).toMatchObject({ name: "builder", href: `/profile/${WALLET}` });
      });

      it("should show the player count", () => {
        expect(data.card.badge).toEqual({ kind: "players", count: 3 });
      });
    });

    describe("and the deployer lookup fails", () => {
      beforeEach(async () => {
        server.loadParcelCreator.mockRejectedValue(new Error("down"));
        data = await placesLoader(args("/jump/places?position=10,20"));
      });

      it("should keep the catalogue contact", () => {
        expect(data.card.creator.name).toBe("Studio");
      });
    });

    describe("and the visitor is on desktop", () => {
      it("should open the web explorer", async () => {
        await expect(placesLoader(args("/jump/places?position=10,20"))).resolves.toMatchObject({ launch: null });
      });
    });

    describe("and the visitor is on Android", () => {
      beforeEach(async () => {
        data = await placesLoader(args("/jump/places?position=10,20", "Mozilla/5.0 (Linux; Android 15; Pixel 9)"));
      });

      it("should open the app through an intent", () => {
        expect(data.launch).toMatchObject({ android: true, href: expect.stringMatching(/^intent:\/\/open\?position=10%2C20#Intent;/) });
      });
    });

    describe("and the visitor is on an iPhone", () => {
      beforeEach(async () => {
        data = await placesLoader(args("/jump/places?position=10,20", IPHONE));
      });

      it("should open the app through its scheme", () => {
        expect(data.launch).toMatchObject({ android: false, href: "decentraland://open?position=10%2C20" });
      });
    });
  });
});

describe("/jump/events loader", () => {
  type EventsData = Awaited<ReturnType<typeof eventsLoader>>;
  let data: EventsData;

  describe("when the position is malformed and there is no id", () => {
    it("should send it to the invalid page", async () => {
      await expect(redirectOf(eventsLoader(args("/jump/events?position=x")))).resolves.toMatchObject({
        location: "/jump/events/invalid",
      });
    });
  });

  describe("when the event link's realm is not a World name", () => {
    it("should send it to the invalid page without asking the events service", async () => {
      await expect(redirectOf(eventsLoader(args(`/jump/events?id=${EVENT_ID}&world=brai`)))).resolves.toEqual({
        status: 302,
        location: "/jump/events/invalid",
      });
      expect(server.loadJumpEvent).not.toHaveBeenCalled();
    });
  });

  describe("when the event id is malformed", () => {
    it("should send it to the invalid page without asking the events service", async () => {
      await expect(redirectOf(eventsLoader(args("/jump/events?id=not-a-uuid")))).resolves.toEqual({
        status: 302,
        location: "/jump/events/invalid",
      });
      expect(server.loadJumpEvent).not.toHaveBeenCalled();
    });
  });

  describe("when the event is unknown", () => {
    beforeEach(() => {
      server.loadJumpEvent.mockResolvedValue(null);
    });

    it("should send it to the invalid page", async () => {
      await expect(redirectOf(eventsLoader(args(`/jump/events?id=${UNKNOWN_ID}`)))).resolves.toMatchObject({
        location: "/jump/events/invalid",
      });
    });
  });

  describe("when the events read fails", () => {
    beforeEach(() => {
      server.loadJumpEvent.mockRejectedValue(new Error("500"));
    });

    it("should answer 503, not the invalid page", async () => {
      await expect(redirectOf(eventsLoader(args(`/jump/events?id=${EVENT_ID}`)))).resolves.toEqual({ status: 503, location: null });
    });
  });

  describe("when the link names the event by position", () => {
    beforeEach(async () => {
      server.loadJumpEvent.mockResolvedValue(eventRow);
      data = await eventsLoader(args("/jump/events?position=5,-3"));
    });

    it("should share the event by its id", () => {
      expect(new URL(data.shareUrl).searchParams.get("id")).toBe(EVENT_ID);
    });

    it("should calendar the event by its id", () => {
      expect(data.calendarHref).toContain(encodeURIComponent(`id=${EVENT_ID}`));
    });
  });

  describe("when a deleted event has no start", () => {
    beforeEach(() => {
      server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null, deleted_by_admin: true });
    });

    it("should still show the deleted notice", async () => {
      await expect(eventsLoader(args(`/jump/events?id=${EVENT_ID}`))).resolves.toMatchObject({ state: "deleted" });
    });
  });

  describe("when the event is upcoming", () => {
    beforeEach(async () => {
      server.loadJumpEvent.mockResolvedValue(eventRow);
      data = await eventsLoader(args(`/jump/events?id=${EVENT_ID}`));
    });

    it("should mark it upcoming", () => {
      expect(data.state).toBe("upcoming");
    });

    it("should offer a calendar link", () => {
      expect(data.calendarHref).toContain("calendar.google.com");
    });

    it("should not offer Jump In", () => {
      expect(data.card.jumpInHref).toBeUndefined();
    });

    it("should show the attendee count", () => {
      expect(data.card.badge).toEqual({ kind: "attendees", count: 7 });
    });

    it("should render the start in UTC on the server", () => {
      expect(data.card.when).toEqual({ label: expect.stringContaining("10:00 AM UTC"), ended: false, dateTime: eventRow.next_start_at });
    });
  });

  describe("when the event is live", () => {
    beforeEach(() => {
      server.loadJumpEvent.mockResolvedValue({ ...eventRow, live: true });
    });

    describe("and its scene lookup answers", () => {
      beforeEach(async () => {
        server.loadJumpPlace.mockResolvedValue({ ...placeRow, user_count: 12 });
        data = await eventsLoader(args("/jump/events?position=5,-3"));
      });

      it("should mark it live", () => {
        expect(data.state).toBe("live");
      });

      it("should show the scene's occupancy", () => {
        expect(data.card.badge).toEqual({ kind: "live", count: 12 });
      });

      it("should offer Jump In at the event's parcel", () => {
        expect(data.card.jumpInHref).toContain("position=5%2C-3");
      });
    });

    describe("and its scene lookup fails", () => {
      beforeEach(async () => {
        server.loadJumpPlace.mockRejectedValue(new Error("500"));
        data = await eventsLoader(args(`/jump/events?id=${EVENT_ID}`));
      });

      it("should fall back to no occupancy", () => {
        expect(data.card.badge).toEqual({ kind: "live", count: 0 });
      });
    });
  });

  describe("when the host's profile lookup fails", () => {
    beforeEach(async () => {
      server.loadJumpEvent.mockResolvedValue(eventRow);
      server.loadCreator.mockRejectedValue(new Error("down"));
      data = await eventsLoader(args(`/jump/events?id=${EVENT_ID}`));
    });

    it("should keep the event's own host name", () => {
      expect(data.card.creator).toMatchObject({ name: "host", href: `/profile/${WALLET}` });
    });
  });

  describe("when the event has no start", () => {
    describe("and it is not live", () => {
      beforeEach(() => {
        server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null });
      });

      it("should offer nothing", async () => {
        await expect(eventsLoader(args(`/jump/events?id=${EVENT_ID}`))).resolves.toMatchObject({ state: "none" });
      });
    });

    describe("and it is live", () => {
      beforeEach(() => {
        server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_start_at: null, live: true });
      });

      it("should mark it live", async () => {
        await expect(eventsLoader(args(`/jump/events?id=${EVENT_ID}`))).resolves.toMatchObject({ state: "live" });
      });
    });
  });

  describe("when a live event was deleted", () => {
    beforeEach(async () => {
      server.loadJumpEvent.mockResolvedValue({ ...eventRow, live: true, deleted_by_user: true });
      data = await eventsLoader(args(`/jump/events?id=${EVENT_ID}`));
    });

    it("should mark it deleted", () => {
      expect(data.state).toBe("deleted");
    });

    it("should drop its actions", () => {
      expect(data.card.jumpInHref).toBeUndefined();
    });
  });

  describe("when the event has ended", () => {
    beforeEach(async () => {
      server.loadJumpEvent.mockResolvedValue({ ...eventRow, next_finish_at: "2000-01-01T00:00:00.000Z" });
      data = await eventsLoader(args(`/jump/events?id=${EVENT_ID}`));
    });

    it("should mark it ended", () => {
      expect(data.state).toBe("ended");
    });

    it("should say it has ended, leaving only Explore events", () => {
      expect(data.card.when).toEqual({ label: "Event has ended", ended: true });
    });
  });
});

describe("/jump/event", () => {
  let response: Response;

  beforeEach(() => {
    response = jumpEventLoader(args("/jump/event?position=0,5&realm=foo.dcl.eth"));
  });

  it("should permanently redirect", () => {
    expect(response.status).toBe(308);
  });

  it("should redirect to /jump/events keeping the query", () => {
    expect(response.headers.get("location")).toBe("/jump/events?position=0,5&realm=foo.dcl.eth");
  });
});

describe("/jump", () => {
  it("should vary on the User-Agent like /jump/places", () => {
    expect(jumpHeaders()).toEqual({ Vary: "User-Agent" });
  });
});

describe("the invalid landings", () => {
  it.each([
    ["places", invalidPlaceLoader],
    ["events", invalidEventLoader],
  ])("should answer 404 for the %s landing", (_name, load) => {
    expect(load()).toMatchObject({ init: { status: 404 } });
  });
});
