import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { loadCreator, loadJumpEvent, loadJumpPlace, loadParcelCreator, loadWorldEntity } from "./jump.server";

const WALLET = "0x" + "ab".repeat(20);

type Route = [needle: string, make: () => Response];
type Stub = ReturnType<typeof stub>;

function stub(routes: Route[]) {
  const urls: string[] = [];
  const inits: (RequestInit | undefined)[] = [];
  const fetchImpl = (async (url: string, init?: RequestInit) => {
    urls.push(url);
    inits.push(init);
    for (const [needle, make] of routes) if (url.includes(needle)) return make();
    throw new Error(`unstubbed ${url}`);
  }) as unknown as typeof fetch;
  return { opts: { fetchImpl, base: "https://node.example" }, urls, inits };
}

const json = (body: unknown, status = 200) => () => new Response(JSON.stringify(body), { status });

const placeRow = (over: Record<string, unknown> = {}) => ({
  title: "Scene",
  description: null,
  image: null,
  base_position: "1,2",
  owner: null,
  contact_name: "Studio",
  user_count: 99,
  world: true,
  world_name: "foo.dcl.eth",
  ...over,
});

/** A full event record: the events client validates the whole schema before the jump card picks its fields. */
const eventRow = () => ({
  id: "e1", name: "Party", image: null, image_vertical: null, description: null, contact: null, details: null,
  start_at: "2030-01-01T10:00:00.000Z", finish_at: "2030-01-01T12:00:00.000Z",
  next_start_at: "2030-01-01T10:00:00.000Z", next_finish_at: "2030-01-01T12:00:00.000Z", duration: 7200000,
  all_day: false, x: 5, y: -3, server: null, url: null, user: WALLET, user_name: "host", estate_id: null,
  estate_name: null, scene_name: null, approved: true, rejected: false, highlighted: false, trending: false,
  world: false, recurrent: false, recurrent_frequency: null, recurrent_weekday_mask: 0, recurrent_month_mask: 0,
  recurrent_interval: 0, recurrent_setpos: null, recurrent_monthday: null, recurrent_count: null,
  recurrent_until: null, recurrent_dates: [], categories: [], schedules: [], total_attendees: 2,
  latest_attendees: [], coordinates: [5, -3], position: [5, -3], live: false, attending: false, place_id: null,
  community_id: null, featured_item: null, created_at: null, updated_at: null, approved_by: null,
  rejected_by: null, rejection_reason: null, deleted_by_user: false, deleted_by_admin: false, deleted_by: null,
  deleted_at: null, deleted_reason: null,
});

const profile = (name: string) =>
  json([{ avatars: [{ name, hasClaimedName: true, description: "", avatar: { snapshots: { face256: "face.png" } } }] }]);

let server: Stub;

describe("loadJumpPlace", () => {
  describe("when the link carries a position", () => {
    beforeEach(() => {
      server = stub([["/places/api/places", json({ ok: true, data: [placeRow({ world: false })], total: 1 })]]);
    });

    it("should answer the parcel's place", async () => {
      await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts)).resolves.toMatchObject({ title: "Scene" });
    });

    it("should look up the parcel by position", async () => {
      await loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts);
      expect(server.urls[0]).toContain("positions=1%2C2");
    });
  });

  describe("when a World link has no position", () => {
    beforeEach(() => {
      server = stub([["/places/api/worlds", json({ ok: true, data: [placeRow()], total: 1 })]]);
    });

    it("should read the World-level record", async () => {
      await loadJumpPlace({ lookup: undefined, realm: "foo.dcl.eth" }, server.opts);
      expect(server.urls).toEqual(["https://node.example/places/api/worlds?names=foo.dcl.eth"]);
    });
  });

  describe("when the place is a World scene", () => {
    describe("and the World read succeeds", () => {
      beforeEach(() => {
        server = stub([
          ["/places/api/places", json({ ok: true, data: [placeRow({ user_count: 500 })], total: 1 })],
          ["/places/api/worlds", json({ ok: true, data: [placeRow({ user_count: 4 })], total: 1 })],
        ]);
      });

      it("should replace the scene's occupancy with the World's own", async () => {
        await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, server.opts)).resolves.toMatchObject({ user_count: 4 });
      });
    });

    describe("and the World read fails", () => {
      beforeEach(() => {
        server = stub([
          ["/places/api/places", json({ ok: true, data: [placeRow({ user_count: 500 })], total: 1 })],
          ["/places/api/worlds", json({}, 503)],
        ]);
      });

      it("should keep the scene's occupancy", async () => {
        await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, server.opts)).resolves.toMatchObject({ user_count: 500 });
      });
    });
  });

  describe("when a row fails the generated schema", () => {
    let warn: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
      warn = vi.spyOn(console, "warn").mockImplementation(() => {});
      server = stub([["/places/api/places", json({ ok: true, data: [{ title: 1 }, placeRow({ world: false })], total: 2 })]]);
    });

    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("should leave it out", async () => {
      await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts)).resolves.toMatchObject({ title: "Scene" });
    });

    it("should report it", async () => {
      await loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts);
      expect(warn).toHaveBeenCalledWith("[catalyst] Place failed schema validation", expect.anything());
    });
  });

  describe("when nothing is catalogued there", () => {
    beforeEach(() => {
      server = stub([["/places/api/places", json({ ok: true, data: [], total: 1 })]]);
    });

    it("should return null", async () => {
      await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts)).resolves.toBeNull();
    });
  });

  describe("when the catalogue fails", () => {
    beforeEach(() => {
      server = stub([["/places/api/places", json({}, 500)]]);
    });

    it("should throw", async () => {
      await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, server.opts)).rejects.toThrow("Catalyst returned 500");
    });
  });
});

describe("loadWorldEntity", () => {
  describe("when the worlds server has no active scene for the name", () => {
    beforeEach(() => {
      server = stub([["/entities/active", json([])]]);
    });

    it("should be null", async () => {
      await expect(loadWorldEntity("Foo.dcl.eth", server.opts)).resolves.toBeNull();
    });

    it("should ask the worlds server, not the content server", async () => {
      await loadWorldEntity("Foo.dcl.eth", server.opts);
      expect(server.urls[0]).toMatch(/\/entities\/active$/);
      expect(server.urls[0]).not.toContain("/content/");
    });
  });

  describe("when the World has an active scene", () => {
    beforeEach(() => {
      server = stub([["/entities/active", json([{ id: "bafy", metadata: { owner: WALLET } }])]]);
    });

    it("should return it with its owner", async () => {
      await expect(loadWorldEntity("foo.dcl.eth", server.opts)).resolves.toMatchObject({ metadata: { owner: WALLET } });
    });
  });

  describe("when the worlds server is down", () => {
    beforeEach(() => {
      server = stub([["/entities/active", json({}, 502)]]);
    });

    it("should throw instead of reporting the World missing", async () => {
      await expect(loadWorldEntity("foo.dcl.eth", server.opts)).rejects.toThrow("502");
    });
  });
});

describe("loadCreator", () => {
  describe("when the wallet has no profile name", () => {
    beforeEach(() => {
      server = stub([["/lambdas/profiles", profile("")]]);
    });

    it("should return null", async () => {
      await expect(loadCreator(WALLET, server.opts)).resolves.toBeNull();
    });
  });

  describe("when the wallet has a profile", () => {
    const mixed = "0x" + "aB".repeat(20);

    beforeEach(() => {
      server = stub([["/lambdas/profiles", profile("builder")]]);
    });

    it("should answer the creator spelled as given", async () => {
      await expect(loadCreator(mixed, server.opts)).resolves.toMatchObject({ address: mixed, name: "builder" });
    });

    it("should post the one id to the profiles route, spelled as given", async () => {
      await loadCreator(mixed, server.opts);
      expect(server.urls[0]).toBe("https://node.example/lambdas/profiles");
      expect(server.inits[0]?.method).toBe("POST");
      expect(JSON.parse(String(server.inits[0]?.body))).toEqual({ ids: [mixed] });
    });
  });

  describe("when the wallet has no profile", () => {
    beforeEach(() => {
      server = stub([["/lambdas/profiles", json([])]]);
    });

    it("should return null", async () => {
      await expect(loadCreator(WALLET, server.opts)).resolves.toBeNull();
    });
  });
});

describe("loadParcelCreator", () => {
  describe("when the parcel has a deployment", () => {
    beforeEach(() => {
      server = stub([
        ["/content/entities/active", json([{ id: "bafy" }])],
        ["/content/deployments", json({ deployments: [{ deployedBy: WALLET }] })],
        ["/lambdas/profiles", profile("builder")],
      ]);
    });

    it("should name the parcel's deployer", async () => {
      await expect(loadParcelCreator([1, 2], server.opts)).resolves.toEqual({ address: WALLET, name: "builder", avatar: "face.png", hasClaimedName: true });
    });

    it("should read the deployment of the parcel's active entity", async () => {
      await loadParcelCreator([1, 2], server.opts);
      expect(server.urls[1]).toBe("https://node.example/content/deployments/?entityId=bafy");
    });
  });

  describe("when the parcel is empty", () => {
    beforeEach(() => {
      server = stub([["/content/entities/active", json([])]]);
    });

    it("should return null", async () => {
      await expect(loadParcelCreator([1, 2], server.opts)).resolves.toBeNull();
    });
  });

  describe("when the content server fails", () => {
    beforeEach(() => {
      server = stub([["/content/entities/active", json({}, 500)]]);
    });

    it("should throw", async () => {
      await expect(loadParcelCreator([1, 2], server.opts)).rejects.toThrow("500");
    });
  });
});

describe("loadJumpEvent", () => {
  describe("when the link carries an event id", () => {
    beforeEach(() => {
      server = stub([["/events/api/events/e1", json({ ok: true, data: eventRow() })]]);
    });

    it("should read the event by id", async () => {
      await expect(loadJumpEvent({ id: "e1", lookup: [0, 0], realm: undefined }, server.opts)).resolves.toMatchObject({ id: "e1" });
    });
  });

  describe("when the id is unknown", () => {
    beforeEach(() => {
      server = stub([["/events/api/events/nope", json({ ok: false }, 404)]]);
    });

    it("should return null", async () => {
      await expect(loadJumpEvent({ id: "nope", lookup: [0, 0], realm: undefined }, server.opts)).resolves.toBeNull();
    });
  });

  describe("when the events service rejects the id as malformed", () => {
    beforeEach(() => {
      server = stub([["/events/api/events/abc", json({ ok: false, error: "invalid id" }, 400)]]);
    });

    it("should return null", async () => {
      await expect(loadJumpEvent({ id: "abc", lookup: [0, 0], realm: undefined }, server.opts)).resolves.toBeNull();
    });
  });

  describe("when the events service fails", () => {
    beforeEach(() => {
      server = stub([["/events/api/events/e1", json({}, 500)]]);
    });

    it("should throw", async () => {
      await expect(loadJumpEvent({ id: "e1", lookup: [0, 0], realm: undefined }, server.opts)).rejects.toThrow("500");
    });
  });

  describe("when a World link has no position", () => {
    beforeEach(async () => {
      server = stub([["/events/v1/events?", json({ ok: true, data: [eventRow()] })]]);
      await loadJumpEvent({ id: null, lookup: undefined, realm: "foo.dcl.eth" }, server.opts);
    });

    it("should look up the World's events by its name", () => {
      expect(server.urls[0]).toContain("world_names=foo.dcl.eth");
    });

    it("should look anywhere in the World", () => {
      expect(server.urls[0]).not.toContain("position=");
    });
  });

  describe("when a World link has a position", () => {
    beforeEach(() => {
      server = stub([["/events/v1/events?", json({ ok: true, data: [{ id: "broken" }, eventRow()] })]]);
    });

    it("should answer the first valid event", async () => {
      await expect(loadJumpEvent({ id: null, lookup: [5, -3], realm: "foo.dcl.eth" }, server.opts)).resolves.toMatchObject({ id: "e1" });
    });

    it("should look up the World's events by its name at the position", async () => {
      await loadJumpEvent({ id: null, lookup: [5, -3], realm: "foo.dcl.eth" }, server.opts);
      expect(server.urls[0]).toContain("position=5%2C-3");
      expect(server.urls[0]).toContain("world_names=foo.dcl.eth");
      expect(server.urls[0]).not.toContain("world=false");
    });
  });

  describe("when a Genesis City link has a position", () => {
    beforeEach(() => {
      server = stub([["/events/v1/events?", json({ ok: true, data: [eventRow()] })]]);
    });

    it("should answer the parcel's event", async () => {
      await expect(loadJumpEvent({ id: null, lookup: [0, 0], realm: undefined }, server.opts)).resolves.toMatchObject({ id: "e1" });
    });

    it("should ask the node for one Genesis City event at the parcel, without World events", async () => {
      await loadJumpEvent({ id: null, lookup: [0, 0], realm: undefined }, server.opts);
      expect(server.urls[0]).toContain("position=0%2C0");
      expect(server.urls[0]).toContain("world=false");
      expect(server.urls[0]).toContain("limit=1");
    });
  });

  describe("when no event is at the parcel", () => {
    beforeEach(() => {
      server = stub([["/events/v1/events?", json({ ok: true, data: [] })]]);
    });

    it("should return null", async () => {
      await expect(loadJumpEvent({ id: null, lookup: [5, -3], realm: undefined }, server.opts)).resolves.toBeNull();
    });
  });
});
