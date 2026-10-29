import { afterEach, describe, expect, it, vi } from "vitest";

import {
  bytesFromString,
  findWorldSize,
  formatBytes,
  liveUsersFor,
  parseWcsWorlds,
  toManagedWorld,
  wcsBase,
  WcsWorldRowSchema,
  LiveDataSchema,
} from "./wcs";
import {
  loadLiveData,
  loadMyWorlds,
  loadPlatformStatus,
  loadWalletStats,
} from "./wcs.server";

const BASE = "https://worlds-content-server.example.test";

function jsonFetch(status: number, body: unknown): typeof fetch {
  return (async () =>
    new Response(status === 204 ? null : JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    })) as unknown as typeof fetch;
}

const WORLD_ROW = {
  name: "041.dcl.eth",
  owner: "0x37b323dd852e38114933f25ad53d0c04ec4ec2bd",
  title: "Ultimate Game Party",
  description: "Template scene with SDK7 for a 4-parcel area",
  content_rating: null,
  spawn_coordinates: "0,0",
  last_deployed_at: "2023-09-06T20:13:48.672Z",
  blocked_since: null,
  deployed_scenes: 1,
  thumbnail_hash: "bafkreidj26",
};

afterEach(() => {
  vi.unstubAllEnvs();
  vi.useRealTimers();
});

describe("wcsBase", () => {
  it("derives from the ONE base, honours the old env names and an override without its trailing slash", () => {
    expect(wcsBase()).toBe("http://127.0.0.1:5210/worlds-content-server");
    expect(wcsBase("https://wcs.example.test/")).toBe("https://wcs.example.test");
    vi.stubEnv("CATALYST_URL", "https://catalyst.example.org/");
    expect(wcsBase()).toBe("https://catalyst.example.org/worlds-content-server");
    vi.stubEnv("WORLDS_URL", "http://127.0.0.1:5143");
    expect(wcsBase()).toBe("http://127.0.0.1:5143");
    vi.stubEnv("WORLDS_CONTENT_SERVER_URL", "https://worlds-content-server.decentraland.org/");
    expect(wcsBase()).toBe("https://worlds-content-server.decentraland.org");
  });
});

describe("the wcs world row is snake_case and must be adapted, not coerced", () => {
  it("adapts deployed_scenes, last_deployed_at and blocked_since, and drops only the rows that do not parse", () => {
    const world = toManagedWorld(WcsWorldRowSchema.parse(WORLD_ROW), BASE);
    expect(world.deployedScenes).toBe(1);
    expect(world.lastDeployedAt).toBe("2023-09-06T20:13:48.672Z");
    expect(world.blockedSince).toBeNull();
    expect(world.thumbnail).toBe(`${BASE}/contents/bafkreidj26`);
    const worlds = parseWcsWorlds({ total: 2, worlds: [WORLD_ROW, { nope: true }] }, BASE);
    expect(worlds.map((w) => w.name)).toEqual(["041.dcl.eth"]);
  });
});

describe("loadMyWorlds", () => {
  it("asks for the sort and page size the screen claims; a 200 with no rows is a real empty answer", async () => {
    const seen: string[] = [];
    const spy = (async (url: string) => {
      seen.push(url);
      return new Response(JSON.stringify({ worlds: [], total: 0 }), { status: 200 });
    }) as unknown as typeof fetch;
    const d = await loadMyWorlds("0xABC", { base: BASE, fetchImpl: spy });
    expect(seen[0]).toContain("authorized_deployer=0xabc");
    expect(seen[0]).toContain("limit=100");
    expect(seen[0]).toContain("sort=last_deployed_at");
    expect(seen[0]).toContain("order=desc");
    expect(d.state).toBe("live");
    if (d.state !== "live") throw new Error("unreachable");
    expect(d.value.worlds).toEqual([]);
    expect(d.value.total).toBe(0);
  });

  it("a 500 or an unreachable host yields unavailable with the endpoint in the reason and NO value key", async () => {
    const failed = await loadMyWorlds("0xabc", {
      base: BASE,
      fetchImpl: jsonFetch(500, { message: "boom" }),
    });
    expect(failed.state).toBe("unavailable");
    expect(Object.keys(failed)).not.toContain("value");
    if (failed.state !== "unavailable") throw new Error("unreachable");
    expect(failed.status).toBe(500);
    expect(failed.reason).toContain("worlds-content-server.example.test/worlds");
    expect(failed.reason).toContain("authorized_deployer=0xabc");

    const down = await loadMyWorlds("0xabc", {
      base: BASE,
      fetchImpl: (async () => {
        throw new Error("ECONNREFUSED");
      }) as unknown as typeof fetch,
    });
    expect(down.state).toBe("unavailable");
    if (down.state !== "unavailable") throw new Error("unreachable");
    expect(down.status).toBeNull();
    expect(down.reason).toContain("worlds-content-server.example.test");
  });
});

describe("loadWalletStats", () => {
  const STATS = {
    wallet: "0x37b3",
    dclNames: [{ name: "041.dcl.eth", size: "6460699" }],
    ensNames: [],
    usedSpace: "6460699",
    maxAllowedSpace: "104857600",
  };

  it("returns live with the byte counts as strings for BigInt parsing, and degrades a 404 to unavailable", async () => {
    const d = await loadWalletStats("0x37b3", { base: BASE, fetchImpl: jsonFetch(200, STATS) });
    expect(d.state).toBe("live");
    if (d.state !== "live") throw new Error("unreachable");
    expect(bytesFromString(d.value.usedSpace)).toBe(6460699n);
    expect(findWorldSize(d.value, "041.DCL.ETH")).toBe(6460699n);

    const missing = await loadWalletStats("0x0", {
      base: BASE,
      fetchImpl: jsonFetch(404, { message: "nope" }),
    });
    expect(missing.state).toBe("unavailable");
  });
});

describe("byte handling", () => {
  it("parses past Number.MAX_SAFE_INTEGER, returns null for junk rather than 0, and formats", () => {
    expect(bytesFromString("9007199254740993")).toBe(9007199254740993n);
    expect(bytesFromString("")).toBeNull();
    expect(bytesFromString("1.5")).toBeNull();
    expect(bytesFromString(null)).toBeNull();
    expect(formatBytes(null)).toBeNull();
    expect(formatBytes(512n)).toBe("512 B");
    expect(formatBytes(6460699n)).toBe("6.2 MB");
  });
});

describe("loadLiveData / loadPlatformStatus", () => {
  const LIVE = {
    data: {
      totalUsers: 5,
      perWorld: [
        { worldName: "petbarn.dcl.eth", users: 3 },
        { worldName: "pokerclub.dcl.eth", users: 1 },
      ],
    },
    lastUpdated: "2026-08-01T09:52:30.775Z",
  };

  it("parses the live-data and status envelopes", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-01T09:53:00.775Z"));
    const live = await loadLiveData({ base: BASE, fetchImpl: jsonFetch(200, LIVE) });
    expect(live.state).toBe("sampled");
    if (live.state !== "sampled") throw new Error("unreachable");
    expect(live.takenAt).toBe(LIVE.lastUpdated);
    expect(live.readAt).toBe("2026-08-01T09:53:00.775Z");
    expect(live.cadenceSeconds).toBe(30);
    expect(live.value.data.totalUsers).toBe(5);
    expect(liveUsersFor(live.value, "PETBARN.DCL.ETH")).toBe(3);
    expect(liveUsersFor(live.value, "elsewhere.dcl.eth")).toBeNull();

    const status = await loadPlatformStatus({
      base: BASE,
      fetchImpl: jsonFetch(200, {
        content: { commitHash: "66fe4f", worldsCount: { ens: 119, dcl: 1432 } },
        comms: { adapterType: "livekit", rooms: 3, users: 5 },
      }),
    });
    expect(status.state).toBe("live");
    if (status.state !== "live") throw new Error("unreachable");
    expect(status.value.content.worldsCount.dcl).toBe(1432);
    vi.useRealTimers();
  });

  it.each([null, undefined, "", "invalid", "0", "1970-01-01T00:00:00.000Z", "2026-08-01T09:53:00.776Z", "2026-08-01T09:51:30.775Z"])(
    "does not certify a successful response with unknown or expired source time %s",
    async (lastUpdated) => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-08-01T09:53:00.775Z"));
      try {
        const result = await loadLiveData({ base: BASE, fetchImpl: jsonFetch(200, { ...LIVE, lastUpdated }) });
        expect(result.state).toBe("unavailable");
        expect(result).not.toHaveProperty("value");
      } finally { vi.useRealTimers(); }
    },
  );

  it("keeps the source clock on repeated reads and expires even an empty zero response", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(LIVE.lastUpdated));
    const fetchImpl = jsonFetch(200, { ...LIVE, data: { totalUsers: 0, perWorld: [] } });
    try {
      const first = await loadLiveData({ base: BASE, fetchImpl });
      vi.advanceTimersByTime(89_999);
      const second = await loadLiveData({ base: BASE, fetchImpl });
      expect(first.state).toBe("sampled");
      expect(second.state).toBe("sampled");
      if (first.state !== "sampled" || second.state !== "sampled") throw new Error("unreachable");
      expect(second.takenAt).toBe(first.takenAt);
      expect(second.readAt).not.toBe(first.readAt);
      expect(second.value.data).toEqual({ totalUsers: 0, perWorld: [] });
      vi.advanceTimersByTime(1);
      expect((await loadLiveData({ base: BASE, fetchImpl })).state).toBe("unavailable");
    } finally { vi.useRealTimers(); }
  });

  it("degrades a live-data body missing perWorld and a status read that returns HTML, inventing nothing", async () => {
    expect(LiveDataSchema.safeParse({ data: { totalUsers: 0 }, lastUpdated: null }).success).toBe(false);
    const live = await loadLiveData({
      base: BASE,
      fetchImpl: jsonFetch(200, { data: { totalUsers: 0 }, lastUpdated: null }),
    });
    expect(live.state).toBe("unavailable");

    const html = (async () =>
      new Response("<!doctype html><html></html>", { status: 200 })) as unknown as typeof fetch;
    const status = await loadPlatformStatus({ base: BASE, fetchImpl: html });
    expect(status.state).toBe("unavailable");
  });
});
