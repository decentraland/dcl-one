import { describe, it, expect } from "vitest";

import { hasOutfitsEntity, loadOutfits } from "./backpack";

const BODY = "urn:decentraland:off-chain:base-avatars:BaseMale";


const ADDR = "0x92de52247aeae00fcfb18072c8564f3549b64f9c";
const BASE = "https://cat.test";

type Route = (init?: RequestInit) => unknown;

function fakeFetch(routes: Record<string, Route>) {
  const calls: string[] = [];
  const fetchImpl = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    calls.push(url);
    const route = routes[url];
    if (!route) return new Response("not found", { status: 404 });
    return new Response(JSON.stringify(route(init)), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;
  return { fetchImpl, calls };
}

const ACTIVE = `${BASE}/content/entities/active`;

const ENTITY = {
  id: "bafyoutfits",
  pointers: [`${ADDR}:outfits`],
  metadata: {
    outfits: [
      {
        slot: 1,
        outfit: {
          bodyShape: BODY,
          wearables: ["urn:decentraland:off-chain:base-avatars:eyes_00"],
          skin: { color: { r: 1, g: 1, b: 1 } },
          hair: { color: { r: 0, g: 0, b: 0 } },
          eyes: { color: { r: 0, g: 0, b: 1 } },
        },
      },
    ],
  },
};

describe("loadOutfits", () => {
  it("loads saved outfits in one request and distinguishes missing entities from failures", async () => {
    const full = fakeFetch({
      [ACTIVE]: (init) => {
        expect(JSON.parse(String(init?.body))).toEqual({ pointers: [`${ADDR}:outfits`] });
        return [ENTITY];
      },
    });
    const outfits = await loadOutfits(ADDR, { base: BASE, fetchImpl: full.fetchImpl });
    expect(full.calls).toEqual([ACTIVE]);
    expect(outfits).toEqual([
      {
        slot: 1,
        bodyShape: BODY,
        wearables: ["urn:decentraland:off-chain:base-avatars:eyes_00"],
        skinColor: "#ffffff",
        hairColor: "#000000",
        eyeColor: "#0000ff",
      },
    ]);

    const noEntity = fakeFetch({ [ACTIVE]: () => [] });
    expect(await loadOutfits(ADDR, { base: BASE, fetchImpl: noEntity.fetchImpl })).toEqual([]);
    expect(noEntity.calls).toEqual([ACTIVE]);

    const emptyEntity = fakeFetch({ [ACTIVE]: () => [{ ...ENTITY, metadata: { outfits: [] } }] });
    expect(await loadOutfits(ADDR, { base: BASE, fetchImpl: emptyEntity.fetchImpl })).toEqual([]);
    expect(emptyEntity.calls).toEqual([ACTIVE]);

    const failed = fakeFetch({});
    await expect(hasOutfitsEntity(ADDR, { base: BASE, fetchImpl: failed.fetchImpl })).rejects.toThrow("Could not load saved outfits");
    await expect(loadOutfits(ADDR, { base: BASE, fetchImpl: failed.fetchImpl })).rejects.toThrow("Could not load saved outfits");
    expect(failed.calls).toEqual([ACTIVE, ACTIVE]);
    const malformed = fakeFetch({ [ACTIVE]: () => ({ error: "unavailable" }) });
    await expect(loadOutfits(ADDR, { base: BASE, fetchImpl: malformed.fetchImpl })).rejects.toThrow("unexpected response");
  });
});
