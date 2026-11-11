import { describe, expect, it, vi } from "vitest";

import { loadCreator, loadJumpEvent, loadJumpPlace, loadParcelCreator, loadWorldEntity } from "./jump.server";

const WALLET = "0x" + "ab".repeat(20);

type Route = [needle: string, make: () => Response];

function stub(routes: Route[]) {
  const urls: string[] = [];
  const fetchImpl = (async (url: string) => {
    urls.push(url);
    for (const [needle, make] of routes) if (url.includes(needle)) return make();
    throw new Error(`unstubbed ${url}`);
  }) as unknown as typeof fetch;
  return { opts: { fetchImpl, base: "https://node.example" }, urls };
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
const eventRow = {
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
};

describe("loadJumpPlace", () => {
  it("should look up a parcel by position", async () => {
    const { opts, urls } = stub([["/places/api/places", json({ ok: true, data: [placeRow({ world: false })], total: 1 })]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).resolves.toMatchObject({ title: "Scene" });
    expect(urls[0]).toContain("positions=1%2C2");
  });

  it("should read the World-level record when a World link has no position", async () => {
    const { opts, urls } = stub([["/places/api/worlds", json({ ok: true, data: [placeRow()], total: 1 })]]);
    await loadJumpPlace({ lookup: undefined, realm: "foo.dcl.eth" }, opts);
    expect(urls).toEqual(["https://node.example/places/api/worlds?names=foo.dcl.eth"]);
  });

  it("should replace a World scene's occupancy with the World's own", async () => {
    const { opts } = stub([
      ["/places/api/places", json({ ok: true, data: [placeRow({ user_count: 500 })], total: 1 })],
      ["/places/api/worlds", json({ ok: true, data: [placeRow({ user_count: 4 })], total: 1 })],
    ]);
    await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ user_count: 4 });
  });

  it("should keep the scene's occupancy when the World read fails", async () => {
    const { opts } = stub([
      ["/places/api/places", json({ ok: true, data: [placeRow({ user_count: 500 })], total: 1 })],
      ["/places/api/worlds", json({}, 503)],
    ]);
    await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ user_count: 500 });
  });

  it("should report a row that fails the generated schema and leave it out", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const { opts } = stub([["/places/api/places", json({ ok: true, data: [{ title: 1 }, placeRow({ world: false })], total: 2 })]]);
      await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).resolves.toMatchObject({ title: "Scene" });
      expect(warn).toHaveBeenCalledWith("[catalyst] Place failed schema validation", expect.anything());
    } finally {
      warn.mockRestore();
    }
  });

  it("should return null when nothing is catalogued there", async () => {
    const { opts } = stub([["/places/api/places", json({ ok: true, data: [], total: 1 })]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).resolves.toBeNull();
  });

  it("should throw when the catalogue fails", async () => {
    const { opts } = stub([["/places/api/places", json({}, 500)]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).rejects.toThrow();
  });
});

describe("loadWorldEntity", () => {
  it("should be null when the worlds server has no active scene for the name", async () => {
    const { opts, urls } = stub([["/entities/active", json([])]]);
    await expect(loadWorldEntity("Foo.dcl.eth", opts)).resolves.toBeNull();
    expect(urls[0]).toMatch(/\/entities\/active$/);
    expect(urls[0]).not.toContain("/content/");
  });

  it("should return the active scene with its owner", async () => {
    const { opts } = stub([["/entities/active", json([{ id: "bafy", metadata: { owner: WALLET } }])]]);
    await expect(loadWorldEntity("foo.dcl.eth", opts)).resolves.toMatchObject({ metadata: { owner: WALLET } });
  });

  it("should throw on an outage instead of reporting the World missing", async () => {
    const { opts } = stub([["/entities/active", json({}, 502)]]);
    await expect(loadWorldEntity("foo.dcl.eth", opts)).rejects.toThrow();
  });
});

const profile = (name: string) =>
  json({ avatars: [{ name, hasClaimedName: true, description: "", avatar: { snapshots: { face256: "face.png" } } }] });

describe("loadCreator", () => {
  it("should return null when the wallet has no profile name", async () => {
    const { opts } = stub([["/lambdas/profile/", profile("")]]);
    await expect(loadCreator(WALLET, opts)).resolves.toBeNull();
  });
});

describe("loadParcelCreator", () => {
  it("should name a parcel's deployer", async () => {
    const { opts } = stub([
      ["/content/entities/active", json([{ id: "bafy" }])],
      ["/content/deployments", json({ deployments: [{ deployedBy: WALLET }] })],
      ["/lambdas/profile/", profile("builder")],
    ]);
    await expect(loadParcelCreator([1, 2], opts)).resolves.toEqual({ address: WALLET, name: "builder", avatar: "face.png" });
  });

  it("should return null for an empty parcel", async () => {
    const { opts } = stub([["/content/entities/active", json([])]]);
    await expect(loadParcelCreator([1, 2], opts)).resolves.toBeNull();
  });

  it("should throw when the content server fails", async () => {
    const { opts } = stub([["/content/entities/active", json({}, 500)]]);
    await expect(loadParcelCreator([1, 2], opts)).rejects.toThrow();
  });
});

describe("loadJumpEvent", () => {
  it("should read an event by id", async () => {
    const { opts } = stub([["/events/api/events/e1", json({ ok: true, data: eventRow })]]);
    await expect(loadJumpEvent({ id: "e1", lookup: [0, 0], realm: undefined }, opts)).resolves.toMatchObject({ id: "e1" });
  });

  it("should return null for an unknown id", async () => {
    const { opts } = stub([["/events/api/events/nope", json({ ok: false }, 404)]]);
    await expect(loadJumpEvent({ id: "nope", lookup: [0, 0], realm: undefined }, opts)).resolves.toBeNull();
  });

  it("should return null for an id the events service rejects as malformed", async () => {
    const { opts } = stub([["/events/api/events/abc", json({ ok: false, error: "invalid id" }, 400)]]);
    await expect(loadJumpEvent({ id: "abc", lookup: [0, 0], realm: undefined }, opts)).resolves.toBeNull();
  });

  it("should look up a World's events anywhere in it when the link has no position", async () => {
    const { opts, urls } = stub([["/events/v1/events?", json({ ok: true, data: [eventRow] })]]);
    await loadJumpEvent({ id: null, lookup: undefined, realm: "foo.dcl.eth" }, opts);
    expect(urls[0]).toContain("world_names=foo.dcl.eth");
    expect(urls[0]).not.toContain("position=");
  });

  it("should throw when the events service fails", async () => {
    const { opts } = stub([["/events/api/events/e1", json({}, 500)]]);
    await expect(loadJumpEvent({ id: "e1", lookup: [0, 0], realm: undefined }, opts)).rejects.toThrow();
  });

  it("should look up a World's events by its name", async () => {
    const { opts, urls } = stub([["/events/v1/events?", json({ ok: true, data: [{ id: "broken" }, eventRow] })]]);
    await expect(loadJumpEvent({ id: null, lookup: [5, -3], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ id: "e1" });
    expect(urls[0]).toContain("position=5%2C-3");
    expect(urls[0]).toContain("world_names=foo.dcl.eth");
    expect(urls[0]).not.toContain("world=false");
  });

  it("should keep World events out of a Genesis City parcel", async () => {
    const { opts, urls } = stub([["/events/v1/events?", json({ ok: true, data: [eventRow] })]]);
    await loadJumpEvent({ id: null, lookup: [0, 0], realm: undefined }, opts);
    expect(urls[0]).toContain("world=false");
    expect(urls[0]).not.toContain("world_names");
  });

  it("should return null when no event is at the parcel", async () => {
    const { opts } = stub([["/events/v1/events?", json({ ok: true, data: [] })]]);
    await expect(loadJumpEvent({ id: null, lookup: [5, -3], realm: undefined }, opts)).resolves.toBeNull();
  });
});
