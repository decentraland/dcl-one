import { describe, expect, it } from "vitest";

import { buildLiveNowCards, loadLiveNowCards, loadProfiles } from "./live-now.server";

const scene = (id: string, name: string, users: number, base: [number, number], parcels: [number, number][] = [base]) => ({
  id,
  name,
  baseCoords: base,
  usersTotalCount: users,
  parcels,
  thumbnail: `https://peer.example/content/contents/${id}hash`,
});

const event = { id: "ev", name: "Party", image: "https://events.example/contents/evhash", x: 10, y: 11, user: "0xAbC", user_name: "Host" };

describe("when building the events page's Live Now cards", () => {
  it("should join each busy scene to its live event and list the other busy scenes after", () => {
    const cards = buildLiveNowCards([event], [scene("a", "Busy", 30, [1, 1]), scene("b", "Event scene", 8, [9, 9], [[9, 9], [10, 11]]), scene("c", "Quiet", 4, [2, 2])]);
    expect(cards.map((c) => [c.id, c.place, c.users])).toEqual([["ev", false, 8], ["a", true, 30]]);
    expect(cards[0]).toMatchObject({ creatorAddress: "0xAbC", userName: "Host", image: "/content/contents/evhash" });
  });

  it("should fall back to Genesis Plaza only when no scene is busy", () => {
    expect(buildLiveNowCards([], [scene("g", "Genesis Plaza", 2, [0, 0])]).map((c) => [c.id, c.creatorName])).toEqual([["g", "Decentraland Foundation"]]);
  });
});

describe("when loading the events page's Live Now", () => {
  const fetchFor = (calls: string[], places: unknown) => (async (url: string, init?: RequestInit) => {
    const path = url.replace("http://node.test", "");
    calls.push(`${init?.method ?? "GET"} ${path} ${init?.body ?? ""}`.trim());
    const body = path.startsWith("/hot-scenes")
      ? [scene("s1", "Club", 12, [-3, -2]), scene("s2", "Lobby", 6, [1, 95])]
      : path.includes("/api/places")
        ? places
        : path.startsWith("/content/entities/active")
          ? [{ id: "bafy1", pointers: ["1,95"] }]
          : path.startsWith("/content/deployments")
            ? { deployments: [{ entityId: "bafy1", deployedBy: "0xDEAD" }] }
            : { data: [] };
    return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
  }) as typeof fetch;

  it("should read live events of every kind and credit place cards from the catalogue", async () => {
    const calls: string[] = [];
    const places = { data: [{ owner: "0x797066a17F83425C1B4C7a8Cca52D19095520a52", contact_name: "MetaDoge", creator_address: "" }] };
    const { cards } = await loadLiveNowCards({ fetchImpl: fetchFor(calls, places), base: "http://node.test" });

    const live = calls.find((c) => c.includes("list=live"));
    expect(live).toContain("limit=20");
    expect(live).not.toContain("world");
    expect(calls.filter((c) => c.includes("positions="))).toEqual(expect.arrayContaining([expect.stringMatching(/^GET \/.*places\?positions=-3%2C-2$/)]));
    expect(cards.map((c) => [c.id, c.creatorAddress, c.creatorName])).toEqual([
      ["s1", "0x797066a17F83425C1B4C7a8Cca52D19095520a52", "MetaDoge"],
      ["s2", "0x797066a17F83425C1B4C7a8Cca52D19095520a52", "MetaDoge"],
    ]);
    expect(calls.some((c) => c.startsWith("POST /content/entities/active"))).toBe(false);
  });

  it("should ask the content server for the deployer when the catalogue names nobody", async () => {
    const calls: string[] = [];
    const { cards } = await loadLiveNowCards({ fetchImpl: fetchFor(calls, { data: [] }), base: "http://node.test" });
    expect(cards.map((c) => [c.id, c.creatorAddress])).toEqual([["s1", undefined], ["s2", "0xDEAD"]]);
  });
});

describe("when loading profiles", () => {
  it("should key each profile by its lowercase address with the face at the node's origin", async () => {
    const calls: string[] = [];
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      calls.push(`${init?.method} ${url.replace("http://node.test", "")} ${init?.body}`);
      return new Response(JSON.stringify([{ avatars: [{ name: "Maker", hasClaimedName: true, ethAddress: "0xDEAD", avatar: { snapshots: { face256: "https://profile-images.decentraland.org/entities/bafyface/face.png" } } }] }]), { status: 200 });
    }) as typeof fetch;
    const profiles = await loadProfiles(["0xDEAD", "0xdead", "0xBEEF"], { fetchImpl, base: "http://node.test" });
    expect(calls).toEqual(['POST /lambdas/profiles {"ids":["0xdead","0xbeef"]}']);
    expect(profiles).toEqual({ "0xdead": { address: "0xDEAD", name: "Maker", hasClaimedName: true, face: "/profile-images/entities/bafyface/face.png" } });
  });
});
