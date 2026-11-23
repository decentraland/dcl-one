import { afterEach, describe, expect, it, vi } from "vitest";

import { buildLiveNowCards, joinLiveNow, loadLiveNowCards, loadProfiles, plazaCard, resetLiveNowCards, withDeployers, withPlaces } from "./live-now.server";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  resetLiveNowCards();
});

const scene = (id: string, name: string, users: number, base: [number, number], parcels: [number, number][] = [base]) => ({
  id,
  name,
  baseCoords: base,
  usersTotalCount: users,
  parcels,
  thumbnail: `https://peer.example/content/contents/${id}hash`,
});

const event = { id: "ev", name: "Party", image: "https://events.example/contents/evhash", x: 10, y: 11, user: "0xAbC", user_name: "Host" };

describe("when building the Live Now cards", () => {
  it("should join each busy scene to its live event and list the other busy scenes after", () => {
    const cards = joinLiveNow([event], [scene("a", "Busy", 30, [1, 1]), scene("b", "Event scene", 8, [9, 9], [[9, 9], [10, 11]]), scene("c", "Quiet", 4, [2, 2])]);
    expect(cards.map((c) => [c.id, c.place, c.users])).toEqual([["ev", false, 8], ["a", true, 30]]);
    expect(cards[0]).toMatchObject({ creatorAddress: "0xAbC", userName: "Host", image: "/content/contents/evhash" });
  });

  it("should supply a thumbnail without a hot-scene row", () => {
    expect(plazaCard([]).image).toMatch(/\.webp$/);
  });

  it("should use the scene thumbnail when the feed supplies one", () => {
    expect(plazaCard([{ ...scene("g", "Genesis Plaza", 2, [0, 0]), thumbnail: "https://node.test/content/contents/plaza" }]).image).toBe("/content/contents/plaza");
  });

  it("should credit the Genesis Plaza card to the Foundation", () => {
    const card = plazaCard([scene("g", "Genesis Plaza", 2, [0, 0])]);
    expect([card.id, card.creatorName]).toEqual(["g", "Decentraland Foundation"]);
  });
});

describe("when a place card names no creator", () => {
  it("should ask the content server for the deployer of its scene", async () => {
    const calls: string[] = [];
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      const path = url.replace("http://node.test", "");
      calls.push(`${init?.method ?? "GET"} ${path}`);
      const body = path.startsWith("/content/entities/active") ? [{ id: "bafy1", pointers: ["1,95"] }] : { deployments: [{ entityId: "bafy1", deployedBy: "0xDEAD" }] };
      return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
    }) as typeof fetch;
    const cards = joinLiveNow([], [scene("s1", "Club", 12, [-3, -2]), scene("s2", "Lobby", 6, [1, 95])]);

    const credited = await withDeployers(cards, { fetchImpl, base: "http://node.test" });

    expect(credited.map((c) => [c.id, c.creatorAddress])).toEqual([["s1", undefined], ["s2", "0xDEAD"]]);
    expect(calls).toEqual(["POST /content/entities/active", "GET /content/deployments/?entityId=bafy1"]);
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

describe("when building the events page's Live Now cards", () => {
  it("should fall back to Genesis Plaza only when no scene is busy", () => {
    expect(buildLiveNowCards([], [scene("g", "Genesis Plaza", 2, [0, 0])]).map((c) => [c.id, c.creatorName])).toEqual([["g", "Decentraland Foundation"]]);
    expect(buildLiveNowCards([], [scene("g", "Genesis Plaza", 2, [0, 0]), scene("a", "Busy", 30, [1, 1])]).map((c) => c.id)).toEqual(["a"]);
  });
});

describe("when loading the events page's Live Now", () => {
  const OWNER = "0x797066a17F83425C1B4C7a8Cca52D19095520a52";
  const MAKER = "0x1111111111111111111111111111111111111111";
  const fetchFor = (calls: string[], places: unknown) => (async (url: string, init?: RequestInit) => {
    const path = url.replace(/^https?:\/\/[^/]+/, "");
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
    const places = { data: [{ creator_address: MAKER, contact_name: "Lobby Studio", positions: ["1,94", "1,95"], base_position: "1,94" }, { owner: OWNER, contact_name: "MetaDoge", creator_address: "", base_position: "-3,-2" }] };
    const { cards } = await loadLiveNowCards({ fetchImpl: fetchFor(calls, places), base: "http://node.test" });

    const live = calls.find((c) => c.includes("list=live"));
    expect(live).toContain("limit=20");
    expect(live).not.toContain("world");
    expect(calls.filter((c) => c.includes("positions="))).toEqual(["GET /places/api/places?positions=-3%2C-2&positions=1%2C95&limit=100"]);
    expect(cards.map((c) => [c.id, c.creatorAddress, c.creatorName])).toEqual([
      ["s1", OWNER, "MetaDoge"],
      ["s2", MAKER, "Lobby Studio"],
    ]);
    expect(calls.some((c) => c.startsWith("POST /content/entities/active"))).toBe(false);
  });

  it("should read the places of all the cards in one request and give each card the place on its parcel", async () => {
    const calls: string[] = [];
    const places = { data: [{ owner: "Yard Studio", positions: ["5,5", "5,6"], base_position: "5,5" }, { owner: "Other", positions: ["9,9"], base_position: "9,9" }, { creator_address: OWNER, base_position: "-3,-2" }] };
    const cards = joinLiveNow([], [scene("s1", "Club", 12, [-3, -2]), scene("s2", "Yard", 8, [5, 6]), scene("s3", "Lobby", 6, [1, 95])]);

    const credited = await withPlaces(cards, { fetchImpl: fetchFor(calls, places), base: "http://node.test" });

    expect(calls).toEqual(["GET /places/api/places?positions=-3%2C-2&positions=5%2C6&positions=1%2C95&limit=100"]);
    expect(credited.map((c) => [c.id, c.creatorAddress, c.creatorName])).toEqual([["s1", OWNER, undefined], ["s2", undefined, "Yard Studio"], ["s3", undefined, undefined]]);
  });

  it("should keep the cards without creators when the places read does not answer by the deadline", async () => {
    vi.useFakeTimers();
    const signals: AbortSignal[] = [];
    const answer = fetchFor([], null);
    const fetchImpl = ((url: string, init?: RequestInit) => {
      if (!url.includes("/api/places")) return answer(url, init);
      const signal = init?.signal as AbortSignal;
      signals.push(signal);
      return new Promise((_resolve, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true }));
    }) as typeof fetch;

    const pending = loadLiveNowCards({ fetchImpl, base: "http://node.test" });
    await vi.advanceTimersByTimeAsync(2_000);
    const { cards } = await pending;

    expect(signals.map((signal) => signal.aborted)).toEqual([true]);
    expect(cards.map((c) => [c.id, c.creatorAddress])).toEqual([["s1", undefined], ["s2", undefined]]);
  });

  it("should keep the cards and ask the deployers when the places read fails", async () => {
    const calls: string[] = [];
    const answer = fetchFor(calls, null);
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      const response = await answer(url, init);
      return url.includes("/api/places") ? new Response("", { status: 503 }) : response;
    }) as typeof fetch;

    const { cards } = await loadLiveNowCards({ fetchImpl, base: "http://node.test" });

    expect(calls.filter((c) => c.includes("/api/places"))).toHaveLength(1);
    expect(cards.map((c) => [c.id, c.creatorAddress])).toEqual([["s1", undefined], ["s2", "0xDEAD"]]);
  });

  it("should serve a second call inside the minute from the first read", async () => {
    const calls: string[] = [];
    vi.stubGlobal("fetch", fetchFor(calls, { data: [{ owner: OWNER, positions: ["-3,-2", "1,95"] }] }));

    const first = await loadLiveNowCards();
    const reads = calls.length;

    expect(first.cards.map((c) => c.creatorAddress)).toEqual([OWNER, OWNER]);
    expect(await loadLiveNowCards()).toBe(first);
    expect(calls).toHaveLength(reads);
  });

  it("should read again after five seconds when the places were not read", async () => {
    vi.useFakeTimers();
    const calls: string[] = [];
    vi.stubGlobal("fetch", fetchFor(calls, null));

    await loadLiveNowCards();
    const reads = calls.length;
    await vi.advanceTimersByTimeAsync(4_999);
    await loadLiveNowCards();
    expect(calls).toHaveLength(reads);
    await vi.advanceTimersByTimeAsync(1);
    await loadLiveNowCards();

    await vi.waitFor(() => expect(calls).toHaveLength(2 * reads));
  });

  it("should ask the content server for the deployer when the catalogue names nobody", async () => {
    const calls: string[] = [];
    const { cards } = await loadLiveNowCards({ fetchImpl: fetchFor(calls, { data: [] }), base: "http://node.test" });
    expect(cards.map((c) => [c.id, c.creatorAddress])).toEqual([["s1", undefined], ["s2", "0xDEAD"]]);
  });
});
