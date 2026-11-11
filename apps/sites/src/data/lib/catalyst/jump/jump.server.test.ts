import { describe, expect, it } from "vitest";

import { loadCreator, loadJumpEvent, loadJumpPlace, loadSceneCreator, worldExists } from "./jump.server";

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

const eventRow = {
  id: "e1",
  name: "Party",
  image: null,
  description: null,
  next_start_at: "2030-01-01T10:00:00.000Z",
  next_finish_at: "2030-01-01T12:00:00.000Z",
  coordinates: [5, -3],
  user: WALLET,
  user_name: "host",
  total_attendees: 2,
  live: false,
  world: false,
  server: null,
  deleted_by_user: false,
  deleted_by_admin: false,
};

describe("loadJumpPlace", () => {
  it("should look up a parcel by position", async () => {
    const { opts, urls } = stub([["/places/api/places", json({ data: [placeRow({ world: false })] })]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).resolves.toMatchObject({ title: "Scene" });
    expect(urls[0]).toContain("positions=1%2C2");
  });

  it("should read the World-level record when a World link has no position", async () => {
    const { opts, urls } = stub([["/places/api/worlds", json({ data: [placeRow()] })]]);
    await loadJumpPlace({ lookup: undefined, realm: "Foo.dcl.eth" }, opts);
    expect(urls).toEqual(["https://node.example/places/api/worlds?names=foo.dcl.eth"]);
  });

  it("should replace a World scene's occupancy with the World's own", async () => {
    const { opts } = stub([
      ["/places/api/places", json({ data: [placeRow({ user_count: 500 })] })],
      ["/places/api/worlds", json({ data: [placeRow({ user_count: 4 })] })],
    ]);
    await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ user_count: 4 });
  });

  it("should keep the scene's occupancy when the World read fails", async () => {
    const { opts } = stub([
      ["/places/api/places", json({ data: [placeRow({ user_count: 500 })] })],
      ["/places/api/worlds", json({}, 503)],
    ]);
    await expect(loadJumpPlace({ lookup: [0, 0], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ user_count: 500 });
  });

  it("should return null when nothing is catalogued there", async () => {
    const { opts } = stub([["/places/api/places", json({ data: [] })]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).resolves.toBeNull();
  });

  it("should throw when the catalogue fails", async () => {
    const { opts } = stub([["/places/api/places", json({}, 500)]]);
    await expect(loadJumpPlace({ lookup: [1, 2], realm: undefined }, opts)).rejects.toThrow();
  });
});

describe("worldExists", () => {
  it("should be false when the worlds server has no active scene for the name", async () => {
    const { opts } = stub([["/entities/active", json([])]]);
    await expect(worldExists("foo.dcl.eth", opts)).resolves.toBe(false);
  });

  it("should be true when it has one", async () => {
    const { opts } = stub([["/entities/active", json([{ id: "bafy" }])]]);
    await expect(worldExists("foo.dcl.eth", opts)).resolves.toBe(true);
  });

  it("should throw on an outage instead of reporting the World missing", async () => {
    const { opts } = stub([["/entities/active", json({}, 502)]]);
    await expect(worldExists("foo.dcl.eth", opts)).rejects.toThrow();
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

describe("loadSceneCreator", () => {
  it("should name a parcel's deployer", async () => {
    const { opts } = stub([
      ["/content/entities/active", json([{ id: "bafy" }])],
      ["/content/deployments", json({ deployments: [{ deployedBy: WALLET }] })],
      ["/lambdas/profile/", profile("builder")],
    ]);
    await expect(loadSceneCreator({ coordinates: [1, 2], realm: undefined }, opts)).resolves.toEqual({
      address: WALLET,
      name: "builder",
      avatar: "face.png",
    });
  });

  it("should name a World's owner", async () => {
    const { opts, urls } = stub([
      ["/entities/active", json([{ id: "bafy", metadata: { owner: WALLET } }])],
      ["/lambdas/profile/", profile("owner")],
    ]);
    await expect(loadSceneCreator({ coordinates: [0, 0], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ name: "owner" });
    expect(urls.some((url) => url.includes("/content/"))).toBe(false);
  });

  it("should return null for an empty parcel", async () => {
    const { opts } = stub([["/content/entities/active", json([])]]);
    await expect(loadSceneCreator({ coordinates: [1, 2], realm: undefined }, opts)).resolves.toBeNull();
  });
});

describe("loadJumpEvent", () => {
  it("should read an event by id", async () => {
    const { opts } = stub([["/events/api/events/e1", json({ ok: true, data: eventRow })]]);
    await expect(loadJumpEvent({ id: "e1", coordinates: [0, 0], realm: undefined }, opts)).resolves.toMatchObject({ id: "e1" });
  });

  it("should return null for an unknown id", async () => {
    const { opts } = stub([["/events/api/events/nope", json({ ok: false }, 404)]]);
    await expect(loadJumpEvent({ id: "nope", coordinates: [0, 0], realm: undefined }, opts)).resolves.toBeNull();
  });

  it("should throw when the events service fails", async () => {
    const { opts } = stub([["/events/api/events/e1", json({}, 500)]]);
    await expect(loadJumpEvent({ id: "e1", coordinates: [0, 0], realm: undefined }, opts)).rejects.toThrow();
  });

  it("should pick the first valid event at a World parcel", async () => {
    const { opts, urls } = stub([["/events/api/events?", json({ ok: true, data: [{ id: "broken" }, eventRow] })]]);
    await expect(loadJumpEvent({ id: null, coordinates: [5, -3], realm: "foo.dcl.eth" }, opts)).resolves.toMatchObject({ id: "e1" });
    expect(urls[0]).toContain("position=5%2C-3");
    expect(urls[0]).toContain("world_names%5B%5D=foo.dcl.eth");
  });

  it("should return null when no event is at the parcel", async () => {
    const { opts } = stub([["/events/api/events?", json({ ok: true, data: [] })]]);
    await expect(loadJumpEvent({ id: null, coordinates: [5, -3], realm: undefined }, opts)).resolves.toBeNull();
  });
});
